use crate::command::{CommandRunner, SystemCommandRunner};
use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use shell_escape::escape;
use std::borrow::Cow;
use std::path::{Path, PathBuf};

/// Get the lock file path for a Mutagen project file.
/// Mutagen creates a `.lock` file with the same name as the project file
/// (e.g., `project.yml.lock` for `project.yml`).
fn get_project_lock_path(project_file: &Path) -> PathBuf {
    let mut lock_path = project_file.as_os_str().to_owned();
    lock_path.push(".lock");
    PathBuf::from(lock_path)
}

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

    pub async fn list_sessions(&self) -> Result<Vec<SyncSession>> {
        let output = self
            .runner
            .run("mutagen", &["sync", "list", "--template", "{{json .}}"], 5)
            .await?;

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

    pub async fn pause_session(&self, identifier: &str) -> Result<()> {
        let output = self
            .runner
            .run("mutagen", &["sync", "pause", identifier], 5)
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen sync pause failed: {}", stderr);
        }

        Ok(())
    }

    pub async fn resume_session(&self, identifier: &str) -> Result<()> {
        let output = self
            .runner
            .run("mutagen", &["sync", "resume", identifier], 5)
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen sync resume failed: {}", stderr);
        }

        Ok(())
    }

    pub async fn terminate_session(&self, identifier: &str) -> Result<()> {
        let output = self
            .runner
            .run("mutagen", &["sync", "terminate", identifier], 5)
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen sync terminate failed: {}", stderr);
        }

        Ok(())
    }

    pub async fn flush_session(&self, identifier: &str) -> Result<()> {
        let output = self
            .runner
            .run("mutagen", &["sync", "flush", identifier], 5)
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen sync flush failed: {}", stderr);
        }

        Ok(())
    }

    pub async fn start_project(&self, project_file: &Path) -> Result<()> {
        let path_str = project_file.to_string_lossy();
        let output = self
            .runner
            .run("mutagen", &["project", "start", "-f", &path_str], 10)
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);

            // Check if failure is due to "project already running"
            if stderr.contains("project already running") {
                // Check if there are actually any running sessions
                let sessions = self.list_sessions().await.unwrap_or_default();
                if sessions.is_empty() {
                    // No sessions running - this is a stale lock file
                    // Remove it and retry
                    let lock_file = get_project_lock_path(project_file);
                    if lock_file.exists() {
                        std::fs::remove_file(&lock_file).with_context(|| {
                            format!("Failed to remove stale lock file: {}", lock_file.display())
                        })?;

                        // Retry the start
                        let retry_output = self
                            .runner
                            .run("mutagen", &["project", "start", "-f", &path_str], 10)
                            .await?;

                        if !retry_output.status.success() {
                            let retry_stderr = String::from_utf8_lossy(&retry_output.stderr);
                            anyhow::bail!("mutagen project start failed: {}", retry_stderr);
                        }

                        return Ok(());
                    }
                }
            }

            anyhow::bail!("mutagen project start failed: {}", stderr);
        }

        Ok(())
    }

    pub async fn terminate_project(&self, project_file: &Path) -> Result<()> {
        let path_str = project_file.to_string_lossy();
        let output = self
            .runner
            .run("mutagen", &["project", "terminate", "-f", &path_str], 10)
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen project terminate failed: {}", stderr);
        }

        Ok(())
    }

    #[allow(dead_code)] // May be used in future for project-level pause operations
    pub async fn pause_project(&self, project_file: &Path) -> Result<()> {
        let path_str = project_file.to_string_lossy();
        let output = self
            .runner
            .run("mutagen", &["project", "pause", "-f", &path_str], 10)
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen project pause failed: {}", stderr);
        }

        Ok(())
    }

    #[allow(dead_code)] // May be used in future for project-level resume operations
    pub async fn resume_project(&self, project_file: &Path) -> Result<()> {
        let path_str = project_file.to_string_lossy();
        let output = self
            .runner
            .run("mutagen", &["project", "resume", "-f", &path_str], 10)
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mutagen project resume failed: {}", stderr);
        }

        Ok(())
    }

    /// Ensures a directory exists on an endpoint (local or remote).
    /// For remote endpoints (SSH, Docker), uses SSH to create the directory.
    /// For local paths, uses std::fs::create_dir_all with tilde expansion.
    pub async fn ensure_endpoint_directory_exists(&self, endpoint: &str) -> Result<()> {
        use crate::endpoint::EndpointAddress;

        let parsed = EndpointAddress::parse(endpoint);

        match parsed {
            EndpointAddress::Local(path) => {
                // Expand tilde for local paths
                let expanded = EndpointAddress::Local(path).expand_tilde();
                let final_path = expanded.path();

                std::fs::create_dir_all(final_path)
                    .with_context(|| format!("Failed to create local directory {:?}", final_path))
            }
            EndpointAddress::Ssh {
                user, host, path, ..
            } => {
                // Build the SSH host string (user@host or just host)
                let ssh_host = match user {
                    Some(u) => format!("{}@{}", u, host),
                    None => host,
                };

                // Remote tilde is handled by the remote shell, don't expand it
                let path_str = path.to_string_lossy();
                let escaped_path = escape(Cow::Borrowed(&*path_str));
                let mkdir_cmd = format!("mkdir -p {}", escaped_path);
                let output = self.runner.run("ssh", &[&ssh_host, &mkdir_cmd], 10).await?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    anyhow::bail!("Failed to create remote directory {}: {}", endpoint, stderr);
                }
                Ok(())
            }
            EndpointAddress::Docker { container, path } => {
                // Use docker exec to create directory in container
                let path_str = path.to_string_lossy();
                let escaped_path = escape(Cow::Borrowed(&*path_str));
                let mkdir_cmd = format!("mkdir -p {}", escaped_path);
                let output = self
                    .runner
                    .run("docker", &["exec", &container, "sh", "-c", &mkdir_cmd], 10)
                    .await?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    anyhow::bail!(
                        "Failed to create directory in container {}: {}",
                        container,
                        stderr
                    );
                }
                Ok(())
            }
        }
    }

    pub async fn create_push_session(
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

        let output = self.runner.run("mutagen", &args, 15).await?;

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

    #[tokio::test]
    async fn test_list_sessions_empty() {
        let runner = MockCommandRunner::new();
        runner.expect(
            "mutagen sync list --template {{json .}}",
            success_output("[]"),
        );

        let client = MutagenClient::with_runner(runner);
        let sessions = client.list_sessions().await.unwrap();

        assert_eq!(sessions.len(), 0);
    }

    #[tokio::test]
    async fn test_list_sessions_empty_string() {
        let runner = MockCommandRunner::new();
        runner.expect(
            "mutagen sync list --template {{json .}}",
            success_output(""),
        );

        let client = MutagenClient::with_runner(runner);
        let sessions = client.list_sessions().await.unwrap();

        assert_eq!(sessions.len(), 0);
    }

    #[tokio::test]
    async fn test_list_sessions_with_sessions() {
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
        let sessions = client.list_sessions().await.unwrap();

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

    #[tokio::test]
    async fn test_list_sessions_command_fails() {
        let runner = MockCommandRunner::new();
        runner.expect(
            "mutagen sync list --template {{json .}}",
            failure_output("daemon not running"),
        );

        let client = MutagenClient::with_runner(runner);
        let result = client.list_sessions().await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("daemon not running"));
    }

    #[tokio::test]
    async fn test_list_sessions_invalid_json() {
        let runner = MockCommandRunner::new();
        runner.expect(
            "mutagen sync list --template {{json .}}",
            success_output("not valid json"),
        );

        let client = MutagenClient::with_runner(runner);
        let result = client.list_sessions().await;

        assert!(result.is_err());
    }

    // ============ pause_session tests ============

    #[tokio::test]
    async fn test_pause_session_success() {
        let runner = MockCommandRunner::new();
        runner.expect("mutagen sync pause session-123", success_output(""));

        let client = MutagenClient::with_runner(runner);
        let result = client.pause_session("session-123").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_pause_session_failure() {
        let runner = MockCommandRunner::new();
        runner.expect(
            "mutagen sync pause session-123",
            failure_output("session not found"),
        );

        let client = MutagenClient::with_runner(runner);
        let result = client.pause_session("session-123").await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("session not found"));
    }

    // ============ resume_session tests ============

    #[tokio::test]
    async fn test_resume_session_success() {
        let runner = MockCommandRunner::new();
        runner.expect("mutagen sync resume session-123", success_output(""));

        let client = MutagenClient::with_runner(runner);
        let result = client.resume_session("session-123").await;

        assert!(result.is_ok());
    }

    // ============ terminate_session tests ============

    #[tokio::test]
    async fn test_terminate_session_success() {
        let runner = MockCommandRunner::new();
        runner.expect("mutagen sync terminate session-123", success_output(""));

        let client = MutagenClient::with_runner(runner);
        let result = client.terminate_session("session-123").await;

        assert!(result.is_ok());
    }

    // ============ flush_session tests ============

    #[tokio::test]
    async fn test_flush_session_success() {
        let runner = MockCommandRunner::new();
        runner.expect("mutagen sync flush session-123", success_output(""));

        let client = MutagenClient::with_runner(runner);
        let result = client.flush_session("session-123").await;

        assert!(result.is_ok());
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

    // ============ ensure_endpoint_directory_exists tests ============

    #[tokio::test]
    async fn test_ensure_endpoint_directory_exists_remote_simple_path() {
        let runner = MockCommandRunner::new();
        runner.expect("ssh server mkdir -p /remote/path", success_output(""));

        let client = MutagenClient::with_runner(runner);
        let result = client
            .ensure_endpoint_directory_exists("server:/remote/path")
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_ensure_endpoint_directory_exists_remote_path_with_spaces() {
        let runner = MockCommandRunner::new();
        // shell-escape wraps paths with spaces in single quotes
        runner.expect(
            "ssh server mkdir -p '/remote/path with spaces'",
            success_output(""),
        );

        let client = MutagenClient::with_runner(runner);
        let result = client
            .ensure_endpoint_directory_exists("server:/remote/path with spaces")
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_ensure_endpoint_directory_exists_remote_path_with_special_chars() {
        let runner = MockCommandRunner::new();
        // shell-escape wraps paths with special characters in single quotes
        runner.expect(
            "ssh server mkdir -p '/remote/path$with\"special'",
            success_output(""),
        );

        let client = MutagenClient::with_runner(runner);
        let result = client
            .ensure_endpoint_directory_exists("server:/remote/path$with\"special")
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_ensure_endpoint_directory_exists_windows_drive_letter() {
        // Windows paths like C:\path should be treated as local, not remote
        // This test verifies the Windows drive letter detection works
        let runner = MockCommandRunner::new();
        // No SSH command configured - if it tries to SSH to "C" as a hostname,
        // the test will fail with "No response configured for command: ssh C ..."

        let client = MutagenClient::with_runner(runner);

        // On non-Windows, this will try to create "C:\Users\test" as a local path
        // This may succeed (creating a directory literally named "C:\Users\test")
        // or fail depending on permissions. The key point is it should NOT try SSH.
        let result = client
            .ensure_endpoint_directory_exists("C:\\Users\\test")
            .await;

        // If it's an error, verify it's not an SSH-related error
        if let Err(e) = result {
            let err_msg = e.to_string();
            // Should NOT contain "No response configured" which would indicate SSH was attempted
            assert!(
                !err_msg.contains("No response configured"),
                "Should not attempt SSH for Windows paths, got error: {}",
                err_msg
            );
        }
        // If it succeeded, that's also fine - it created a local directory

        // Clean up if we created the directory
        let _ = std::fs::remove_dir_all("C:\\Users\\test");
        let _ = std::fs::remove_dir("C:\\Users");
        let _ = std::fs::remove_dir("C:");
    }

    #[tokio::test]
    async fn test_ensure_endpoint_directory_exists_ssh_failure() {
        let runner = MockCommandRunner::new();
        runner.expect(
            "ssh server mkdir -p /remote/path",
            failure_output("Permission denied"),
        );

        let client = MutagenClient::with_runner(runner);
        let result = client
            .ensure_endpoint_directory_exists("server:/remote/path")
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Permission denied"));
    }

    // ============ get_project_lock_path tests ============

    #[test]
    fn test_get_project_lock_path() {
        let project_path = Path::new("/path/to/mutagen.yml");
        let lock_path = get_project_lock_path(project_path);
        assert_eq!(lock_path, PathBuf::from("/path/to/mutagen.yml.lock"));
    }

    #[test]
    fn test_get_project_lock_path_with_target() {
        let project_path = Path::new("/path/to/mutagen-server.yml");
        let lock_path = get_project_lock_path(project_path);
        assert_eq!(lock_path, PathBuf::from("/path/to/mutagen-server.yml.lock"));
    }

    // ============ start_project tests ============

    #[tokio::test]
    async fn test_start_project_success() {
        let runner = MockCommandRunner::new();
        runner.expect(
            "mutagen project start -f /path/to/project.yml",
            success_output(""),
        );

        let client = MutagenClient::with_runner(runner);
        let result = client
            .start_project(Path::new("/path/to/project.yml"))
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_start_project_failure() {
        let runner = MockCommandRunner::new();
        runner.expect(
            "mutagen project start -f /path/to/project.yml",
            failure_output("some error"),
        );

        let client = MutagenClient::with_runner(runner);
        let result = client
            .start_project(Path::new("/path/to/project.yml"))
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("some error"));
    }

    #[tokio::test]
    async fn test_start_project_stale_lock_removed() {
        use std::io::Write;

        let temp_dir = tempfile::tempdir().unwrap();
        let project_path = temp_dir.path().join("mutagen.yml");
        let lock_path = temp_dir.path().join("mutagen.yml.lock");

        // Create the project file
        let mut project_file = std::fs::File::create(&project_path).unwrap();
        writeln!(
            project_file,
            "sync:\n  test:\n    alpha: /local\n    beta: server:/remote"
        )
        .unwrap();

        // Create the stale lock file
        let mut lock_file = std::fs::File::create(&lock_path).unwrap();
        writeln!(lock_file, "proj_stale_identifier").unwrap();

        // Verify lock file was created
        assert!(lock_path.exists(), "Lock file should exist before test");

        // Verify get_project_lock_path returns the right path
        let computed_lock = get_project_lock_path(&project_path);
        assert_eq!(
            computed_lock, lock_path,
            "get_project_lock_path should return correct path"
        );

        let runner = MockCommandRunner::new();

        // First start attempt fails with "project already running"
        runner.expect(
            &format!(
                "mutagen project start -f {}",
                project_path.to_string_lossy()
            ),
            failure_output("Error: project already running"),
        );

        // list_sessions returns empty (no sessions running)
        runner.expect(
            "mutagen sync list --template {{json .}}",
            success_output("[]"),
        );

        // Second start attempt succeeds (after lock removal)
        runner.expect(
            &format!(
                "mutagen project start -f {}",
                project_path.to_string_lossy()
            ),
            success_output(""),
        );

        let client = MutagenClient::with_runner(runner);
        let result = client.start_project(&project_path).await;

        assert!(result.is_ok(), "start_project should succeed");
        assert!(
            !lock_path.exists(),
            "Lock file should have been removed after stale lock cleanup"
        );
    }

    #[tokio::test]
    async fn test_start_project_already_running_with_sessions() {
        let runner = MockCommandRunner::new();

        // Start attempt fails with "project already running"
        runner.expect(
            "mutagen project start -f /path/to/project.yml",
            failure_output("Error: project already running"),
        );

        // list_sessions returns sessions (project is actually running)
        let session_json = r#"[{
            "name": "test-session",
            "identifier": "session-123",
            "alpha": { "protocol": "local", "path": "/local", "connected": true, "scanned": true },
            "beta": { "protocol": "ssh", "path": "/remote", "host": "server", "connected": true, "scanned": true },
            "status": "Watching for changes",
            "paused": false,
            "conflicts": []
        }]"#;
        runner.expect(
            "mutagen sync list --template {{json .}}",
            success_output(session_json),
        );

        let client = MutagenClient::with_runner(runner);
        let result = client
            .start_project(Path::new("/path/to/project.yml"))
            .await;

        // Should still fail because project is actually running
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("project already running"));
    }
}
