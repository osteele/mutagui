//! Command execution abstraction for dependency injection and testing.
//!
//! This module provides a trait-based abstraction over command execution,
//! allowing the `MutagenClient` to be tested without requiring the actual
//! `mutagen` binary.

use anyhow::{anyhow, Result};
use std::process::{Command, Output};
use std::time::Duration;

/// Trait for executing external commands.
///
/// This abstraction allows `MutagenClient` to be generic over how commands
/// are executed, enabling dependency injection of mock implementations for testing.
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
    fn run(&self, program: &str, args: &[&str], timeout_secs: u64) -> Result<Output>;
}

/// Production implementation that executes real system commands.
#[derive(Debug, Clone, Default)]
pub struct SystemCommandRunner;

impl SystemCommandRunner {
    pub fn new() -> Self {
        Self
    }
}

impl CommandRunner for SystemCommandRunner {
    fn run(&self, program: &str, args: &[&str], timeout_secs: u64) -> Result<Output> {
        let mut cmd = Command::new(program);
        cmd.args(args);
        execute_with_timeout(cmd, timeout_secs)
    }
}

/// Cross-platform timeout implementation using thread-based approach.
fn execute_with_timeout(mut cmd: Command, timeout_secs: u64) -> Result<Output> {
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
        Ok(Err(e)) => Err(anyhow!("Failed to execute command: {}", e)),
        Err(mpsc::RecvTimeoutError::Timeout) => {
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

/// Mock implementation for testing that returns pre-configured responses.
#[cfg(test)]
#[derive(Debug, Default)]
pub struct MockCommandRunner {
    /// Map of command strings to their expected outputs.
    /// Key format: "program arg1 arg2 ..."
    responses: std::sync::Mutex<std::collections::HashMap<String, Result<Output, String>>>,
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
    ///
    /// # Arguments
    /// * `command` - The full command string (e.g., "mutagen sync list --template {{json .}}")
    /// * `output` - The output to return when this command is executed
    pub fn expect(&self, command: &str, output: Output) {
        self.responses
            .lock()
            .unwrap()
            .insert(command.to_string(), Ok(output));
    }

    /// Configure an expected command to return an error.
    pub fn expect_error(&self, command: &str, error_msg: &str) {
        self.responses
            .lock()
            .unwrap()
            .insert(command.to_string(), Err(error_msg.to_string()));
    }

    /// Get the list of commands that were executed.
    pub fn executed_commands(&self) -> Vec<String> {
        self.executed.lock().unwrap().clone()
    }

    /// Check if a specific command was executed.
    pub fn was_executed(&self, command: &str) -> bool {
        self.executed.lock().unwrap().iter().any(|c| c == command)
    }
}

#[cfg(test)]
impl CommandRunner for MockCommandRunner {
    fn run(&self, program: &str, args: &[&str], _timeout_secs: u64) -> Result<Output> {
        let command = format!("{} {}", program, args.join(" "));

        // Record the execution
        self.executed.lock().unwrap().push(command.clone());

        // Look up the response
        let responses = self.responses.lock().unwrap();
        match responses.get(&command) {
            Some(Ok(output)) => Ok(output.clone()),
            Some(Err(msg)) => Err(anyhow!("{}", msg)),
            None => Err(anyhow!(
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

    #[test]
    fn test_mock_runner_returns_configured_output() {
        let mock = MockCommandRunner::new();
        mock.expect("echo hello world", success_output("hello world\n"));

        let result = mock.run("echo", &["hello", "world"], 5).unwrap();

        assert!(result.status.success());
        assert_eq!(String::from_utf8_lossy(&result.stdout), "hello world\n");
    }

    #[test]
    fn test_mock_runner_records_executed_commands() {
        let mock = MockCommandRunner::new();
        mock.expect("test cmd", success_output(""));

        let _ = mock.run("test", &["cmd"], 5);

        assert!(mock.was_executed("test cmd"));
        assert_eq!(mock.executed_commands(), vec!["test cmd"]);
    }

    #[test]
    fn test_mock_runner_returns_error_for_unconfigured_command() {
        let mock = MockCommandRunner::new();

        let result = mock.run("unknown", &["cmd"], 5);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No response configured"));
    }

    #[test]
    fn test_mock_runner_returns_configured_error() {
        let mock = MockCommandRunner::new();
        mock.expect_error("fail cmd", "Command failed");

        let result = mock.run("fail", &["cmd"], 5);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Command failed"));
    }

    #[test]
    fn test_failure_output_has_nonzero_exit_code() {
        let output = failure_output("error message");

        assert!(!output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stderr), "error message");
    }
}
