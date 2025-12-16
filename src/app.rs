use crate::config::{Config, DisplayMode, ThemeMode};
use crate::mutagen::MutagenClient;
use crate::project::{correlate_projects_with_sessions, discover_project_files, Project};
use crate::selection::SelectionManager;
use crate::theme::{detect_theme, ColorScheme};
use crate::ui;
use anyhow::Result;
use chrono::{DateTime, Local};
use ratatui::{backend::Backend, Terminal};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionDisplayMode {
    ShowPaths,
    ShowLastRefresh,
}

#[derive(Debug, Clone)]
pub enum StatusMessage {
    Info(String),
    Warning(String),
    Error(String),
}

impl StatusMessage {
    pub fn info(msg: impl Into<String>) -> Self {
        Self::Info(msg.into())
    }

    pub fn warning(msg: impl Into<String>) -> Self {
        Self::Warning(msg.into())
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self::Error(msg.into())
    }

    pub fn text(&self) -> &str {
        match self {
            Self::Info(s) | Self::Warning(s) | Self::Error(s) => s,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BlockingOperation {
    pub message: String,
    pub current: Option<usize>,
    pub total: Option<usize>,
}

pub struct App {
    pub projects: Vec<Project>,
    pub selection: SelectionManager,
    pub should_quit: bool,
    pub status_message: Option<StatusMessage>,
    pub mutagen_client: MutagenClient,
    pub color_scheme: ColorScheme,
    pub last_refresh: Option<DateTime<Local>>,
    pub project_dir: Option<PathBuf>,
    pub session_display_mode: SessionDisplayMode,
    pub viewing_conflicts: bool,
    pub viewing_help: bool,
    pub has_refresh_error: bool, // Track if last refresh failed to prevent error loops
    pub blocking_op: Option<BlockingOperation>,
    config: Config,
}

impl App {
    pub fn new(project_dir: Option<PathBuf>) -> Self {
        // Load config (use defaults if file doesn't exist or has errors)
        let config = Config::load().unwrap_or_default();

        // Determine color scheme based on config theme setting
        let color_scheme = match config.ui.theme {
            ThemeMode::Auto => detect_theme(),
            ThemeMode::Light => ColorScheme::light(),
            ThemeMode::Dark => ColorScheme::dark(),
        };

        // Map config display mode to session display mode
        let session_display_mode = match config.ui.default_display_mode {
            DisplayMode::Paths => SessionDisplayMode::ShowPaths,
            DisplayMode::LastRefresh => SessionDisplayMode::ShowLastRefresh,
        };

        Self {
            projects: Vec::new(),
            selection: SelectionManager::new(),
            should_quit: false,
            status_message: None,
            mutagen_client: MutagenClient::new(),
            color_scheme,
            last_refresh: None,
            project_dir,
            session_display_mode,
            viewing_conflicts: false,
            viewing_help: false,
            has_refresh_error: false,
            blocking_op: None,
            config,
        }
    }

    // ============ Selection accessors (delegate to SelectionManager) ============

    pub async fn refresh_sessions(&mut self) -> Result<()> {
        match self.mutagen_client.list_sessions().await {
            Ok(sessions) => {
                // Track when successfulCycles changes to detect actual sync activity
                // We need to preserve sync_time from previous refresh
                let is_first_refresh = self.projects.is_empty();

                // Build map of old sessions by identifier for sync_time tracking
                let mut old_sessions_by_id = std::collections::HashMap::new();
                for project in &self.projects {
                    for spec in &project.specs {
                        if let Some(session) = &spec.running_session {
                            old_sessions_by_id.insert(session.identifier.clone(), session.clone());
                        }
                    }
                }

                let new_sessions: Vec<_> = sessions
                    .into_iter()
                    .map(|mut new_session| {
                        // Find the previous version of this session
                        if let Some(old_session) = old_sessions_by_id.get(&new_session.identifier) {
                            // If successfulCycles increased, we observed a sync
                            let new_cycles = new_session.successful_cycles.unwrap_or(0);
                            let old_cycles = old_session.successful_cycles.unwrap_or(0);
                            if new_cycles > old_cycles {
                                new_session.sync_time = crate::mutagen::SyncTime::At;
                            } else {
                                // Keep the previous sync_time
                                new_session.sync_time = old_session.sync_time.clone();
                            }
                        } else {
                            // Newly discovered session
                            if is_first_refresh {
                                // First refresh: all sessions pre-existed, sync history unknown
                                new_session.sync_time = crate::mutagen::SyncTime::Unknown;
                            } else {
                                // Session discovered after first refresh
                                let cycles = new_session.successful_cycles.unwrap_or(0);
                                new_session.sync_time = if cycles > 0 {
                                    crate::mutagen::SyncTime::At
                                } else {
                                    crate::mutagen::SyncTime::Never
                                };
                            }
                        }
                        new_session
                    })
                    .collect();

                // Save current fold state before rebuilding projects
                let fold_state: std::collections::HashMap<_, _> = self
                    .projects
                    .iter()
                    .map(|p| (p.file.path.clone(), p.folded))
                    .collect();

                match discover_project_files(
                    self.project_dir.as_deref(),
                    Some(&self.config.projects),
                ) {
                    Ok(project_files) => {
                        self.projects =
                            correlate_projects_with_sessions(project_files, &new_sessions);

                        // Restore fold state for existing projects, use auto-unfold for new ones
                        for project in &mut self.projects {
                            if let Some(&saved_folded) = fold_state.get(&project.file.path) {
                                project.folded = saved_folded;
                            }
                            // Otherwise keep the auto-unfold value from correlate_projects_with_sessions
                        }

                        // Sort projects alphabetically by display name
                        self.projects
                            .sort_by(|a, b| a.file.display_name().cmp(&b.file.display_name()));
                    }
                    Err(e) => {
                        // Note: Error is silently ignored here as project discovery is optional
                        // The app continues to work without project correlation
                        let _ = e; // Explicit acknowledgment of ignored error
                    }
                }

                // Rebuild selection manager from projects
                self.selection.rebuild_from_projects(&self.projects);

                self.last_refresh = Some(Local::now());
                // Only show "Sessions refreshed" if there's no status message, or if showing temporary messages
                let should_show_refreshed = self.status_message.is_none()
                    || self
                        .status_message
                        .as_ref()
                        .map(|msg| {
                            msg.text() == "Creating push session..."
                                || msg.text() == "Starting sync spec..."
                        })
                        .unwrap_or(false);

                if should_show_refreshed {
                    self.status_message = Some(StatusMessage::info("Sessions refreshed"));
                }
                self.has_refresh_error = false; // Clear error flag on success
                Ok(())
            }
            Err(e) => {
                // Display error to user but don't crash the UI
                // Transient CLI failures (missing binary, timeouts) should not tear down the terminal
                self.status_message = Some(StatusMessage::error(format!(
                    "Error: {} (press 'r' to retry)",
                    e
                )));
                self.has_refresh_error = true; // Set error flag to prevent auto-refresh loop

                // Error is displayed in the UI status bar, no need for stderr output
                Ok(())
            }
        }
    }

    pub fn select_next(&mut self) {
        self.selection.select_next();
    }

    pub fn select_previous(&mut self) {
        self.selection.select_previous();
    }

    /// Toggle fold state for a project
    pub fn toggle_project_fold(&mut self, project_idx: usize) {
        if let Some(project) = self.projects.get_mut(project_idx) {
            project.folded = !project.folded;
            // Rebuild selection items to reflect fold change
            self.selection.rebuild_from_projects(&self.projects);
        }
    }

    /// Get the selected project index (either directly or parent of selected spec)
    pub fn get_selected_project_index(&self) -> Option<usize> {
        self.selection.selected_project_index()
    }

    /// Get the selected spec (returns (project_index, spec_index) if a spec is selected)
    pub fn get_selected_spec(&self) -> Option<(usize, usize)> {
        self.selection.selected_spec()
    }

    pub async fn pause_selected(&mut self) {
        if let Some((proj_idx, spec_idx)) = self.get_selected_spec() {
            if let Some(project) = self.projects.get(proj_idx) {
                if let Some(spec) = project.specs.get(spec_idx) {
                    if let Some(session) = &spec.running_session {
                        match self.mutagen_client.pause_session(&session.identifier).await {
                            Ok(_) => {
                                self.status_message = Some(StatusMessage::info(format!(
                                    "Paused spec: {}",
                                    spec.name
                                )));
                            }
                            Err(e) => {
                                self.status_message =
                                    Some(StatusMessage::error(format!("Failed to pause: {}", e)));
                            }
                        }
                    }
                }
            }
        }
    }

    pub async fn resume_selected(&mut self) {
        if let Some((proj_idx, spec_idx)) = self.get_selected_spec() {
            if let Some(project) = self.projects.get(proj_idx) {
                if let Some(spec) = project.specs.get(spec_idx) {
                    if let Some(session) = &spec.running_session {
                        match self
                            .mutagen_client
                            .resume_session(&session.identifier)
                            .await
                        {
                            Ok(_) => {
                                self.status_message = Some(StatusMessage::info(format!(
                                    "Resumed spec: {}",
                                    spec.name
                                )));
                            }
                            Err(e) => {
                                self.status_message =
                                    Some(StatusMessage::error(format!("Failed to resume: {}", e)));
                            }
                        }
                    }
                }
            }
        }
    }

    pub async fn terminate_selected(&mut self) {
        if let Some((proj_idx, spec_idx)) = self.get_selected_spec() {
            if let Some(project) = self.projects.get(proj_idx) {
                if let Some(spec) = project.specs.get(spec_idx) {
                    if let Some(session) = &spec.running_session {
                        match self
                            .mutagen_client
                            .terminate_session(&session.identifier)
                            .await
                        {
                            Ok(_) => {
                                self.status_message = Some(StatusMessage::info(format!(
                                    "Terminated spec: {}",
                                    spec.name
                                )));
                            }
                            Err(e) => {
                                self.status_message = Some(StatusMessage::error(format!(
                                    "Failed to terminate: {}",
                                    e
                                )));
                            }
                        }
                    }
                }
            }
        }
    }

    pub async fn flush_selected(&mut self) {
        if let Some((proj_idx, spec_idx)) = self.get_selected_spec() {
            if let Some(project) = self.projects.get(proj_idx) {
                if let Some(spec) = project.specs.get(spec_idx) {
                    if let Some(session) = &spec.running_session {
                        match self.mutagen_client.flush_session(&session.identifier).await {
                            Ok(_) => {
                                self.status_message = Some(StatusMessage::info(format!(
                                    "Flushed spec: {}",
                                    spec.name
                                )));
                            }
                            Err(e) => {
                                self.status_message =
                                    Some(StatusMessage::error(format!("Failed to flush: {}", e)));
                            }
                        }
                    }
                }
            }
        }
    }

    pub async fn start_selected_spec(&mut self) {
        if let Some((project_idx, spec_idx)) = self.selection.selected_spec() {
            if let Some(project) = self.projects.get(project_idx) {
                if let Some(spec) = project.specs.get(spec_idx) {
                    // Don't start if already running
                    if spec.is_running() {
                        self.status_message = Some(StatusMessage::warning(format!(
                            "Spec already running: {}",
                            spec.name
                        )));
                        return;
                    }

                    // Get session definition from project file
                    if let Some(session_def) = project.file.sessions.get(&spec.name) {
                        // Get defaults for ignore patterns
                        let defaults_value = project
                            .file
                            .defaults
                            .as_ref()
                            .and_then(|defaults| serde_yaml::to_value(defaults).ok());

                        // Extract ignore patterns (same as push_selected_spec)
                        let ignore_patterns =
                            session_def.get_ignore_patterns(defaults_value.as_ref());
                        let ignore = if ignore_patterns.is_empty() {
                            None
                        } else {
                            Some(ignore_patterns)
                        };

                        // Ensure directories exist (same pattern as push_selected_spec)
                        if let Err(e) = self
                            .mutagen_client
                            .ensure_endpoint_directory_exists(&session_def.alpha)
                            .await
                        {
                            self.status_message = Some(StatusMessage::error(format!(
                                "Failed to create alpha directory: {}",
                                e
                            )));
                            return;
                        }
                        if let Err(e) = self
                            .mutagen_client
                            .ensure_endpoint_directory_exists(&session_def.beta)
                            .await
                        {
                            self.status_message = Some(StatusMessage::error(format!(
                                "Failed to create beta directory: {}",
                                e
                            )));
                            return;
                        }

                        // Create two-way session
                        match self
                            .mutagen_client
                            .create_two_way_session(
                                &spec.name,
                                &session_def.alpha,
                                &session_def.beta,
                                ignore.as_deref(),
                            )
                            .await
                        {
                            Ok(_) => {
                                self.status_message = Some(StatusMessage::info(format!(
                                    "Started spec: {}",
                                    spec.name
                                )));
                            }
                            Err(e) => {
                                self.status_message = Some(StatusMessage::error(format!(
                                    "Failed to start spec: {}",
                                    e
                                )));
                            }
                        }
                    } else {
                        self.status_message = Some(StatusMessage::error(format!(
                            "Session definition not found: {}",
                            spec.name
                        )));
                    }
                }
            }
        }
    }

    pub async fn start_selected_project(&mut self) {
        if let Some(project_idx) = self.get_selected_project_index() {
            if let Some(project) = self.projects.get(project_idx) {
                match self.mutagen_client.start_project(&project.file.path).await {
                    Ok(_) => {
                        self.status_message = Some(StatusMessage::info(format!(
                            "Started project: {}",
                            project.file.display_name()
                        )));
                    }
                    Err(e) => {
                        self.status_message = Some(StatusMessage::error(format!(
                            "Failed to start project: {}",
                            e
                        )));
                    }
                }
            }
        }
    }

    pub async fn terminate_selected_project<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<()> {
        if let Some(project_idx) = self.get_selected_project_index() {
            if let Some(project) = self.projects.get(project_idx) {
                let running_specs: Vec<_> =
                    project.specs.iter().filter(|s| s.is_running()).collect();

                if running_specs.is_empty() {
                    self.status_message =
                        Some(StatusMessage::info("No running specs to terminate"));
                    return Ok(());
                }

                let total = running_specs.len();
                let mut terminated_count = 0;
                let mut errors: Vec<String> = Vec::new();

                for (idx, spec) in running_specs.iter().enumerate() {
                    // Update progress
                    if let Some(ref mut blocking_op) = self.blocking_op {
                        blocking_op.current = Some(idx + 1);
                        blocking_op.total = Some(total);
                    }
                    // Redraw to show progress
                    terminal.draw(|f| ui::draw(f, self))?;

                    if let Some(session) = &spec.running_session {
                        match self
                            .mutagen_client
                            .terminate_session(&session.identifier)
                            .await
                        {
                            Ok(_) => terminated_count += 1,
                            Err(e) => errors.push(format!("{}: {}", spec.name, e)),
                        }
                    }
                }

                // Status message (follows pattern from push_selected_project)
                if terminated_count > 0 && errors.is_empty() {
                    self.status_message = Some(StatusMessage::info(format!(
                        "Terminated {} session(s)",
                        terminated_count
                    )));
                } else if terminated_count > 0 && !errors.is_empty() {
                    self.status_message = Some(StatusMessage::warning(format!(
                        "Terminated {} session(s), {} failed. First error: {}",
                        terminated_count,
                        errors.len(),
                        errors[0]
                    )));
                } else {
                    self.status_message = Some(StatusMessage::error(format!(
                        "Failed to terminate {} session(s). First error: {}",
                        errors.len(),
                        errors[0]
                    )));
                }
            }
        }
        Ok(())
    }

    pub async fn flush_selected_project<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<()> {
        if let Some(project_idx) = self.get_selected_project_index() {
            if let Some(project) = self.projects.get(project_idx) {
                let running_specs: Vec<_> =
                    project.specs.iter().filter(|s| s.is_running()).collect();

                if running_specs.is_empty() {
                    self.status_message = Some(StatusMessage::info("No running specs to flush"));
                    return Ok(());
                }

                let total = running_specs.len();
                let mut flushed_count = 0;
                let mut errors: Vec<String> = Vec::new();

                for (idx, spec) in running_specs.iter().enumerate() {
                    // Update progress
                    if let Some(ref mut blocking_op) = self.blocking_op {
                        blocking_op.current = Some(idx + 1);
                        blocking_op.total = Some(total);
                    }
                    // Redraw to show progress
                    terminal.draw(|f| ui::draw(f, self))?;

                    if let Some(session) = &spec.running_session {
                        match self.mutagen_client.flush_session(&session.identifier).await {
                            Ok(_) => flushed_count += 1,
                            Err(e) => errors.push(format!("{}: {}", spec.name, e)),
                        }
                    }
                }

                // Status message (same pattern as terminate)
                if flushed_count > 0 && errors.is_empty() {
                    self.status_message = Some(StatusMessage::info(format!(
                        "Flushed {} session(s)",
                        flushed_count
                    )));
                } else if flushed_count > 0 && !errors.is_empty() {
                    self.status_message = Some(StatusMessage::warning(format!(
                        "Flushed {} session(s), {} failed. First error: {}",
                        flushed_count,
                        errors.len(),
                        errors[0]
                    )));
                } else {
                    self.status_message = Some(StatusMessage::error(format!(
                        "Failed to flush {} session(s). First error: {}",
                        errors.len(),
                        errors[0]
                    )));
                }
            }
        }
        Ok(())
    }

    pub async fn resume_selected_project<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<()> {
        if let Some(project_idx) = self.get_selected_project_index() {
            if let Some(project) = self.projects.get(project_idx) {
                let paused_specs: Vec<_> = project
                    .specs
                    .iter()
                    .filter(|s| s.running_session.as_ref().is_some_and(|sess| sess.paused))
                    .collect();

                if paused_specs.is_empty() {
                    self.status_message = Some(StatusMessage::info("No paused specs to resume"));
                    return Ok(());
                }

                let total = paused_specs.len();
                let mut resumed_count = 0;
                let mut errors: Vec<String> = Vec::new();

                for (idx, spec) in paused_specs.iter().enumerate() {
                    // Update progress
                    if let Some(ref mut blocking_op) = self.blocking_op {
                        blocking_op.current = Some(idx + 1);
                        blocking_op.total = Some(total);
                    }
                    // Redraw to show progress
                    terminal.draw(|f| ui::draw(f, self))?;

                    if let Some(session) = &spec.running_session {
                        match self
                            .mutagen_client
                            .resume_session(&session.identifier)
                            .await
                        {
                            Ok(_) => resumed_count += 1,
                            Err(e) => errors.push(format!("{}: {}", spec.name, e)),
                        }
                    }
                }

                // Status message (same pattern)
                if resumed_count > 0 && errors.is_empty() {
                    self.status_message = Some(StatusMessage::info(format!(
                        "Resumed {} session(s)",
                        resumed_count
                    )));
                } else if resumed_count > 0 && !errors.is_empty() {
                    self.status_message = Some(StatusMessage::warning(format!(
                        "Resumed {} session(s), {} failed. First error: {}",
                        resumed_count,
                        errors.len(),
                        errors[0]
                    )));
                } else {
                    self.status_message = Some(StatusMessage::error(format!(
                        "Failed to resume {} session(s). First error: {}",
                        errors.len(),
                        errors[0]
                    )));
                }
            }
        }
        Ok(())
    }

    pub async fn push_selected_project<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<()> {
        if let Some(project_idx) = self.get_selected_project_index() {
            if let Some(project) = self.projects.get(project_idx) {
                // Terminate all running sessions for this project before creating push sessions
                for spec in &project.specs {
                    if let Some(session) = &spec.running_session {
                        let _ = self
                            .mutagen_client
                            .terminate_session(&session.identifier)
                            .await;
                    }
                }

                if project.file.sessions.is_empty() {
                    self.status_message =
                        Some(StatusMessage::error("No sessions defined in project file"));
                    return Ok(());
                }

                // Create push sessions for ALL sessions in the project
                let mut created_count = 0;
                let mut errors: Vec<(String, String)> = Vec::new();
                let total_sessions = project.file.sessions.len();

                // Get defaults for ignore patterns
                let defaults_value = project
                    .file
                    .defaults
                    .as_ref()
                    .and_then(|defaults| serde_yaml::to_value(defaults).ok());

                for (idx, (session_name, session_def)) in project.file.sessions.iter().enumerate() {
                    // Update progress
                    if let Some(ref mut blocking_op) = self.blocking_op {
                        blocking_op.current = Some(idx + 1);
                        blocking_op.total = Some(total_sessions);
                    }
                    // Redraw to show progress
                    terminal.draw(|f| ui::draw(f, self))?;
                    let push_name = format!("{}-push", session_name);

                    // Extract ignore patterns, merging with defaults
                    let ignore_patterns = session_def.get_ignore_patterns(defaults_value.as_ref());
                    let ignore = if ignore_patterns.is_empty() {
                        None
                    } else {
                        Some(ignore_patterns)
                    };

                    // Ensure both endpoints' parent directories exist before creating session
                    if let Err(e) = self
                        .mutagen_client
                        .ensure_endpoint_directory_exists(&session_def.alpha)
                        .await
                    {
                        errors.push((
                            session_name.clone(),
                            format!("Failed to create alpha directory: {}", e),
                        ));
                        continue;
                    }
                    if let Err(e) = self
                        .mutagen_client
                        .ensure_endpoint_directory_exists(&session_def.beta)
                        .await
                    {
                        errors.push((
                            session_name.clone(),
                            format!("Failed to create beta directory: {}", e),
                        ));
                        continue;
                    }

                    match self
                        .mutagen_client
                        .create_push_session(
                            &push_name,
                            &session_def.alpha,
                            &session_def.beta,
                            ignore.as_deref(),
                        )
                        .await
                    {
                        Ok(_) => {
                            created_count += 1;
                        }
                        Err(e) => {
                            errors.push((session_name.clone(), e.to_string()));
                        }
                    }
                }

                // Set status message based on results
                if created_count > 0 && errors.is_empty() {
                    let msg = if created_count == total_sessions {
                        format!("Created {} push session(s)", created_count)
                    } else {
                        format!(
                            "Created {} of {} push session(s)",
                            created_count, total_sessions
                        )
                    };
                    self.status_message = Some(StatusMessage::info(msg));
                } else if created_count > 0 && !errors.is_empty() {
                    // Show first error for context
                    let first_error = &errors[0];
                    self.status_message = Some(StatusMessage::warning(format!(
                        "Created {} push session(s), {} failed. First error: {}: {}",
                        created_count,
                        errors.len(),
                        first_error.0,
                        first_error.1
                    )));
                } else {
                    // All failed
                    let error_msg = if errors.len() == 1 {
                        format!(
                            "Failed to create push session {}: {}",
                            errors[0].0, errors[0].1
                        )
                    } else {
                        let error_details: Vec<String> = errors
                            .iter()
                            .map(|(name, err)| format!("{}: {}", name, err))
                            .collect();
                        format!(
                            "Failed to create {} push sessions: {}",
                            errors.len(),
                            error_details.join("; ")
                        )
                    };
                    self.status_message = Some(StatusMessage::error(error_msg));
                }
            } else {
                self.status_message = Some(StatusMessage::error("Failed to get selected project"));
            }
        } else {
            self.status_message = Some(StatusMessage::error("No project selected"));
        }
        Ok(())
    }

    /// Create a push session for the selected spec, replacing any existing two-way session.
    pub async fn push_selected_spec(&mut self) {
        if let Some((project_idx, spec_idx)) = self.selection.selected_spec() {
            if let Some(project) = self.projects.get(project_idx) {
                if let Some(spec) = project.specs.get(spec_idx) {
                    // Terminate any running two-way session for this spec
                    if let Some(session) = &spec.running_session {
                        if spec.state == crate::project::SyncSpecState::RunningTwoWay {
                            let _ = self
                                .mutagen_client
                                .terminate_session(&session.identifier)
                                .await;
                        }
                    }

                    // Get the session definition from the project file
                    if let Some(session_def) = project.file.sessions.get(&spec.name) {
                        let push_name = format!("{}-push", spec.name);

                        // Get defaults for ignore patterns
                        let defaults_value = project
                            .file
                            .defaults
                            .as_ref()
                            .and_then(|defaults| serde_yaml::to_value(defaults).ok());

                        // Extract ignore patterns, merging with defaults
                        let ignore_patterns =
                            session_def.get_ignore_patterns(defaults_value.as_ref());
                        let ignore = if ignore_patterns.is_empty() {
                            None
                        } else {
                            Some(ignore_patterns)
                        };

                        // Ensure both endpoints' parent directories exist
                        if let Err(e) = self
                            .mutagen_client
                            .ensure_endpoint_directory_exists(&session_def.alpha)
                            .await
                        {
                            self.status_message = Some(StatusMessage::error(format!(
                                "Failed to create alpha directory: {}",
                                e
                            )));
                            return;
                        }
                        if let Err(e) = self
                            .mutagen_client
                            .ensure_endpoint_directory_exists(&session_def.beta)
                            .await
                        {
                            self.status_message = Some(StatusMessage::error(format!(
                                "Failed to create beta directory: {}",
                                e
                            )));
                            return;
                        }

                        // Create the push session
                        match self
                            .mutagen_client
                            .create_push_session(
                                &push_name,
                                &session_def.alpha,
                                &session_def.beta,
                                ignore.as_deref(),
                            )
                            .await
                        {
                            Ok(_) => {
                                self.status_message = Some(StatusMessage::info(format!(
                                    "Created push session: {}",
                                    push_name
                                )));
                            }
                            Err(e) => {
                                self.status_message = Some(StatusMessage::error(format!(
                                    "Failed to create push session: {}",
                                    e
                                )));
                            }
                        }
                    } else {
                        self.status_message = Some(StatusMessage::error(format!(
                            "Session definition not found: {}",
                            spec.name
                        )));
                    }
                } else {
                    self.status_message = Some(StatusMessage::error("Failed to get selected spec"));
                }
            } else {
                self.status_message = Some(StatusMessage::error("Failed to get selected project"));
            }
        } else {
            self.status_message = Some(StatusMessage::error("No spec selected"));
        }
    }

    pub async fn pause_selected_project<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<()> {
        if let Some(project_idx) = self.get_selected_project_index() {
            if let Some(project) = self.projects.get(project_idx) {
                let running_specs: Vec<_> =
                    project.specs.iter().filter(|s| s.is_running()).collect();

                if running_specs.is_empty() {
                    self.status_message = Some(StatusMessage::info("No running specs to pause"));
                    return Ok(());
                }

                let total = running_specs.len();
                // Pause ALL running sessions individually
                let mut paused_count = 0;
                let mut errors: Vec<String> = Vec::new();

                for (idx, spec) in running_specs.iter().enumerate() {
                    // Update progress
                    if let Some(ref mut blocking_op) = self.blocking_op {
                        blocking_op.current = Some(idx + 1);
                        blocking_op.total = Some(total);
                    }
                    // Redraw to show progress
                    terminal.draw(|f| ui::draw(f, self))?;

                    if let Some(session) = &spec.running_session {
                        match self.mutagen_client.pause_session(&session.identifier).await {
                            Ok(_) => paused_count += 1,
                            Err(e) => errors.push(format!("{}: {}", spec.name, e)),
                        }
                    }
                }

                // Set status message based on results
                if paused_count > 0 && errors.is_empty() {
                    self.status_message = Some(StatusMessage::info(format!(
                        "Paused {} session(s)",
                        paused_count
                    )));
                } else if paused_count > 0 && !errors.is_empty() {
                    self.status_message = Some(StatusMessage::warning(format!(
                        "Paused {} session(s), {} failed",
                        paused_count,
                        errors.len()
                    )));
                } else {
                    self.status_message = Some(StatusMessage::error(format!(
                        "Failed to pause {} session(s)",
                        errors.len()
                    )));
                }
            }
        }
        Ok(())
    }

    pub async fn toggle_pause_selected<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<()> {
        if let Some((proj_idx, spec_idx)) = self.get_selected_spec() {
            // Individual spec selected - toggle its pause state
            if let Some(project) = self.projects.get(proj_idx) {
                if let Some(spec) = project.specs.get(spec_idx) {
                    if spec.is_paused() {
                        self.resume_selected().await;
                    } else {
                        self.pause_selected().await;
                    }
                }
            }
        } else if let Some(project_idx) = self.get_selected_project_index() {
            // Project selected - toggle pause for all its running specs
            if let Some(project) = self.projects.get(project_idx) {
                let running_specs: Vec<_> =
                    project.specs.iter().filter(|s| s.is_running()).collect();

                if running_specs.is_empty() {
                    self.status_message = Some(StatusMessage::info(
                        "Project has no running specs. Use 's' to start.",
                    ));
                    return Ok(());
                }

                // Check if any spec is running (not paused)
                let has_running = running_specs.iter().any(|s| !s.is_paused());
                if has_running {
                    self.pause_selected_project(terminal).await?;
                } else {
                    self.resume_selected_project(terminal).await?;
                }
            }
        }
        Ok(())
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn toggle_session_display(&mut self) {
        self.session_display_mode = match self.session_display_mode {
            SessionDisplayMode::ShowPaths => SessionDisplayMode::ShowLastRefresh,
            SessionDisplayMode::ShowLastRefresh => SessionDisplayMode::ShowPaths,
        };
        self.status_message = Some(StatusMessage::info(format!(
            "Display mode: {}",
            match self.session_display_mode {
                SessionDisplayMode::ShowPaths => "Paths",
                SessionDisplayMode::ShowLastRefresh => "Last Sync Time",
            }
        )));
    }

    pub fn toggle_conflict_view(&mut self) {
        if let Some((proj_idx, spec_idx)) = self.get_selected_spec() {
            if let Some(project) = self.projects.get(proj_idx) {
                if let Some(spec) = project.specs.get(spec_idx) {
                    if spec.has_conflicts() {
                        self.viewing_conflicts = !self.viewing_conflicts;
                        if self.viewing_conflicts {
                            self.status_message = Some(StatusMessage::info(format!(
                                "Viewing conflicts for: {}",
                                spec.name
                            )));
                        } else {
                            self.status_message = Some(StatusMessage::info("Closed conflict view"));
                        }
                    } else {
                        self.status_message =
                            Some(StatusMessage::error("No conflicts in selected spec"));
                    }
                }
            }
        } else {
            self.status_message = Some(StatusMessage::error("Select a spec to view conflicts"));
        }
    }

    pub fn toggle_help_view(&mut self) {
        self.viewing_help = !self.viewing_help;
    }

    pub fn get_selected_spec_conflicts(&self) -> Option<&Vec<crate::mutagen::Conflict>> {
        if let Some((proj_idx, spec_idx)) = self.get_selected_spec() {
            if let Some(project) = self.projects.get(proj_idx) {
                if let Some(spec) = project.specs.get(spec_idx) {
                    return spec.conflicts();
                }
            }
        }
        None
    }

    pub fn should_auto_refresh(&self) -> bool {
        // Check if auto-refresh is enabled in config
        if !self.config.refresh.enabled {
            return false;
        }

        // Don't auto-refresh if the last refresh resulted in an error
        // User must manually retry with 'r' to clear the error state
        if self.has_refresh_error {
            return false;
        }

        let interval_secs = self.config.refresh.interval_secs as i64;

        match self.last_refresh {
            Some(last) => {
                let elapsed = Local::now().signed_duration_since(last);
                elapsed.num_seconds() >= interval_secs
            }
            None => true,
        }
    }
}
