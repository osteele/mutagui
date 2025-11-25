use crate::command::{CommandRunner, SystemCommandRunner};
use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::path::Path;

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

/// Type alias for the production MutagenClient.
#[allow(dead_code)] // Available for future use and testing
pub type ProductionMutagenClient = MutagenClient<SystemCommandRunner>;

/// Client for interacting with the Mutagen CLI.
///
/// Generic over `CommandRunner` to allow dependency injection of mock
/// implementations for testing.
pub struct MutagenClient<R: CommandRunner = SystemCommandRunner> {
    runner: R,
}

impl MutagenClient<SystemCommandRunner> {
    /// Create a new MutagenClient with the default system command runner.
    pub fn new() -> Self {
        Self {
            runner: SystemCommandRunner::new(),
        }
    }
}

impl Default for MutagenClient<SystemCommandRunner> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: CommandRunner> MutagenClient<R> {
    /// Create a new MutagenClient with a custom command runner.
    /// Primarily used for testing with mock runners.
    #[cfg(test)]
    pub fn with_runner(runner: R) -> Self {
        Self { runner }
    }

    pub fn list_sessions(&self) -> Result<Vec<SyncSession>> {
        let output =
            self.runner
                .run("mutagen", &["sync", "list", "--template", "{{json .}}"], 5)?;

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
        let output = self
            .runner
            .run("mutagen", &["sync", "pause", identifier], 5)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen sync pause failed: {}", stderr);
        }

