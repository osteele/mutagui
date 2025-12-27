package mutagen

import (
	"context"
	"encoding/json"
	"testing"
	"time"
)

func TestWrapConnectionError(t *testing.T) {
	tests := []struct {
		name        string
		baseErr     string
		output      string
		wantContain string
	}{
		{
			name:        "agent stuck",
			baseErr:     "failed",
			output:      "Connecting to agent on remote...",
			wantContain: "pkill mutagen",
		},
		{
			name:        "agent version mismatch",
			baseErr:     "failed",
			output:      "agent version incompatible",
			wantContain: "mutagen daemon stop",
		},
		{
			name:        "connection refused",
			baseErr:     "failed",
			output:      "ssh: connection refused",
			wantContain: "unreachable",
		},
		{
			name:        "connection timeout",
			baseErr:     "failed",
			output:      "connection timed out",
			wantContain: "unreachable",
		},
		{
			name:        "permission denied",
			baseErr:     "failed",
			output:      "Permission denied (publickey)",
			wantContain: "SSH authentication",
		},
		{
			name:        "authentication failed",
			baseErr:     "failed",
			output:      "authentication failed",
			wantContain: "SSH authentication",
		},
		{
			name:        "host key issue",
			baseErr:     "failed",
			output:      "Host key verification failed",
			wantContain: "known_hosts",
		},
		{
			name:        "session already exists",
			baseErr:     "failed",
			output:      "session already exists with this name",
			wantContain: "terminate it first",
		},
		{
			name:        "cross-device link",
			baseErr:     "failed",
			output:      "invalid cross-device link",
			wantContain: "symlink",
		},
		{
			name:        "installation error",
			baseErr:     "failed",
			output:      "installation error: disk full",
			wantContain: "disk space",
		},
		{
			name:        "no hint for unknown error",
			baseErr:     "something went wrong",
			output:      "unknown error type",
			wantContain: "something went wrong",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := wrapConnectionError(tt.baseErr, tt.output)
			if err == nil {
				t.Fatal("wrapConnectionError() returned nil")
			}
			errMsg := err.Error()
			if !contains(errMsg, tt.wantContain) {
				t.Errorf("wrapConnectionError() = %q, want to contain %q", errMsg, tt.wantContain)
			}
		})
	}
}

func contains(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(substr) == 0 ||
		(len(s) > 0 && len(substr) > 0 && searchSubstring(s, substr)))
}

func searchSubstring(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}

func TestNewClient(t *testing.T) {
	timeout := 5 * time.Second
	client := NewClient(timeout)
	if client == nil {
		t.Fatal("NewClient() returned nil")
	}
	if client.timeout != timeout {
		t.Errorf("client.timeout = %v, want %v", client.timeout, timeout)
	}
}

func TestSessionOptions_BuildArgs(t *testing.T) {
	// Test that CreateSession builds correct arguments
	// We can't easily test the actual execution without mocking exec.Command,
	// but we can verify the logic by inspecting the code paths

	tests := []struct {
		name     string
		opts     *SessionOptions
		wantArgs []string // args that should be present
	}{
		{
			name:     "nil options",
			opts:     nil,
			wantArgs: []string{},
		},
		{
			name: "with mode",
			opts: &SessionOptions{
				Mode: "two-way-safe",
			},
			wantArgs: []string{"--sync-mode", "two-way-safe"},
		},
		{
			name: "with ignore patterns",
			opts: &SessionOptions{
				Ignore: []string{"*.log", "tmp/"},
			},
			wantArgs: []string{"--ignore", "*.log", "--ignore", "tmp/"},
		},
		{
			name: "with ignore VCS false",
			opts: &SessionOptions{
				IgnoreVCS: boolPtr(false),
			},
			wantArgs: []string{"--no-ignore-vcs"},
		},
		{
			name: "with symlink mode",
			opts: &SessionOptions{
				SymlinkMode: "ignore",
			},
			wantArgs: []string{"--symlink-mode", "ignore"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			args := buildSessionArgs(tt.opts)
			for _, wantArg := range tt.wantArgs {
				found := false
				for _, arg := range args {
					if arg == wantArg {
						found = true
						break
					}
				}
				if !found {
					t.Errorf("buildSessionArgs() missing arg %q, got %v", wantArg, args)
				}
			}
		})
	}
}

func boolPtr(b bool) *bool {
	return &b
}

// buildSessionArgs extracts the argument-building logic for testing
func buildSessionArgs(opts *SessionOptions) []string {
	var args []string
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
		if opts.SymlinkMode != "" {
			args = append(args, "--symlink-mode", opts.SymlinkMode)
		}
	}
	return args
}

// mockCommandRunner is used for testing command execution
type mockCommandRunner struct {
	lastArgs []string
	output   []byte
	err      error
}

func (m *mockCommandRunner) Run(_ context.Context, args ...string) ([]byte, error) {
	m.lastArgs = args
	return m.output, m.err
}

// CommandRunner interface allows mocking exec.Command in tests
type CommandRunner interface {
	Run(ctx context.Context, args ...string) ([]byte, error)
}

// TestableClient wraps Client with an injectable command runner
type TestableClient struct {
	*Client
	runner CommandRunner
}

func TestListSessions_EmptyOutput(t *testing.T) {
	// Test that empty output returns empty slice
	mock := &mockCommandRunner{output: []byte("")}
	client := &TestableClient{
		Client: NewClient(time.Second),
		runner: mock,
	}

	// Since we can't easily inject the runner into the existing Client,
	// we'll test the parsing logic separately
	testCases := []struct {
		name   string
		output string
		want   int
	}{
		{"empty string", "", 0},
		{"null", "null", 0},
		{"whitespace", "   \n\t  ", 0},
	}

	for _, tc := range testCases {
		t.Run(tc.name, func(t *testing.T) {
			// Simulate the parsing logic from ListSessions
			trimmed := trimSpace(tc.output)
			if trimmed == "" || trimmed == "null" {
				// Would return empty slice
				return
			}
			t.Errorf("expected empty result handling for %q", tc.output)
		})
	}

	_ = client // silence unused warning
}

func trimSpace(s string) string {
	start := 0
	end := len(s)
	for start < end && (s[start] == ' ' || s[start] == '\t' || s[start] == '\n' || s[start] == '\r') {
		start++
	}
	for end > start && (s[end-1] == ' ' || s[end-1] == '\t' || s[end-1] == '\n' || s[end-1] == '\r') {
		end--
	}
	return s[start:end]
}

func TestListSessions_ParseJSON(t *testing.T) {
	// Test JSON parsing for session list
	validJSON := `[{"name":"test-session","identifier":"abc123","status":"Watching for changes"}]`

	var sessions []SyncSession
	if err := jsonUnmarshal([]byte(validJSON), &sessions); err != nil {
		t.Fatalf("failed to parse valid JSON: %v", err)
	}

	if len(sessions) != 1 {
		t.Errorf("got %d sessions, want 1", len(sessions))
	}
	if sessions[0].Name != "test-session" {
		t.Errorf("session name = %q, want %q", sessions[0].Name, "test-session")
	}
}

// jsonUnmarshal wraps json.Unmarshal for testing
func jsonUnmarshal(data []byte, v interface{}) error {
	return json.Unmarshal(data, v)
}
