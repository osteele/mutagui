package mutagen

import (
	"context"
	"encoding/json"
	"fmt"
	"os/exec"
	"strings"
	"time"
)

// Client provides methods for interacting with the Mutagen CLI.
type Client struct {
	timeout time.Duration
}

// wrapConnectionError checks the output for connection-related issues and returns
// an improved error message with hints about potential causes.
func wrapConnectionError(baseErr string, output string) error {
	lowerOutput := strings.ToLower(output)

	// Check for agent connection hanging - most common issue
	if strings.Contains(lowerOutput, "connecting to agent") &&
		!strings.Contains(lowerOutput, "connected") {
		return fmt.Errorf("%s (hint: mutagen agent may be stuck on remote - try 'ssh <host> pkill mutagen')", baseErr)
	}

	// Check for agent installation/version issues
	if strings.Contains(lowerOutput, "agent") &&
		(strings.Contains(lowerOutput, "version") || strings.Contains(lowerOutput, "install")) {
		return fmt.Errorf("%s (hint: mutagen agent version mismatch - try 'mutagen daemon stop && mutagen daemon start')", baseErr)
	}

	// Check for SSH connection issues
	if strings.Contains(lowerOutput, "connection refused") ||
		strings.Contains(lowerOutput, "connection timed out") ||
		strings.Contains(lowerOutput, "no route to host") {
		return fmt.Errorf("%s (hint: remote host may be unreachable)", baseErr)
	}

	// Check for authentication issues
	if strings.Contains(lowerOutput, "permission denied") ||
		strings.Contains(lowerOutput, "authentication failed") {
		return fmt.Errorf("%s (hint: check SSH authentication)", baseErr)
	}

	// Check for host key issues
	if strings.Contains(lowerOutput, "host key") {
		return fmt.Errorf("%s (hint: SSH host key issue - may need to update known_hosts)", baseErr)
	}

	// Check for session name conflicts
	if strings.Contains(lowerOutput, "already exists") ||
		strings.Contains(lowerOutput, "duplicate") {
		return fmt.Errorf("%s (hint: session with this name already exists - terminate it first)", baseErr)
	}

	// Check for cross-device link error (NFS/remote filesystem issue)
	if strings.Contains(lowerOutput, "invalid cross-device link") ||
		strings.Contains(lowerOutput, "cross-device") {
		return fmt.Errorf("%s (hint: /tmp and ~/.mutagen are on different filesystems - manually install agent and create symlink: ln -s <nfs-path>/.mutagen ~/.mutagen)", baseErr)
	}

	// Check for agent installation issues
	if strings.Contains(lowerOutput, "unable to install agent") ||
		strings.Contains(lowerOutput, "installation error") {
		return fmt.Errorf("%s (hint: agent installation failed on remote - check disk space and permissions)", baseErr)
	}

	return fmt.Errorf("%s", baseErr)
}

// NewClient creates a new Mutagen client with the given timeout.
func NewClient(timeout time.Duration) *Client {
	return &Client{timeout: timeout}
}

// ListSessions returns all Mutagen sync sessions.
func (c *Client) ListSessions(ctx context.Context) ([]SyncSession, error) {
	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	cmd := exec.CommandContext(ctx, "mutagen", "sync", "list", "--template", "{{json .}}")
	output, err := cmd.Output()
	if err != nil {
		if exitErr, ok := err.(*exec.ExitError); ok {
			return nil, fmt.Errorf("mutagen sync list failed: %s", string(exitErr.Stderr))
		}
		return nil, fmt.Errorf("mutagen sync list failed: %w", err)
	}

	// Handle empty output (no sessions)
	trimmed := strings.TrimSpace(string(output))
	if trimmed == "" || trimmed == "null" {
		return []SyncSession{}, nil
	}

	// Parse the JSON array output
	// Note: mutagen --template '{{json .}}' outputs a JSON array: [{session1}, {session2}, ...]
	var sessions []SyncSession
	if err := json.Unmarshal(output, &sessions); err != nil {
		return nil, fmt.Errorf("failed to parse mutagen output: %w", err)
	}

	return sessions, nil
}

// SessionOptions contains optional settings for creating a sync session.
type SessionOptions struct {
	Mode      string   // Sync mode (e.g., "two-way-safe", "one-way-replica")
	Ignore    []string // Paths to ignore
	IgnoreVCS *bool    // Whether to ignore VCS directories
}

// CreateSession creates a new sync session with the given name and endpoints.
// This is used for starting individual specs (not whole projects).
func (c *Client) CreateSession(ctx context.Context, name, alpha, beta string, opts *SessionOptions) error {
	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	args := []string{"sync", "create", alpha, beta, "--name", name}

	// Apply session options
	if opts != nil {
		if opts.Mode != "" {
			args = append(args, "--sync-mode", opts.Mode)
		}
		for _, pattern := range opts.Ignore {
			args = append(args, "--ignore", pattern)
		}
		if opts.IgnoreVCS != nil && !*opts.IgnoreVCS {
			args = append(args, "--no-ignore-vcs")
		}
	}

	cmd := exec.CommandContext(ctx, "mutagen", args...)
	if output, err := cmd.CombinedOutput(); err != nil {
		return wrapConnectionError("mutagen sync create failed", string(output))
	}
	return nil
}

