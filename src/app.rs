use crate::mutagen::{MutagenClient, SyncSession};
use crate::project::{correlate_projects_with_sessions, discover_project_files, Project};
use crate::theme::{detect_theme, ColorScheme};
use anyhow::Result;
use chrono::{DateTime, Local};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionDisplayMode {
    ShowPaths,
    ShowLastRefresh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusArea {
    Projects,
    Sessions,
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
        if project.active_sessions.iter().any(|s| s.name == session.name) {
            let project_name = project.file.display_name();
            return format!("{} > {}", project_name, session.name);
        }
    }
    session.name.clone()
}

#[derive(Debug, Clone)]
pub struct BlockingOperation {
    pub message: String,
}

pub struct App {
    pub sessions: Vec<SyncSession>,
    pub projects: Vec<Project>,
    pub selected_index: usize,
    pub should_quit: bool,
    pub status_message: Option<StatusMessage>,
    pub mutagen_client: MutagenClient,
    pub color_scheme: ColorScheme,
    pub last_refresh: Option<DateTime<Local>>,
    pub project_dir: Option<PathBuf>,
    pub session_display_mode: SessionDisplayMode,
    pub viewing_conflicts: bool,
    pub has_refresh_error: bool, // Track if last refresh failed to prevent error loops
    pub focus_area: FocusArea,
    pub last_project_index: Option<usize>,
    pub last_session_index: Option<usize>,
    pub blocking_op: Option<BlockingOperation>,
}

impl App {
    pub fn new(project_dir: Option<PathBuf>) -> Self {
        Self {
            sessions: Vec::new(),
            projects: Vec::new(),
            selected_index: 0,
            should_quit: false,
            status_message: None,
            mutagen_client: MutagenClient::new(),
            color_scheme: detect_theme(),
            last_refresh: None,
            project_dir,
            session_display_mode: SessionDisplayMode::ShowLastRefresh,
            viewing_conflicts: false,
            has_refresh_error: false,
            focus_area: FocusArea::Projects,
            last_project_index: None,
            last_session_index: None,
            blocking_op: None,
        }
    }

