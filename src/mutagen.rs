use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::{Command, Output};
use std::time::Duration;

#[derive(Debug, Clone, Default)]
pub enum SyncTime {
    Never, // Brand new session, no syncs yet
    #[default]
    Unknown, // Pre-existing session, sync history unknown
    At(DateTime<Local>), // Observed sync at this time
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileState {
    pub kind: String,
    #[serde(default)]
    pub digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub path: String,
    pub old: Option<FileState>,
    pub new: Option<FileState>,
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
    #[serde(rename = "successfulCycles")]
    pub successful_cycles: Option<u64>,
    #[serde(default)]
    pub conflicts: Vec<Conflict>,
    #[serde(skip, default)]
    pub sync_time: SyncTime,
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
        match &self.sync_time {
            SyncTime::At(sync_time) => {
                let now = Local::now();
                let duration = now.signed_duration_since(*sync_time);
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
            SyncTime::Unknown => "unknown".to_string(),
            SyncTime::Never => "never".to_string(),
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
        // Note: The mutagen template '{{json .}}' outputs a JSON array: [{session1}, {session2}, ...]
        // This is NOT JSONL format (one object per line). The entire output is a single JSON array.
        // See: https://mutagen.io/documentation/introduction/templates
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

    #[allow(dead_code)] // Prefer individual session termination for broader compatibility
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

    #[allow(dead_code)] // May be used in future for project-level pause operations
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

    #[allow(dead_code)] // May be used in future for project-level resume operations
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

    /// Ensures a directory exists on an endpoint (local or remote).
    /// For remote endpoints (format: `host:path`), uses SSH to create the directory.
    /// For local paths, uses std::fs::create_dir_all.
    pub fn ensure_endpoint_directory_exists(&self, endpoint: &str) -> Result<()> {
        // Check if this is a Windows drive letter path (e.g., C:\, D:\)
        let is_windows_drive = endpoint.len() >= 2
            && endpoint.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && endpoint.chars().nth(1) == Some(':');

        // Remote paths (host:path format) - but exclude Windows drive letters
        if endpoint.contains(':') && !is_windows_drive {
            // Parse as remote: host:path
            if let Some((host, path)) = endpoint.split_once(':') {
                // Use SSH to create directory on remote host
                let cmd = Command::new("ssh")
                    .arg(host)
                    .arg(format!("mkdir -p {}", path))
                    .output();

                match cmd {
                    Ok(output) if output.status.success() => Ok(()),
                    Ok(output) => {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        anyhow::bail!("Failed to create remote directory {}: {}", endpoint, stderr)
                    }
                    Err(e) => anyhow::bail!("Failed to run ssh to create directory {}: {}", endpoint, e),
                }
            } else {
                anyhow::bail!("Invalid remote endpoint format: {}", endpoint)
            }
        } else {
            // Local path - use std::fs
            std::fs::create_dir_all(endpoint)
                .with_context(|| format!("Failed to create local directory {}", endpoint))
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_conflict_with_digest() {
        // Test case: Real file conflict with digest field present
        let json = r#"{
            "root": "test.txt",
            "alphaChanges": [{
                "path": "test.txt",
                "old": null,
                "new": {
                    "kind": "file",
                    "digest": "fee7d500607ccbc550c97bd094ddfd2d5f170d0b"
                }
            }],
            "betaChanges": [{
                "path": "test.txt",
                "old": null,
                "new": {
                    "kind": "file",
                    "digest": "2dec8677cc6572dd75622e977dcf0e929238f7c0"
                }
            }]
        }"#;

        let conflict: Conflict =
            serde_json::from_str(json).expect("Failed to parse conflict with digest");

        assert_eq!(conflict.root, "test.txt");
        assert_eq!(conflict.alpha_changes.len(), 1);
        assert_eq!(conflict.beta_changes.len(), 1);

        let alpha_change = &conflict.alpha_changes[0];
        assert_eq!(alpha_change.path, "test.txt");
        assert!(alpha_change.old.is_none());

        let alpha_new = alpha_change.new.as_ref().unwrap();
        assert_eq!(alpha_new.kind, "file");
        assert_eq!(
            alpha_new.digest.as_ref().unwrap(),
            "fee7d500607ccbc550c97bd094ddfd2d5f170d0b"
        );
    }

    #[test]
    fn test_parse_conflict_without_digest() {
        // Test case: Directory or untracked file without digest
        let json = r#"{
            "root": "config",
            "alphaChanges": [{
                "path": "config",
                "old": null,
                "new": null
            }],
            "betaChanges": [{
                "path": "config/mutagen/mutagen-cool30.yml.lock",
                "old": null,
                "new": {
                    "kind": "untracked"
                }
            }]
        }"#;

        let conflict: Conflict =
            serde_json::from_str(json).expect("Failed to parse conflict without digest");

        assert_eq!(conflict.root, "config");
        assert_eq!(conflict.alpha_changes.len(), 1);
        assert_eq!(conflict.beta_changes.len(), 1);

        let beta_change = &conflict.beta_changes[0];
        assert_eq!(beta_change.path, "config/mutagen/mutagen-cool30.yml.lock");
        assert!(beta_change.old.is_none());

        let beta_new = beta_change.new.as_ref().unwrap();
        assert_eq!(beta_new.kind, "untracked");
        assert!(beta_new.digest.is_none()); // No digest for untracked files
    }
}
