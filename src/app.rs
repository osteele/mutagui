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

pub struct App {
    pub sessions: Vec<SyncSession>,
    pub projects: Vec<Project>,
    pub selected_index: usize,
    pub should_quit: bool,
    pub status_message: Option<String>,
    pub mutagen_client: MutagenClient,
    pub color_scheme: ColorScheme,
    pub last_refresh: Option<DateTime<Local>>,
    pub project_dir: Option<PathBuf>,
    pub session_display_mode: SessionDisplayMode,
    pub viewing_conflicts: bool,
    pub has_refresh_error: bool, // Track if last refresh failed to prevent error loops
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
        }
    }

    pub async fn refresh_sessions(&mut self) -> Result<()> {
        match self.mutagen_client.list_sessions() {
            Ok(sessions) => {
                let now = Local::now();

                // Track when successfulCycles changes to detect actual sync activity
                let new_sessions: Vec<_> = sessions
                    .into_iter()
                    .map(|mut new_session| {
                        // Find the previous version of this session
                        if let Some(old_session) = self
                            .sessions
                            .iter()
                            .find(|s| s.identifier == new_session.identifier)
                        {
                            // If successfulCycles increased, update last_sync_time
                            if new_session.successful_cycles > old_session.successful_cycles {
                                new_session.last_sync_time = Some(now);
                            } else {
                                // Keep the previous last_sync_time
                                new_session.last_sync_time = old_session.last_sync_time;
                            }
                        } else {
                            // New session - set last_sync_time if it has already synced
                            if new_session.successful_cycles > 0 {
                                new_session.last_sync_time = Some(now);
                            }
                        }
                        new_session
                    })
                    .collect();

                self.sessions = new_sessions;

                match discover_project_files(self.project_dir.as_deref()) {
                    Ok(project_files) => {
                        self.projects =
                            correlate_projects_with_sessions(project_files, &self.sessions);
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

                self.last_refresh = Some(Local::now());
                self.status_message = Some("Sessions refreshed".to_string());
                self.has_refresh_error = false; // Clear error flag on success
                Ok(())
            }
            Err(e) => {
                // Display error to user but don't crash the UI
                // Transient CLI failures (missing binary, timeouts) should not tear down the terminal
                self.status_message = Some(format!("Error: {} (press 'r' to retry)", e));
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

    fn get_selected_project_index(&self) -> Option<usize> {
        if self.selected_index < self.projects.len() {
            Some(self.selected_index)
        } else {
            None
        }
    }

    fn get_selected_session_index(&self) -> Option<usize> {
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
                        self.status_message = Some(format!("Paused session: {}", session.name));
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Failed to pause: {}", e));
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
                        self.status_message = Some(format!("Resumed session: {}", session.name));
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Failed to resume: {}", e));
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
                        self.status_message = Some(format!("Terminated session: {}", session.name));
                        self.sessions.remove(idx);
                        let total_items = self.sessions.len() + self.projects.len();
                        if self.selected_index >= total_items && total_items > 0 {
                            self.selected_index = total_items - 1;
                        }
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Failed to terminate: {}", e));
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
                        self.status_message = Some(format!("Flushed session: {}", session.name));
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Failed to flush: {}", e));
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
                            Some(format!("Started project: {}", project.file.display_name()));
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Failed to start project: {}", e));
                    }
                }
            }
        }
    }

    pub fn stop_selected_project(&mut self) {
        if let Some(project_idx) = self.get_selected_project_index() {
            if let Some(project) = self.projects.get(project_idx) {
                match self.mutagen_client.terminate_project(&project.file.path) {
                    Ok(_) => {
                        self.status_message =
                            Some(format!("Stopped project: {}", project.file.display_name()));
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Failed to stop project: {}", e));
                    }
                }
            }
        }
    }

    pub fn toggle_selected_project(&mut self) {
        if self.selected_project_has_sessions() {
            self.stop_selected_project();
        } else {
            self.start_selected_project();
        }
    }

    pub fn push_selected_project(&mut self) {
        if let Some(project_idx) = self.get_selected_project_index() {
            if let Some(project) = self.projects.get(project_idx) {
                // Determine which session to push - prefer active sessions, then alphabetical order
                let selected_session = if project.file.sessions.len() == 1 {
                    // Only one session - use it
                    project.file.sessions.iter().next()
                } else if !project.active_sessions.is_empty() {
                    // Multiple sessions, but some are active - find the first active one alphabetically
                    let active_names: std::collections::HashSet<_> = project
                        .active_sessions
                        .iter()
                        .map(|s| s.name.as_str())
                        .collect();

                    let mut sorted_sessions: Vec<_> = project.file.sessions.iter().collect();
                    sorted_sessions.sort_by_key(|(name, _)| *name);

                    sorted_sessions
                        .into_iter()
                        .find(|(name, _)| active_names.contains(name.as_str()))
                } else {
                    // Multiple sessions, none active - use first alphabetically
                    let mut sorted_sessions: Vec<_> = project.file.sessions.iter().collect();
                    sorted_sessions.sort_by_key(|(name, _)| *name);
                    sorted_sessions.into_iter().next()
                };

                if let Some((session_name, session_def)) = selected_session {
                    let push_name = format!("{}-push", session_name);

                    // Extract ignore patterns, merging with defaults
                    let defaults_value = project
                        .file
                        .defaults
                        .as_ref()
                        .and_then(|defaults| serde_yaml::to_value(defaults).ok());
                    let ignore_patterns = session_def.get_ignore_patterns(defaults_value.as_ref());
                    let ignore = if ignore_patterns.is_empty() {
                        None
                    } else {
                        Some(ignore_patterns)
                    };

                    match self.mutagen_client.create_push_session(
                        &push_name,
                        &session_def.alpha,
                        &session_def.beta,
                        ignore.as_deref(),
                    ) {
                        Ok(_) => {
                            self.status_message =
                                Some(format!("Created push session: {}", push_name));
                        }
                        Err(e) => {
                            self.status_message =
                                Some(format!("Failed to create push session: {}", e));
                        }
                    }
                } else {
                    self.status_message = Some("No sessions defined in project file".to_string());
                }
            }
        }
    }

    pub fn pause_selected_project(&mut self) {
        if let Some(project_idx) = self.get_selected_project_index() {
            if let Some(project) = self.projects.get(project_idx) {
                match self.mutagen_client.pause_project(&project.file.path) {
                    Ok(_) => {
                        self.status_message =
                            Some(format!("Paused project: {}", project.file.display_name()));
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Failed to pause project: {}", e));
                    }
                }
            }
        }
    }

    pub fn resume_selected_project(&mut self) {
        if let Some(project_idx) = self.get_selected_project_index() {
            if let Some(project) = self.projects.get(project_idx) {
                match self.mutagen_client.resume_project(&project.file.path) {
                    Ok(_) => {
                        self.status_message =
                            Some(format!("Resumed project: {}", project.file.display_name()));
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Failed to resume project: {}", e));
                    }
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
                        Some("Project has no active sessions. Use 's' to start.".to_string());
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
        self.status_message = Some(format!(
            "Display mode: {}",
            match self.session_display_mode {
                SessionDisplayMode::ShowPaths => "Paths",
                SessionDisplayMode::ShowLastRefresh => "Last Sync Time",
            }
        ));
    }

    pub fn toggle_conflict_view(&mut self) {
        if let Some(idx) = self.get_selected_session_index() {
            if let Some(session) = self.sessions.get(idx) {
                if session.has_conflicts() {
                    self.viewing_conflicts = !self.viewing_conflicts;
                    if self.viewing_conflicts {
                        self.status_message =
                            Some(format!("Viewing conflicts for: {}", session.name));
                    } else {
                        self.status_message = Some("Closed conflict view".to_string());
                    }
                } else {
                    self.status_message = Some("No conflicts in selected session".to_string());
                }
            }
        } else {
            self.status_message = Some("Select a session to view conflicts".to_string());
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