        Ok(())
    }

    pub fn resume_session(&self, identifier: &str) -> Result<()> {
        let output = self
            .runner
            .run("mutagen", &["sync", "resume", identifier], 5)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen sync resume failed: {}", stderr);
        }

        Ok(())
    }

    pub fn terminate_session(&self, identifier: &str) -> Result<()> {
        let output = self
            .runner
            .run("mutagen", &["sync", "terminate", identifier], 5)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen sync terminate failed: {}", stderr);
        }

        Ok(())
    }

    pub fn flush_session(&self, identifier: &str) -> Result<()> {
        let output = self
            .runner
            .run("mutagen", &["sync", "flush", identifier], 5)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen sync flush failed: {}", stderr);
        }

        Ok(())
    }

    pub fn start_project(&self, project_file: &Path) -> Result<()> {
        let path_str = project_file.to_string_lossy();
        let output = self
            .runner
            .run("mutagen", &["project", "start", "-f", &path_str], 10)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen project start failed: {}", stderr);
        }

        Ok(())
    }

    pub fn terminate_project(&self, project_file: &Path) -> Result<()> {
        let path_str = project_file.to_string_lossy();
        let output = self
            .runner
            .run("mutagen", &["project", "terminate", "-f", &path_str], 10)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen project terminate failed: {}", stderr);
        }

        Ok(())
    }

    #[allow(dead_code)] // May be used in future for project-level pause operations
    pub fn pause_project(&self, project_file: &Path) -> Result<()> {
        let path_str = project_file.to_string_lossy();
        let output = self
            .runner
            .run("mutagen", &["project", "pause", "-f", &path_str], 10)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen project pause failed: {}", stderr);
        }

        Ok(())
    }

    #[allow(dead_code)] // May be used in future for project-level resume operations
    pub fn resume_project(&self, project_file: &Path) -> Result<()> {
        let path_str = project_file.to_string_lossy();
        let output = self
            .runner
            .run("mutagen", &["project", "resume", "-f", &path_str], 10)?;

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
            && endpoint
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic())
            && endpoint.chars().nth(1) == Some(':');

        // Remote paths (host:path format) - but exclude Windows drive letters
        if endpoint.contains(':') && !is_windows_drive {
            // Parse as remote: host:path
            if let Some((host, path)) = endpoint.split_once(':') {
                // Use SSH to create directory on remote host
                let mkdir_cmd = format!("mkdir -p {}", path);
                let output = self.runner.run("ssh", &[host, &mkdir_cmd], 10)?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    anyhow::bail!("Failed to create remote directory {}: {}", endpoint, stderr);
                }
                Ok(())
            } else {
                anyhow::bail!("Invalid remote endpoint format: {}", endpoint)
            }
        } else {
            // Local path - use std::fs
            std::fs::create_dir_all(endpoint)
                .with_context(|| format!("Failed to create local directory {}", endpoint))
        }
    }

    /// Checks if a project is currently running.
    /// Returns true if the project is running, false otherwise.
    pub fn is_project_running(&self, project_file: &Path) -> bool {
        let path_str = project_file.to_string_lossy();
        match self
            .runner
            .run("mutagen", &["project", "list", "-f", &path_str], 3)
        {
            Ok(output) => output.status.success(),
            Err(_) => false, // Timeout or execution error means not running
        }
    }

    pub fn create_push_session(
        &self,
        name: &str,
        alpha: &str,
        beta: &str,
        ignore: Option<&[String]>,
    ) -> Result<()> {
        let mut args = vec![
            "sync",
            "create",
            alpha,
            beta,
            "-m",
            "one-way-replica",
            "-n",
            name,
        ];

        // Collect ignore patterns as owned strings to extend lifetime
        let ignore_args: Vec<String> = ignore
            .unwrap_or(&[])
            .iter()
            .flat_map(|pattern| vec!["--ignore".to_string(), pattern.clone()])
            .collect();

        // Convert to &str slice for the runner
        let ignore_refs: Vec<&str> = ignore_args.iter().map(|s| s.as_str()).collect();
        args.extend(ignore_refs);

        let output = self.runner.run("mutagen", &args, 15)?;

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
    use crate::command::{failure_output, success_output, MockCommandRunner};

    // ============ list_sessions tests ============

    #[test]
    fn test_list_sessions_empty() {
        let runner = MockCommandRunner::new();
        runner.expect(
            "mutagen sync list --template {{json .}}",
            success_output("[]"),
        );

        let client = MutagenClient::with_runner(runner);
        let sessions = client.list_sessions().unwrap();

        assert_eq!(sessions.len(), 0);
    }

    #[test]
    fn test_list_sessions_empty_string() {
        let runner = MockCommandRunner::new();
        runner.expect(
            "mutagen sync list --template {{json .}}",
            success_output(""),
        );

        let client = MutagenClient::with_runner(runner);
        let sessions = client.list_sessions().unwrap();

        assert_eq!(sessions.len(), 0);
    }

    #[test]
    fn test_list_sessions_with_sessions() {
        let runner = MockCommandRunner::new();
        let json = r#"[{
            "name": "test-session",
            "identifier": "session-123",
            "alpha": {
                "protocol": "local",
                "path": "/local/path",
                "connected": true,
                "scanned": true
            },
            "beta": {
                "protocol": "ssh",
                "path": "/remote/path",
                "host": "server.example.com",
                "connected": true,
                "scanned": true
            },
            "status": "Watching for changes",
            "paused": false,
            "conflicts": []
        }]"#;

        runner.expect(
            "mutagen sync list --template {{json .}}",
            success_output(json),
        );

        let client = MutagenClient::with_runner(runner);
        let sessions = client.list_sessions().unwrap();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].name, "test-session");
        assert_eq!(sessions[0].identifier, "session-123");
        assert!(!sessions[0].paused);
        assert_eq!(sessions[0].alpha.path, "/local/path");
        assert_eq!(
            sessions[0].beta.host.as_ref().unwrap(),
            "server.example.com"
        );
    }

    #[test]
    fn test_list_sessions_command_fails() {
        let runner = MockCommandRunner::new();
        runner.expect(
            "mutagen sync list --template {{json .}}",
            failure_output("daemon not running"),
        );

        let client = MutagenClient::with_runner(runner);
        let result = client.list_sessions();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("daemon not running"));
    }

    #[test]
    fn test_list_sessions_invalid_json() {
        let runner = MockCommandRunner::new();
        runner.expect(
            "mutagen sync list --template {{json .}}",
            success_output("not valid json"),
        );

        let client = MutagenClient::with_runner(runner);
        let result = client.list_sessions();

        assert!(result.is_err());
    }

    // ============ pause_session tests ============

    #[test]
    fn test_pause_session_success() {
        let runner = MockCommandRunner::new();
        runner.expect("mutagen sync pause session-123", success_output(""));

        let client = MutagenClient::with_runner(runner);
        let result = client.pause_session("session-123");

        assert!(result.is_ok());
    }

    #[test]
    fn test_pause_session_failure() {
        let runner = MockCommandRunner::new();
        runner.expect(
            "mutagen sync pause session-123",
            failure_output("session not found"),
        );

        let client = MutagenClient::with_runner(runner);
        let result = client.pause_session("session-123");

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("session not found"));
    }

    // ============ resume_session tests ============

    #[test]
    fn test_resume_session_success() {
        let runner = MockCommandRunner::new();
        runner.expect("mutagen sync resume session-123", success_output(""));

        let client = MutagenClient::with_runner(runner);
        let result = client.resume_session("session-123");

        assert!(result.is_ok());
    }

    // ============ terminate_session tests ============

    #[test]
    fn test_terminate_session_success() {
        let runner = MockCommandRunner::new();
        runner.expect("mutagen sync terminate session-123", success_output(""));

        let client = MutagenClient::with_runner(runner);
        let result = client.terminate_session("session-123");

        assert!(result.is_ok());
    }

    // ============ flush_session tests ============

    #[test]
    fn test_flush_session_success() {
        let runner = MockCommandRunner::new();
        runner.expect("mutagen sync flush session-123", success_output(""));

        let client = MutagenClient::with_runner(runner);
        let result = client.flush_session("session-123");

        assert!(result.is_ok());
    }

    // ============ is_project_running tests ============

    #[test]
    fn test_is_project_running_true() {
        let runner = MockCommandRunner::new();
        runner.expect(
            "mutagen project list -f /path/to/project.yml",
            success_output("project output"),
        );

        let client = MutagenClient::with_runner(runner);
        let result = client.is_project_running(std::path::Path::new("/path/to/project.yml"));

        assert!(result);
    }

    #[test]
    fn test_is_project_running_false() {
        let runner = MockCommandRunner::new();
        runner.expect(
            "mutagen project list -f /path/to/project.yml",
            failure_output("no project"),
        );

        let client = MutagenClient::with_runner(runner);
        let result = client.is_project_running(std::path::Path::new("/path/to/project.yml"));

        assert!(!result);
    }

    // ============ Existing tests ============

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
