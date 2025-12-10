use crate::config::{Config, DisplayMode, ThemeMode};
use crate::mutagen::{MutagenClient, SyncSession};
use crate::project::{correlate_projects_with_sessions, discover_project_files, Project};
use crate::selection::SelectionManager;
use crate::theme::{detect_theme, ColorScheme};
use anyhow::Result;
use chrono::{DateTime, Local};
use std::path::PathBuf;

// Re-export FocusArea for external use (e.g., in ui.rs)
pub use crate::selection::FocusArea;

/// Represents a selectable item in the sessions panel.
/// This includes both project headers (for grouped sessions) and individual sessions.
#[derive(Debug, Clone)]
pub enum SessionPanelItem {
    /// A project header row (for grouping sessions under a project)
    ProjectHeader {
        /// Index into app.projects
        project_index: usize,
    },
    /// An individual session row
    Session {
        /// Index into app.sessions
        session_index: usize,
    },
}

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

    #[allow(dead_code)] // Part of public API, may be used in future
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }
}

/// Get the display name for a session to use for sorting.
/// If the session belongs to a project, returns "project-name > session-name".
/// Otherwise, returns just the session name.
fn get_session_sort_key(session: &SyncSession, projects: &[Project]) -> String {
    for project in projects {
        if project
            .active_sessions
            .iter()
            .any(|s| s.name == session.name)
        {
            let project_name = project.file.display_name();
            return format!("{} > {}", project_name, session.name);
        }
    }
    session.name.clone()
}

/// Find which project a session belongs to (by index), if any.
fn find_session_project_index(session: &SyncSession, projects: &[Project]) -> Option<usize> {
    projects
        .iter()
        .position(|p| p.active_sessions.iter().any(|s| s.name == session.name))
}

/// Build the list of selectable items for the sessions panel.
/// Groups sessions by project and creates a flat list with project headers followed by their sessions.
fn build_session_panel_items(
    sessions: &[SyncSession],
    projects: &[Project],
) -> Vec<SessionPanelItem> {
    use std::collections::{HashMap, HashSet};

    // First, group sessions by project
    let mut project_sessions: HashMap<Option<usize>, Vec<usize>> = HashMap::new();
    for (session_idx, session) in sessions.iter().enumerate() {
        let project_idx = find_session_project_index(session, projects);
        project_sessions
            .entry(project_idx)
            .or_default()
            .push(session_idx);
    }

    // Build the display list in order sessions appear
    let mut result = Vec::new();
    let mut processed_projects: HashSet<Option<usize>> = HashSet::new();

    for session in sessions {
        let project_idx = find_session_project_index(session, projects);

        if processed_projects.contains(&project_idx) {
            continue;
        }
        processed_projects.insert(project_idx);

        let session_indices = project_sessions.remove(&project_idx).unwrap_or_default();

        if let Some(proj_idx) = project_idx {
            // Project with sessions: add header first, then sessions
            result.push(SessionPanelItem::ProjectHeader {
                project_index: proj_idx,
            });
            for idx in session_indices {
                result.push(SessionPanelItem::Session { session_index: idx });
            }
        } else {
            // Standalone sessions (no project): add directly
            for idx in session_indices {
                result.push(SessionPanelItem::Session { session_index: idx });
            }
        }
    }

    result
}

#[derive(Debug, Clone)]
pub struct BlockingOperation {
    pub message: String,
}

