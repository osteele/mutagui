use crate::mutagen::{MutagenClient, SyncSession};
use crate::project::{correlate_projects_with_sessions, discover_project_files, Project};
use crate::theme::{detect_theme, ColorScheme};
use anyhow::Result;
use chrono::{DateTime, Local};
use std::path::PathBuf;

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
        }
    }

    pub async fn refresh_sessions(&mut self) -> Result<()> {
        match self.mutagen_client.list_sessions() {
            Ok(sessions) => {
                self.sessions = sessions.clone();

                match discover_project_files(self.project_dir.as_deref()) {
                    Ok(project_files) => {
                        self.projects = correlate_projects_with_sessions(project_files, &sessions);
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to discover project files: {}", e);
                    }
                }

                let total_items = self.sessions.len() + self.projects.len();
                if self.selected_index >= total_items && total_items > 0 {
                    self.selected_index = total_items - 1;
                }

                self.last_refresh = Some(Local::now());
                self.status_message = Some("Sessions refreshed".to_string());
                Ok(())
            }
            Err(e) => {
                self.status_message = Some(format!("Error: {}", e));
                Err(e)
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

    fn get_selected_session_index(&self) -> Option<usize> {
        if self.selected_index < self.sessions.len() {
            Some(self.selected_index)
        } else {
            None
        }
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

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn should_auto_refresh(&self) -> bool {
        const AUTO_REFRESH_INTERVAL_SECS: i64 = 3;

        match self.last_refresh {
            Some(last) => {
                let elapsed = Local::now().signed_duration_since(last);
                elapsed.num_seconds() >= AUTO_REFRESH_INTERVAL_SECS
            }
            None => true,
        }
    }
}
