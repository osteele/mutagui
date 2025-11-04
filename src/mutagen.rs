use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::{Command, Output};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileState {
    pub kind: String,
    pub digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub path: String,
    pub old: FileState,
    pub new: FileState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    pub root: String,
    #[serde(rename = "alphaChanges", default)]
    pub alpha_changes: Vec<Change>,
    #[serde(rename = "betaChanges", default)]
    pub beta_changes: Vec<Change>,
}

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
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(rename = "creationTime")]
    pub creation_time: Option<String>,
    #[serde(rename = "successfulCycles", default)]
    pub successful_cycles: u64,
    #[serde(default)]
    pub conflicts: Vec<Conflict>,
    #[serde(skip)]
    pub last_sync_time: Option<DateTime<Local>>,
}

impl SyncSession {
    pub fn has_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }

    pub fn conflict_count(&self) -> usize {
        self.conflicts.len()
    }

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

fn execute_with_timeout(mut cmd: Command, timeout_secs: u64) -> Result<Output> {
    // Cross-platform timeout implementation using thread-based approach
    // This avoids the Unix-only non-blocking pipe implementation that would
    // hang on Windows where pipes remain blocking.
    use std::sync::mpsc;
    use std::thread;

    let (tx, rx) = mpsc::channel();

    // Spawn command execution in a separate thread
    thread::spawn(move || {
        let result = cmd
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output();
        let _ = tx.send(result);
    });

    // Wait for completion with timeout
    let timeout_duration = Duration::from_secs(timeout_secs);
    match rx.recv_timeout(timeout_duration) {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => Err(anyhow!("Failed to execute mutagen command: {}", e)),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // Note: We can't easily kill the process from here since we don't have
            // a handle to it. The spawned thread will complete eventually.
            // In practice, mutagen commands should complete or fail quickly.
            anyhow::bail!(
                "Command timed out after {} seconds (may be waiting for input or hanging)",
                timeout_secs
            )
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            anyhow::bail!("Command execution thread terminated unexpectedly")
        }
    }
}

pub struct MutagenClient;

impl MutagenClient {
    pub fn new() -> Self {
        Self
    }

    pub fn list_sessions(&self) -> Result<Vec<SyncSession>> {
        let mut cmd = Command::new("mutagen");
        cmd.arg("sync")
            .arg("list")
            .arg("--template")
            .arg("{{json .}}");

        let output = execute_with_timeout(cmd, 5)?;

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
        let mut cmd = Command::new("mutagen");
        cmd.arg("sync").arg("pause").arg(identifier);

        let output = execute_with_timeout(cmd, 5)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen sync pause failed: {}", stderr);
        }

        Ok(())
    }

    pub fn resume_session(&self, identifier: &str) -> Result<()> {
        let mut cmd = Command::new("mutagen");
        cmd.arg("sync").arg("resume").arg(identifier);

        let output = execute_with_timeout(cmd, 5)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen sync resume failed: {}", stderr);
        }

        Ok(())
    }

    pub fn terminate_session(&self, identifier: &str) -> Result<()> {
        let mut cmd = Command::new("mutagen");
        cmd.arg("sync").arg("terminate").arg(identifier);

        let output = execute_with_timeout(cmd, 5)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen sync terminate failed: {}", stderr);
        }

        Ok(())
    }

    pub fn flush_session(&self, identifier: &str) -> Result<()> {
        let mut cmd = Command::new("mutagen");
        cmd.arg("sync").arg("flush").arg(identifier);

        let output = execute_with_timeout(cmd, 5)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen sync flush failed: {}", stderr);
        }

        Ok(())
    }

    pub fn start_project(&self, project_file: &Path) -> Result<()> {
        let mut cmd = Command::new("mutagen");
        cmd.arg("project").arg("start").arg("-f").arg(project_file);

        let output = execute_with_timeout(cmd, 10)?; // Project operations might take longer

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen project start failed: {}", stderr);
        }

        Ok(())
    }

    pub fn terminate_project(&self, project_file: &Path) -> Result<()> {
        let mut cmd = Command::new("mutagen");
        cmd.arg("project")
            .arg("terminate")
            .arg("-f")
            .arg(project_file);

        let output = execute_with_timeout(cmd, 10)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen project terminate failed: {}", stderr);
        }

        Ok(())
    }

    pub fn pause_project(&self, project_file: &Path) -> Result<()> {
        let mut cmd = Command::new("mutagen");
        cmd.arg("project").arg("pause").arg("-f").arg(project_file);

        let output = execute_with_timeout(cmd, 10)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen project pause failed: {}", stderr);
        }

        Ok(())
    }

    pub fn resume_project(&self, project_file: &Path) -> Result<()> {
        let mut cmd = Command::new("mutagen");
        cmd.arg("project").arg("resume").arg("-f").arg(project_file);

        let output = execute_with_timeout(cmd, 10)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen project resume failed: {}", stderr);
        }

        Ok(())
    }

    pub fn create_push_session(
        &self,
        name: &str,
        alpha: &str,
        beta: &str,
        ignore: Option<&[String]>,
    ) -> Result<()> {
        let mut cmd = Command::new("mutagen");
        cmd.arg("sync")
            .arg("create")
            .arg(alpha)
            .arg(beta)
            .arg("-m")
            .arg("one-way-replica")
            .arg("-n")
            .arg(name);

        if let Some(ignore_patterns) = ignore {
            for pattern in ignore_patterns {
                cmd.arg("--ignore").arg(pattern);
            }
        }

        let output = execute_with_timeout(cmd, 15)?; // Create might take longer

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen sync create failed: {}", stderr);
        }

        Ok(())
    }
}