    pub async fn refresh_sessions(&mut self) -> Result<()> {
        match self.mutagen_client.list_sessions() {
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

                match discover_project_files(self.project_dir.as_deref()) {
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

                let total_items = self.sessions.len() + self.projects.len();
                if self.selected_index >= total_items && total_items > 0 {
                    self.selected_index = total_items - 1;
                }

                // Update focus area based on current selection
                if self.selected_index < self.projects.len() {
                    self.focus_area = FocusArea::Projects;
                } else if !self.sessions.is_empty() {
                    self.focus_area = FocusArea::Sessions;
                }

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
        let total_items = self.sessions.len() + self.projects.len();
        if total_items > 0 {
            self.selected_index = (self.selected_index + 1) % total_items;
        }
    }

    pub fn select_previous(&mut self) {
        let total_items = self.sessions.len() + self.projects.len();
        if total_items > 0 {
            if self.selected_index == 0 {
                self.selected_index = total_items - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }

    pub fn toggle_focus_area(&mut self) {
        // Save current position in current area
        if self.get_selected_project_index().is_some() {
            self.last_project_index = Some(self.selected_index);
        } else if self.get_selected_session_index().is_some() {
            self.last_session_index = self.get_selected_session_index();
        }

        // Toggle focus area
        self.focus_area = match self.focus_area {
            FocusArea::Projects => FocusArea::Sessions,
            FocusArea::Sessions => FocusArea::Projects,
        };

        // Determine new selection index
        match self.focus_area {
            FocusArea::Projects => {
                if self.projects.is_empty() {
                    // No projects, stay in sessions
                    self.focus_area = FocusArea::Sessions;
                    return;
                }

                // Try to restore last position
                if let Some(last_idx) = self.last_project_index {
                    if last_idx < self.projects.len() {
                        self.selected_index = last_idx;
                        return;
                    }
                }

                // Try to find related project for current session
                if let Some(session_idx) = self.last_session_index {
                    if let Some(session) = self.sessions.get(session_idx) {
                        // Find project that has this session
                        for (proj_idx, project) in self.projects.iter().enumerate() {
                            if project
                                .active_sessions
                                .iter()
                                .any(|s| s.name == session.name)
                            {
                                self.selected_index = proj_idx;
                                return;
                            }
                        }
                    }
                }

                // Default to first project
                self.selected_index = 0;
            }
            FocusArea::Sessions => {
                if self.sessions.is_empty() {
                    // No sessions, stay in projects
                    self.focus_area = FocusArea::Projects;
                    return;
                }

                // Try to restore last position
                if let Some(last_idx) = self.last_session_index {
                    if last_idx < self.sessions.len() {
                        self.selected_index = self.projects.len() + last_idx;
                        return;
                    }
                }

                // Try to find related session for current project
                if let Some(proj_idx) = self.last_project_index {
                    if let Some(project) = self.projects.get(proj_idx) {
                        if let Some(active_session) = project.active_sessions.first() {
                            // Find this session in the sessions list
                            for (sess_idx, session) in self.sessions.iter().enumerate() {
                                if session.name == active_session.name {
                                    self.selected_index = self.projects.len() + sess_idx;
                                    return;
                                }
                            }
                        }
                    }
                }

                // Default to first session
                self.selected_index = self.projects.len();
            }
        }
    }

    pub fn get_selected_project_index(&self) -> Option<usize> {
        if self.selected_index < self.projects.len() {
            Some(self.selected_index)
        } else {
            None
        }
    }

    pub fn get_selected_session_index(&self) -> Option<usize> {
        if self.selected_index >= self.projects.len() {
            Some(self.selected_index - self.projects.len())
        } else {
            None
        }
    }

    pub fn selected_project_has_sessions(&self) -> bool {
        if let Some(project_idx) = self.get_selected_project_index() {
            if let Some(project) = self.projects.get(project_idx) {
                return !project.active_sessions.is_empty();
            }
        }
        false
    }

    pub fn pause_selected(&mut self) {
        if let Some(idx) = self.get_selected_session_index() {
            if let Some(session) = self.sessions.get(idx) {
                match self.mutagen_client.pause_session(&session.identifier) {
                    Ok(_) => {
                        self.status_message = Some(StatusMessage::info(format!("Paused session: {}", session.name)));
                    }
                    Err(e) => {
                        self.status_message = Some(StatusMessage::error(format!("Failed to pause: {}", e)));
                    }
                }
            }
        }
    }

    pub fn resume_selected(&mut self) {
        if let Some(idx) = self.get_selected_session_index() {
            if let Some(session) = self.sessions.get(idx) {
                match self.mutagen_client.resume_session(&session.identifier) {
                    Ok(_) => {
                        self.status_message = Some(StatusMessage::info(format!("Resumed session: {}", session.name)));
                    }
                    Err(e) => {
                        self.status_message = Some(StatusMessage::error(format!("Failed to resume: {}", e)));
                    }
                }
            }
        }
    }

    pub fn terminate_selected(&mut self) {
        if let Some(idx) = self.get_selected_session_index() {
            if let Some(session) = self.sessions.get(idx) {
                match self.mutagen_client.terminate_session(&session.identifier) {
                    Ok(_) => {
                        self.status_message = Some(StatusMessage::info(format!("Terminated session: {}", session.name)));
                        self.sessions.remove(idx);
                        let total_items = self.sessions.len() + self.projects.len();
                        if self.selected_index >= total_items && total_items > 0 {
                            self.selected_index = total_items - 1;
                        }
                    }
                    Err(e) => {
                        self.status_message = Some(StatusMessage::error(format!("Failed to terminate: {}", e)));
                    }
                }
            }
        }
    }

    pub fn flush_selected(&mut self) {
        if let Some(idx) = self.get_selected_session_index() {
            if let Some(session) = self.sessions.get(idx) {
                match self.mutagen_client.flush_session(&session.identifier) {
                    Ok(_) => {
                        self.status_message = Some(StatusMessage::info(format!("Flushed session: {}", session.name)));
                    }
                    Err(e) => {
                        self.status_message = Some(StatusMessage::error(format!("Failed to flush: {}", e)));
                    }
                }
            }
        }
    }

    pub fn start_selected_project(&mut self) {
        if let Some(project_idx) = self.get_selected_project_index() {
            if let Some(project) = self.projects.get(project_idx) {
                match self.mutagen_client.start_project(&project.file.path) {
                    Ok(_) => {
                        self.status_message =
                            Some(StatusMessage::info(format!("Started project: {}", project.file.display_name())));
                    }
                    Err(e) => {
                        self.status_message = Some(StatusMessage::error(format!("Failed to start project: {}", e)));
                    }
                }
            }
        }
    }

    pub fn toggle_selected_project(&mut self) {
        if let Some(project_idx) = self.get_selected_project_index() {
            if let Some(project) = self.projects.get(project_idx) {
                // Check actual project state, not just session existence
                let is_running = self.mutagen_client.is_project_running(&project.file.path);

                if is_running {
                    // Project is running → terminate it
                    match self.mutagen_client.terminate_project(&project.file.path) {
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
                        let _ = self.mutagen_client.terminate_session(&session.identifier);
                    }
                    self.start_selected_project();
                }
            }
        }
    }

    pub fn push_selected_project(&mut self) {
        if let Some(project_idx) = self.get_selected_project_index() {
            if let Some(project) = self.projects.get(project_idx) {
                // Terminate all active sessions for this project before creating push sessions
                for session in &project.active_sessions {
                    let _ = self.mutagen_client.terminate_session(&session.identifier);
                }

                if project.file.sessions.is_empty() {
                    self.status_message = Some(StatusMessage::error("No sessions defined in project file"));
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
                    if let Err(e) = self.mutagen_client.ensure_endpoint_directory_exists(&session_def.alpha) {
                        errors.push((session_name.clone(), format!("Failed to create alpha directory: {}", e)));
                        continue;
                    }
                    if let Err(e) = self.mutagen_client.ensure_endpoint_directory_exists(&session_def.beta) {
                        errors.push((session_name.clone(), format!("Failed to create beta directory: {}", e)));
                        continue;
                    }

                    match self.mutagen_client.create_push_session(
                        &push_name,
                        &session_def.alpha,
                        &session_def.beta,
                        ignore.as_deref(),
                    ) {
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

    pub fn pause_selected_project(&mut self) {
        if let Some(project_idx) = self.get_selected_project_index() {
            if let Some(project) = self.projects.get(project_idx) {
                if project.active_sessions.is_empty() {
                    self.status_message = Some(StatusMessage::info("No active sessions to pause"));
                    return;
                }

                // Pause ALL active sessions individually
                let mut paused_count = 0;
                let mut errors: Vec<String> = Vec::new();

                for session in &project.active_sessions {
                    match self.mutagen_client.pause_session(&session.identifier) {
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

    pub fn resume_selected_project(&mut self) {
        if let Some(project_idx) = self.get_selected_project_index() {
            if let Some(project) = self.projects.get(project_idx) {
                if project.active_sessions.is_empty() {
                    self.status_message = Some(StatusMessage::info("No active sessions to resume"));
                    return;
                }

                // Resume ALL active sessions individually
                let mut resumed_count = 0;
                let mut errors: Vec<String> = Vec::new();

                for session in &project.active_sessions {
                    match self.mutagen_client.resume_session(&session.identifier) {
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

    pub fn toggle_pause_selected(&mut self) {
        if let Some(idx) = self.get_selected_session_index() {
            if let Some(session) = self.sessions.get(idx) {
                if session.paused {
                    self.resume_selected();
                } else {
                    self.pause_selected();
                }
            }
        } else if let Some(project_idx) = self.get_selected_project_index() {
            if let Some(project) = self.projects.get(project_idx) {
                // Only pause/resume if project has active sessions
                if project.active_sessions.is_empty() {
                    self.status_message =
                        Some(StatusMessage::info("Project has no active sessions. Use 's' to start."));
                    return;
                }

                // Check if any session is running (not paused)
                let has_running = project.active_sessions.iter().any(|s| !s.paused);
                if has_running {
                    self.pause_selected_project();
                } else {
                    self.resume_selected_project();
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
        self.status_message = Some(StatusMessage::info(format!("Display mode: {}",
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
                        self.status_message =
                            Some(StatusMessage::info(format!("Viewing conflicts for: {}", session.name)));
                    } else {
                        self.status_message = Some(StatusMessage::info("Closed conflict view"));
                    }
                } else {
                    self.status_message = Some(StatusMessage::error("No conflicts in selected session"));
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
        const AUTO_REFRESH_INTERVAL_SECS: i64 = 3;

        // Don't auto-refresh if the last refresh resulted in an error
        // User must manually retry with 'r' to clear the error state
        if self.has_refresh_error {
            return false;
        }

        match self.last_refresh {
            Some(last) => {
                let elapsed = Local::now().signed_duration_since(last);
                elapsed.num_seconds() >= AUTO_REFRESH_INTERVAL_SECS
            }
            None => true,
        }
    }
}
