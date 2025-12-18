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
func (c *Client) CreateSession(ctx context.Context, name, alpha, beta string, labels map[string]string, opts *SessionOptions) error {
	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	args := []string{"sync", "create", alpha, beta, "--name", name}
	// Add name label for label-selector filtering
	args = append(args, "--label", fmt.Sprintf("name=%s", name))
	for k, v := range labels {
		args = append(args, "--label", fmt.Sprintf("%s=%s", k, v))
	}

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
		return fmt.Errorf("mutagen sync create failed: %s", string(output))
	}
	return nil
}

// CreatePushSession creates a one-way sync session (alpha to beta).
func (c *Client) CreatePushSession(ctx context.Context, name, alpha, beta string, labels map[string]string, ignore []string) error {
	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	args := []string{"sync", "create", alpha, beta, "--name", name, "--sync-mode", "one-way-replica"}
	// Add name label for label-selector filtering
	args = append(args, "--label", fmt.Sprintf("name=%s", name))
	for k, v := range labels {
		args = append(args, "--label", fmt.Sprintf("%s=%s", k, v))
	}
	for _, pattern := range ignore {
		args = append(args, "--ignore", pattern)
	}

	cmd := exec.CommandContext(ctx, "mutagen", args...)
	if output, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("mutagen sync create (push) failed: %s", string(output))
	}
	return nil
}

// TerminateSession terminates a sync session by name and project path.
func (c *Client) TerminateSession(ctx context.Context, name, projectPath string) error {
	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	// Use label selector to target specific project's session
	selector := fmt.Sprintf("name=%s,project=%s", name, projectPath)
	cmd := exec.CommandContext(ctx, "mutagen", "sync", "terminate", "--label-selector", selector)
	if output, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("mutagen sync terminate failed: %s", string(output))
	}
	return nil
}

// PauseSession pauses a sync session by name and project path.
func (c *Client) PauseSession(ctx context.Context, name, projectPath string) error {
	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	selector := fmt.Sprintf("name=%s,project=%s", name, projectPath)
	cmd := exec.CommandContext(ctx, "mutagen", "sync", "pause", "--label-selector", selector)
	if output, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("mutagen sync pause failed: %s", string(output))
	}
	return nil
}

// ResumeSession resumes a paused sync session by name and project path.
func (c *Client) ResumeSession(ctx context.Context, name, projectPath string) error {
	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	selector := fmt.Sprintf("name=%s,project=%s", name, projectPath)
	cmd := exec.CommandContext(ctx, "mutagen", "sync", "resume", "--label-selector", selector)
	if output, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("mutagen sync resume failed: %s", string(output))
	}
	return nil
}

// FlushSession forces a sync cycle on a session by name and project path.
func (c *Client) FlushSession(ctx context.Context, name, projectPath string) error {
	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	selector := fmt.Sprintf("name=%s,project=%s", name, projectPath)
	cmd := exec.CommandContext(ctx, "mutagen", "sync", "flush", "--label-selector", selector)
	if output, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("mutagen sync flush failed: %s", string(output))
	}
	return nil
}

// ResetSession resets a sync session by name and project path to resolve conflicts.
func (c *Client) ResetSession(ctx context.Context, name, projectPath string) error {
	ctx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	selector := fmt.Sprintf("name=%s,project=%s", name, projectPath)
	cmd := exec.CommandContext(ctx, "mutagen", "sync", "reset", "--label-selector", selector)
	if output, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("mutagen sync reset failed: %s", string(output))
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