pub struct App {
    pub sessions: Vec<SyncSession>,
    pub projects: Vec<Project>,
    /// Ordered list of selectable items in the sessions panel (project headers + sessions)
    pub session_panel_items: Vec<SessionPanelItem>,
    selection: SelectionManager,
    pub should_quit: bool,
    pub status_message: Option<StatusMessage>,
    pub mutagen_client: MutagenClient,
    pub color_scheme: ColorScheme,
    pub last_refresh: Option<DateTime<Local>>,
    pub project_dir: Option<PathBuf>,
    pub session_display_mode: SessionDisplayMode,
    pub viewing_conflicts: bool,
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
            sessions: Vec::new(),
            projects: Vec::new(),
            session_panel_items: Vec::new(),
            selection: SelectionManager::new(),
            should_quit: false,
            status_message: None,
            mutagen_client: MutagenClient::new(),
            color_scheme,
            last_refresh: None,
            project_dir,
            session_display_mode,
            viewing_conflicts: false,
            has_refresh_error: false,
            blocking_op: None,
            config,
        }
    }

    // ============ Selection accessors (delegate to SelectionManager) ============

    /// Get the raw selected index for UI rendering.
    pub fn selected_index(&self) -> usize {
        self.selection.raw_index()
    }

    pub async fn refresh_sessions(&mut self) -> Result<()> {
        match self.mutagen_client.list_sessions().await {
            Ok(sessions) => {
                let now = Local::now();

                // Track when successfulCycles changes to detect actual sync activity
                let is_first_refresh = self.sessions.is_empty();
                let new_sessions: Vec<_> = sessions
                    .into_iter()
                    .map(|mut new_session| {
                        // Find the previous version of this session
                        if let Some(old_session) = self
                            .sessions
                            .iter()
                            .find(|s| s.identifier == new_session.identifier)
                        {
                            // If successfulCycles increased, we observed a sync
                            let new_cycles = new_session.successful_cycles.unwrap_or(0);
                            let old_cycles = old_session.successful_cycles.unwrap_or(0);
                            if new_cycles > old_cycles {
                                new_session.sync_time = crate::mutagen::SyncTime::At(now);
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
                                    crate::mutagen::SyncTime::At(now)
                                } else {
                                    crate::mutagen::SyncTime::Never
                                };
                            }
                        }
                        new_session
                    })
                    .collect();

                // Sort sessions alphabetically by name
                let mut sorted_sessions = new_sessions;
                sorted_sessions.sort_by(|a, b| a.name.cmp(&b.name));

                self.sessions = sorted_sessions;

                match discover_project_files(
                    self.project_dir.as_deref(),
                    Some(&self.config.projects),
                ) {
                    Ok(project_files) => {
                        self.projects =
                            correlate_projects_with_sessions(project_files, &self.sessions);

                        // Re-sort sessions by display name (including project prefix) now that we have project correlation
                        self.sessions.sort_by(|a, b| {
                            let a_key = get_session_sort_key(a, &self.projects);
                            let b_key = get_session_sort_key(b, &self.projects);
                            a_key.cmp(&b_key)
                        });
                    }
                    Err(e) => {
                        // Note: Error is silently ignored here as project discovery is optional
                        // The app continues to work without project correlation
                        let _ = e; // Explicit acknowledgment of ignored error
                    }
                }

                // Build session panel items (project headers + sessions in display order)
                self.session_panel_items =
                    build_session_panel_items(&self.sessions, &self.projects);

                // Update selection manager with new list sizes (handles clamping and focus)
                // Use session_panel_items.len() which includes both project headers and sessions
                self.selection
                    .update_sizes(self.projects.len(), self.session_panel_items.len());

                self.last_refresh = Some(Local::now());
                // Only show "Sessions refreshed" if there's no status message, or if showing the temporary "Creating push session..." message
                // Preserve all other messages (errors, warnings, and operation success messages like "Created push session: xyz")
                let should_show_refreshed = self.status_message.is_none()
                    || self
                        .status_message
                        .as_ref()
                        .map(|msg| msg.text() == "Creating push session...")
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

    pub fn toggle_focus_area(&mut self) {
        // Save current context for related item lookup
        let last_session_idx = self.selection.selected_session();
        let last_project_idx = self.selection.selected_project();

        self.selection.toggle_focus();

        // Try to find related items after basic toggle
        match self.selection.focus_area() {
            FocusArea::Projects => {
                // If we just came from a session, try to find its parent project
                if let Some(session_idx) = last_session_idx {
                    if let Some(session) = self.sessions.get(session_idx) {
                        for (proj_idx, project) in self.projects.iter().enumerate() {
                            if project
                                .active_sessions
                                .iter()
                                .any(|s| s.name == session.name)
                            {
                                self.selection.select_project(proj_idx);
                                return;
                            }
                        }
                    }
                }
            }
            FocusArea::Sessions => {
                // If we just came from a project, try to find its first session
                if let Some(proj_idx) = last_project_idx {
                    if let Some(project) = self.projects.get(proj_idx) {
                        if let Some(active_session) = project.active_sessions.first() {
                            for (sess_idx, session) in self.sessions.iter().enumerate() {
                                if session.name == active_session.name {
                                    self.selection.select_session(sess_idx);
                                    return;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Get the selected project index from the Projects panel.
    pub fn get_selected_project_index(&self) -> Option<usize> {
        self.selection.selected_project()
    }

    /// Get the currently selected item in the sessions panel.
    pub fn get_selected_session_panel_item(&self) -> Option<&SessionPanelItem> {
        self.selection
            .selected_session()
            .and_then(|idx| self.session_panel_items.get(idx))
    }

    /// Get the selected session index (from either the sessions panel Session item).
    /// Returns None if a ProjectHeader is selected in the sessions panel.
    pub fn get_selected_session_index(&self) -> Option<usize> {
        match self.get_selected_session_panel_item() {
            Some(SessionPanelItem::Session { session_index }) => Some(*session_index),
            _ => None,
        }
    }

    /// Get the project index if a ProjectHeader is selected in the sessions panel.
    pub fn get_selected_session_panel_project_index(&self) -> Option<usize> {
        match self.get_selected_session_panel_item() {
            Some(SessionPanelItem::ProjectHeader { project_index }) => Some(*project_index),
            _ => None,
        }
    }

    /// Get the effective selected project index from either:
    /// - Projects panel selection, OR
    /// - A ProjectHeader selected in the sessions panel
    pub fn get_effective_project_index(&self) -> Option<usize> {
        self.get_selected_project_index()
            .or_else(|| self.get_selected_session_panel_project_index())
    }

    pub fn selected_project_has_sessions(&self) -> bool {
        if let Some(project_idx) = self.get_effective_project_index() {
            if let Some(project) = self.projects.get(project_idx) {
                return !project.active_sessions.is_empty();
            }
        }
        false
    }

    pub async fn pause_selected(&mut self) {
        if let Some(idx) = self.get_selected_session_index() {
            if let Some(session) = self.sessions.get(idx) {
                match self.mutagen_client.pause_session(&session.identifier).await {
                    Ok(_) => {
                        self.status_message = Some(StatusMessage::info(format!(
                            "Paused session: {}",
                            session.name
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

    pub async fn resume_selected(&mut self) {
        if let Some(idx) = self.get_selected_session_index() {
            if let Some(session) = self.sessions.get(idx) {
                match self
                    .mutagen_client
                    .resume_session(&session.identifier)
                    .await
                {
                    Ok(_) => {
                        self.status_message = Some(StatusMessage::info(format!(
                            "Resumed session: {}",
                            session.name
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

    pub async fn terminate_selected(&mut self) {
        if let Some(idx) = self.get_selected_session_index() {
            if let Some(session) = self.sessions.get(idx) {
                match self
                    .mutagen_client
                    .terminate_session(&session.identifier)
                    .await
                {
                    Ok(_) => {
                        self.status_message = Some(StatusMessage::info(format!(
                            "Terminated session: {}",
                            session.name
                        )));
                        self.sessions.remove(idx);
                        // Rebuild session panel items and update selection manager
                        self.session_panel_items =
                            build_session_panel_items(&self.sessions, &self.projects);
                        self.selection
                            .update_sizes(self.projects.len(), self.session_panel_items.len());
                    }
                    Err(e) => {
                        self.status_message =
                            Some(StatusMessage::error(format!("Failed to terminate: {}", e)));
                    }
                }
            }
        }
    }

    pub async fn flush_selected(&mut self) {
        if let Some(idx) = self.get_selected_session_index() {
            if let Some(session) = self.sessions.get(idx) {
                match self.mutagen_client.flush_session(&session.identifier).await {
                    Ok(_) => {
                        self.status_message = Some(StatusMessage::info(format!(
                            "Flushed session: {}",
                            session.name
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

    pub async fn start_selected_project(&mut self) {
        if let Some(project_idx) = self.get_effective_project_index() {
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

    pub async fn toggle_selected_project(&mut self) {
        if let Some(project_idx) = self.get_effective_project_index() {
            if let Some(project) = self.projects.get(project_idx) {
                // Use project.is_active() which checks if there are active sessions
                let is_running = project.is_active();

                if is_running {
                    // Project is running → terminate it
                    match self
                        .mutagen_client
                        .terminate_project(&project.file.path)
                        .await
                    {
                        Ok(_) => {
                            self.status_message = Some(StatusMessage::info(format!(
                                "Terminated project: {}",
                                project.file.display_name()
                            )));
                        }
                        Err(e) => {
                            self.status_message = Some(StatusMessage::error(format!(
                                "Failed to terminate project: {}",
                                e
                            )));
                        }
                    }
                } else {
                    // Project not running → start it
                    // First terminate any lingering sessions that might interfere
                    for session in &project.active_sessions {
                        let _ = self
                            .mutagen_client
                            .terminate_session(&session.identifier)
                            .await;
                    }
                    self.start_selected_project().await;
                }
            }
        }
    }

    pub async fn push_selected_project(&mut self) {
        if let Some(project_idx) = self.get_effective_project_index() {
            if let Some(project) = self.projects.get(project_idx) {
                // Terminate all active sessions for this project before creating push sessions
                for session in &project.active_sessions {
                    let _ = self
                        .mutagen_client
                        .terminate_session(&session.identifier)
                        .await;
                }

                if project.file.sessions.is_empty() {
                    self.status_message =
                        Some(StatusMessage::error("No sessions defined in project file"));
                    return;
                }

                // Create push sessions for ALL sessions in the project
                let mut created_count = 0;
                let mut errors: Vec<(String, String)> = Vec::new();

                // Get defaults for ignore patterns
                let defaults_value = project
                    .file
                    .defaults
                    .as_ref()
                    .and_then(|defaults| serde_yaml::to_value(defaults).ok());

                for (session_name, session_def) in &project.file.sessions {
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
                    self.status_message = Some(StatusMessage::info(format!(
                        "Created {} push session(s)",
                        created_count
                    )));
                } else if created_count > 0 && !errors.is_empty() {
                    self.status_message = Some(StatusMessage::warning(format!(
                        "Created {} push session(s), {} failed",
                        created_count,
                        errors.len()
                    )));
                } else {
                    // All failed
                    let error_msg = if errors.len() == 1 {
                        format!("Failed to create push session: {}", errors[0].1)
                    } else {
                        format!("Failed to create {} push sessions", errors.len())
                    };
                    self.status_message = Some(StatusMessage::error(error_msg));
                }
            } else {
                self.status_message = Some(StatusMessage::error("Failed to get selected project"));
            }
        } else {
            self.status_message = Some(StatusMessage::error("No project selected"));
        }
    }

    pub async fn pause_selected_project(&mut self) {
        if let Some(project_idx) = self.get_effective_project_index() {
            if let Some(project) = self.projects.get(project_idx) {
                if project.active_sessions.is_empty() {
                    self.status_message = Some(StatusMessage::info("No active sessions to pause"));
                    return;
                }

                // Pause ALL active sessions individually
                let mut paused_count = 0;
                let mut errors: Vec<String> = Vec::new();

                for session in &project.active_sessions {
                    match self.mutagen_client.pause_session(&session.identifier).await {
                        Ok(_) => paused_count += 1,
                        Err(e) => errors.push(format!("{}: {}", session.name, e)),
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
    }

    pub async fn resume_selected_project(&mut self) {
        if let Some(project_idx) = self.get_effective_project_index() {
            if let Some(project) = self.projects.get(project_idx) {
                if project.active_sessions.is_empty() {
                    self.status_message = Some(StatusMessage::info("No active sessions to resume"));
                    return;
                }

                // Resume ALL active sessions individually
                let mut resumed_count = 0;
                let mut errors: Vec<String> = Vec::new();

                for session in &project.active_sessions {
                    match self
                        .mutagen_client
                        .resume_session(&session.identifier)
                        .await
                    {
                        Ok(_) => resumed_count += 1,
                        Err(e) => errors.push(format!("{}: {}", session.name, e)),
                    }
                }

                // Set status message based on results
                if resumed_count > 0 && errors.is_empty() {
                    self.status_message = Some(StatusMessage::info(format!(
                        "Resumed {} session(s)",
                        resumed_count
                    )));
                } else if resumed_count > 0 && !errors.is_empty() {
                    self.status_message = Some(StatusMessage::warning(format!(
                        "Resumed {} session(s), {} failed",
                        resumed_count,
                        errors.len()
                    )));
                } else {
                    self.status_message = Some(StatusMessage::error(format!(
                        "Failed to resume {} session(s)",
                        errors.len()
                    )));
                }
            }
        }
    }

    pub async fn toggle_pause_selected(&mut self) {
        if let Some(idx) = self.get_selected_session_index() {
            // Individual session selected - toggle its pause state
            if let Some(session) = self.sessions.get(idx) {
                if session.paused {
                    self.resume_selected().await;
                } else {
                    self.pause_selected().await;
                }
            }
        } else if let Some(project_idx) = self.get_effective_project_index() {
            // Project selected (from either panel) - toggle pause for all its sessions
            if let Some(project) = self.projects.get(project_idx) {
                // Only pause/resume if project has active sessions
                if project.active_sessions.is_empty() {
                    self.status_message = Some(StatusMessage::info(
                        "Project has no active sessions. Use 's' to start.",
                    ));
                    return;
                }

                // Check if any session is running (not paused)
                let has_running = project.active_sessions.iter().any(|s| !s.paused);
                if has_running {
                    self.pause_selected_project().await;
                } else {
                    self.resume_selected_project().await;
                }
            }
        }
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
        if let Some(idx) = self.get_selected_session_index() {
            if let Some(session) = self.sessions.get(idx) {
                if session.has_conflicts() {
                    self.viewing_conflicts = !self.viewing_conflicts;
                    if self.viewing_conflicts {
                        self.status_message = Some(StatusMessage::info(format!(
                            "Viewing conflicts for: {}",
                            session.name
                        )));
                    } else {
                        self.status_message = Some(StatusMessage::info("Closed conflict view"));
                    }
                } else {
                    self.status_message =
                        Some(StatusMessage::error("No conflicts in selected session"));
                }
            }
        } else {
            self.status_message = Some(StatusMessage::error("Select a session to view conflicts"));
        }
    }

    pub fn get_selected_session_conflicts(&self) -> Option<&Vec<crate::mutagen::Conflict>> {
        if let Some(idx) = self.get_selected_session_index() {
            if let Some(session) = self.sessions.get(idx) {
                return Some(&session.conflicts);
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
