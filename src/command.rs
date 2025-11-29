//! Command execution abstraction for dependency injection and testing.
//!
//! This module provides a trait-based abstraction over command execution,
//! allowing the `MutagenClient` to be tested without requiring the actual
//! `mutagen` binary.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::process::Output;
use std::time::Duration;
use tokio::process::Command as TokioCommand;
use tokio::time::timeout;

/// Trait for executing external commands.
///
/// This abstraction allows `MutagenClient` to be generic over how commands
/// are executed, enabling dependency injection of mock implementations for testing.
#[async_trait]
pub trait CommandRunner: Send + Sync {
    /// Execute a command with the given program and arguments.
    ///
    /// # Arguments
    /// * `program` - The program to execute (e.g., "mutagen", "ssh")
    /// * `args` - Command-line arguments
    /// * `timeout_secs` - Maximum time to wait for command completion
    ///
    /// # Returns
    /// The command's output including stdout, stderr, and exit status.
    async fn run(&self, program: &str, args: &[&str], timeout_secs: u64) -> Result<Output>;
}

/// Production implementation that executes real system commands.
#[derive(Debug, Clone, Default)]
pub struct SystemCommandRunner;

impl SystemCommandRunner {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandRunner for SystemCommandRunner {
    async fn run(&self, program: &str, args: &[&str], timeout_secs: u64) -> Result<Output> {
        let child = TokioCommand::new(program)
            .args(args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| anyhow!("Failed to spawn command '{}': {}", program, e))?;

        let timeout_duration = Duration::from_secs(timeout_secs);

        match timeout(timeout_duration, child.wait_with_output()).await {
            Ok(Ok(output)) => Ok(output),
            Ok(Err(e)) => Err(anyhow!("Command '{}' failed: {}", program, e)),
            Err(_) => {
                // Timeout occurred - kill the child process
                // Note: child.kill() requires &mut self, but we've moved child into wait_with_output
                // The timeout cancellation will drop the future which should clean up the child
                anyhow::bail!(
                    "Command '{}' timed out after {} seconds",
                    program,
                    timeout_secs
                )
            }
        }
    }
}

/// Mock implementation for testing that returns pre-configured responses.
/// Supports sequential responses: if the same command is expected multiple times,
/// each call will return the next response in the sequence.
#[cfg(test)]
#[derive(Debug, Default)]
pub struct MockCommandRunner {
    /// Map of command strings to their expected outputs (as a queue for sequential calls).
    /// Key format: "program arg1 arg2 ..."
    responses: std::sync::Mutex<std::collections::HashMap<String, Vec<Result<Output, String>>>>,
    /// Record of commands that were executed (for verification)
    executed: std::sync::Mutex<Vec<String>>,
}

#[cfg(test)]
impl MockCommandRunner {
    pub fn new() -> Self {
        Self {
            responses: std::sync::Mutex::new(std::collections::HashMap::new()),
            executed: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Configure an expected command and its response.
    /// If the same command is expected multiple times, responses are returned in order.
    ///
    /// # Arguments
    /// * `command` - The full command string (e.g., "mutagen sync list --template {{json .}}")
    /// * `output` - The output to return when this command is executed
    pub fn expect(&self, command: &str, output: Output) {
        self.responses
            .lock()
            .unwrap()
            .entry(command.to_string())
            .or_default()
            .push(Ok(output));
    }

    /// Configure an expected command to return an error.
    /// If the same command is expected multiple times, responses are returned in order.
    pub fn expect_error(&self, command: &str, error_msg: &str) {
        self.responses
            .lock()
            .unwrap()
            .entry(command.to_string())
            .or_default()
            .push(Err(error_msg.to_string()));
    }

    /// Get the list of commands that were executed.
    pub fn executed_commands(&self) -> Vec<String> {
        self.executed.lock().unwrap().clone()
    }

    /// Check if a specific command was executed.
    #[allow(dead_code)]
    pub fn was_executed(&self, command: &str) -> bool {
        self.executed.lock().unwrap().iter().any(|c| c == command)
    }
}

#[cfg(test)]
#[async_trait]
impl CommandRunner for MockCommandRunner {
    async fn run(&self, program: &str, args: &[&str], _timeout_secs: u64) -> Result<Output> {
        let command = format!("{} {}", program, args.join(" "));

        // Record the execution
        self.executed.lock().unwrap().push(command.clone());

        // Look up and consume the next response for this command
        let mut responses = self.responses.lock().unwrap();
        match responses.get_mut(&command) {
            Some(queue) if !queue.is_empty() => {
                // Take the first response from the queue
                match queue.remove(0) {
                    Ok(output) => Ok(output),
                    Err(msg) => Err(anyhow!("{}", msg)),
                }
            }
            _ => Err(anyhow!(
                "MockCommandRunner: No response configured for command: {}",
                command
            )),
        }
    }
}

/// Helper to create a successful Output with given stdout.
#[cfg(test)]
pub fn success_output(stdout: &str) -> Output {
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    Output {
        status: ExitStatus::from_raw(0),
        stdout: stdout.as_bytes().to_vec(),
        stderr: Vec::new(),
    }
}

/// Helper to create a failed Output with given stderr.
#[cfg(test)]
pub fn failure_output(stderr: &str) -> Output {
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    Output {
        status: ExitStatus::from_raw(1 << 8), // Exit code 1
        stdout: Vec::new(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_runner_returns_configured_output() {
        let mock = MockCommandRunner::new();
        mock.expect("echo hello world", success_output("hello world\n"));

        let result = mock.run("echo", &["hello", "world"], 5).await.unwrap();

        assert!(result.status.success());
        assert_eq!(String::from_utf8_lossy(&result.stdout), "hello world\n");
    }

    #[tokio::test]
    async fn test_mock_runner_records_executed_commands() {
        let mock = MockCommandRunner::new();
        mock.expect("test cmd", success_output(""));

        let _ = mock.run("test", &["cmd"], 5).await;

        assert!(mock.was_executed("test cmd"));
        assert_eq!(mock.executed_commands(), vec!["test cmd"]);
    }

    #[tokio::test]
    async fn test_mock_runner_returns_error_for_unconfigured_command() {
        let mock = MockCommandRunner::new();

        let result = mock.run("unknown", &["cmd"], 5).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No response configured"));
    }

    #[tokio::test]
    async fn test_mock_runner_returns_configured_error() {
        let mock = MockCommandRunner::new();
        mock.expect_error("fail cmd", "Command failed");

        let result = mock.run("fail", &["cmd"], 5).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Command failed"));
    }

    #[test]
    fn test_failure_output_has_nonzero_exit_code() {
        let output = failure_output("error message");

        assert!(!output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stderr), "error message");
    }

    #[tokio::test]
    async fn test_system_runner_executes_command() {
        let runner = SystemCommandRunner::new();

        let result = runner.run("echo", &["hello"], 5).await.unwrap();

        assert!(result.status.success());
        assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "hello");
    }

    #[tokio::test]
    async fn test_system_runner_timeout() {
        let runner = SystemCommandRunner::new();

        // Use sleep command with 10 second sleep, but only 1 second timeout
        let result = runner.run("sleep", &["10"], 1).await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("timed out"),
            "Expected timeout error, got: {}",
            err
        );
    }
}