// CreatePushSession creates a one-way sync session (alpha to beta).
func (c *Client) CreatePushSession(ctx context.Context, name, alpha, beta string, ignore []string) error {
	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	args := []string{"sync", "create", alpha, beta, "--name", name, "--sync-mode", "one-way-replica"}
	for _, pattern := range ignore {
		args = append(args, "--ignore", pattern)
	}

	cmd := exec.CommandContext(ctx, "mutagen", args...)
	if output, err := cmd.CombinedOutput(); err != nil {
		return wrapConnectionError("mutagen sync create (push) failed", string(output))
	}
	return nil
}

// TerminateSession terminates a sync session by name.
func (c *Client) TerminateSession(ctx context.Context, name string) error {
	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	cmd := exec.CommandContext(ctx, "mutagen", "sync", "terminate", name)
	if output, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("mutagen sync terminate failed: %s", string(output))
	}
	return nil
}

// PauseSession pauses a sync session by name.
func (c *Client) PauseSession(ctx context.Context, name string) error {
	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	cmd := exec.CommandContext(ctx, "mutagen", "sync", "pause", name)
	if output, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("mutagen sync pause failed: %s", string(output))
	}
	return nil
}

// ResumeSession resumes a paused sync session by name.
func (c *Client) ResumeSession(ctx context.Context, name string) error {
	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	cmd := exec.CommandContext(ctx, "mutagen", "sync", "resume", name)
	if output, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("mutagen sync resume failed: %s", string(output))
	}
	return nil
}

// FlushSession forces a sync cycle on a session by name.
func (c *Client) FlushSession(ctx context.Context, name string) error {
	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	cmd := exec.CommandContext(ctx, "mutagen", "sync", "flush", name)
	if output, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("mutagen sync flush failed: %s", string(output))
	}
	return nil
}

// ResetSession resets a sync session by name to resolve conflicts.
func (c *Client) ResetSession(ctx context.Context, name string) error {
	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	cmd := exec.CommandContext(ctx, "mutagen", "sync", "reset", name)
	if output, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("mutagen sync reset failed: %s", string(output))
	}
	return nil
}

// ProjectStart starts all sessions defined in a mutagen project file.
func (c *Client) ProjectStart(ctx context.Context, projectFilePath string) error {
	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	cmd := exec.CommandContext(ctx, "mutagen", "project", "start", "-f", projectFilePath)
	if output, err := cmd.CombinedOutput(); err != nil {
		return wrapConnectionError("mutagen project start failed", string(output))
	}
	return nil
}

// ProjectTerminate terminates all sessions for a mutagen project file.
func (c *Client) ProjectTerminate(ctx context.Context, projectFilePath string) error {
	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	cmd := exec.CommandContext(ctx, "mutagen", "project", "terminate", "-f", projectFilePath)
	if output, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("mutagen project terminate failed: %s", string(output))
	}
	return nil
}

// ProjectPause pauses all sessions for a mutagen project file.
func (c *Client) ProjectPause(ctx context.Context, projectFilePath string) error {
	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	cmd := exec.CommandContext(ctx, "mutagen", "project", "pause", "-f", projectFilePath)
	if output, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("mutagen project pause failed: %s", string(output))
	}
	return nil
}

// ProjectResume resumes all sessions for a mutagen project file.
func (c *Client) ProjectResume(ctx context.Context, projectFilePath string) error {
	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	cmd := exec.CommandContext(ctx, "mutagen", "project", "resume", "-f", projectFilePath)
	if output, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("mutagen project resume failed: %s", string(output))
	}
	return nil
}

// ProjectFlush flushes all sessions for a mutagen project file.
func (c *Client) ProjectFlush(ctx context.Context, projectFilePath string) error {
	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	cmd := exec.CommandContext(ctx, "mutagen", "project", "flush", "-f", projectFilePath)
	if output, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("mutagen project flush failed: %s", string(output))
	}
	return nil
}

// IsInstalled checks if the mutagen CLI is installed and accessible.
func (c *Client) IsInstalled() bool {
	cmd := exec.Command("mutagen", "version")
	return cmd.Run() == nil
}

// GetVersion returns the installed mutagen version.
func (c *Client) GetVersion() (string, error) {
	cmd := exec.Command("mutagen", "version")
	output, err := cmd.Output()
	if err != nil {
		return "", fmt.Errorf("failed to get mutagen version: %w", err)
	}
	return strings.TrimSpace(string(output)), nil
}
