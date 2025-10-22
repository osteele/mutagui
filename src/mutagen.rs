use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
    pub protocol: String,
    pub path: String,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub connected: bool,
    #[serde(default)]
    pub scanned: bool,
    #[serde(default)]
    pub directories: Option<u64>,
    #[serde(default)]
    pub files: Option<u64>,
    #[serde(rename = "symbolicLinks", default)]
    pub symbolic_links: Option<u64>,
    #[serde(rename = "totalFileSize", default)]
    pub total_file_size: Option<u64>,
}

impl Endpoint {
    pub fn display_path(&self) -> String {
        if let Some(host) = &self.host {
            format!("{}:{}", host, self.path)
        } else {
            self.path.clone()
        }
    }

    pub fn status_icon(&self) -> &str {
        if !self.connected {
            "⊗"
        } else if !self.scanned {
            "⟳"
        } else {
            "✓"
        }
    }

    pub fn stats_display(&self) -> String {
        match (self.files, self.directories) {
            (Some(f), Some(d)) => format!("{}f/{}d", f, d),
            (Some(f), None) => format!("{}f", f),
            (None, Some(d)) => format!("{}d", d),
            (None, None) => String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSession {
    pub name: String,
    pub identifier: String,
    pub alpha: Endpoint,
    pub beta: Endpoint,
    pub status: String,
    pub paused: bool,
    #[serde(rename = "creationTime")]
    pub creation_time: Option<String>,
    #[serde(rename = "successfulCycles", default)]
    pub successful_cycles: u64,
    #[serde(skip)]
    pub last_sync_time: Option<DateTime<Local>>,
}

impl SyncSession {
    pub fn alpha_display(&self) -> String {
        self.alpha.display_path()
    }

    pub fn beta_display(&self) -> String {
        self.beta.display_path()
    }

    pub fn time_ago_display(&self) -> String {
        match self.last_sync_time {
            Some(sync_time) => {
                let now = Local::now();
                let duration = now.signed_duration_since(sync_time);
                let seconds = duration.num_seconds();

                if seconds < 60 {
                    "just now".to_string()
                } else if seconds < 120 {
                    "1 min ago".to_string()
                } else if seconds < 3600 {
                    let mins = seconds / 60;
                    format!("{} mins ago", mins)
                } else if seconds < 7200 {
                    "1 hour ago".to_string()
                } else if seconds < 86400 {
                    let hours = seconds / 3600;
                    format!("{} hours ago", hours)
                } else if seconds < 172800 {
                    "1 day ago".to_string()
                } else {
                    let days = seconds / 86400;
                    format!("{} days ago", days)
                }
            }
            None => "never".to_string(),
        }
    }
}

pub struct MutagenClient;

impl MutagenClient {
    pub fn new() -> Self {
        Self
    }

    pub fn list_sessions(&self) -> Result<Vec<SyncSession>> {
        let output = Command::new("mutagen")
            .arg("sync")
            .arg("list")
            .arg("--template")
            .arg("{{json .}}")
            .output()
            .context("Failed to execute mutagen sync list")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen sync list failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse the JSON output
        let sessions: Vec<SyncSession> = if stdout.trim().is_empty() {
            Vec::new()
        } else {
            serde_json::from_str(&stdout).context("Failed to parse mutagen output")?
        };

        Ok(sessions)
    }

    pub fn pause_session(&self, identifier: &str) -> Result<()> {
        let output = Command::new("mutagen")
            .arg("sync")
            .arg("pause")
            .arg(identifier)
            .output()
            .context("Failed to execute mutagen sync pause")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen sync pause failed: {}", stderr);
        }

        Ok(())
    }

    pub fn resume_session(&self, identifier: &str) -> Result<()> {
        let output = Command::new("mutagen")
            .arg("sync")
            .arg("resume")
            .arg(identifier)
            .output()
            .context("Failed to execute mutagen sync resume")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen sync resume failed: {}", stderr);
        }

        Ok(())
    }

    pub fn terminate_session(&self, identifier: &str) -> Result<()> {
        let output = Command::new("mutagen")
            .arg("sync")
            .arg("terminate")
            .arg(identifier)
            .output()
            .context("Failed to execute mutagen sync terminate")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen sync terminate failed: {}", stderr);
        }

        Ok(())
    }

    pub fn flush_session(&self, identifier: &str) -> Result<()> {
        let output = Command::new("mutagen")
            .arg("sync")
            .arg("flush")
            .arg(identifier)
            .output()
            .context("Failed to execute mutagen sync flush")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen sync flush failed: {}", stderr);
        }

        Ok(())
    }
}
