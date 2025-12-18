package mutagen

import (
	"testing"
)

func TestEndpoint_DisplayPath(t *testing.T) {
	tests := []struct {
		name     string
		endpoint Endpoint
		want     string
	}{
		{
			name: "local path",
			endpoint: Endpoint{
				Path: "/home/user/project",
			},
			want: "/home/user/project",
		},
		{
			name: "remote path with host",
			endpoint: Endpoint{
				Path: "/remote/path",
				Host: strPtr("server"),
			},
			want: "server:/remote/path",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := tt.endpoint.DisplayPath()
			if got != tt.want {
				t.Errorf("DisplayPath() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestEndpoint_StatusIcon(t *testing.T) {
	tests := []struct {
		name     string
		endpoint Endpoint
		want     string
	}{
		{
			name: "disconnected",
			endpoint: Endpoint{
				Connected: false,
				Scanned:   false,
			},
			want: "⊗",
		},
		{
			name: "connected but not scanned",
			endpoint: Endpoint{
				Connected: true,
				Scanned:   false,
			},
			want: "⟳",
		},
		{
			name: "connected and scanned",
			endpoint: Endpoint{
				Connected: true,
				Scanned:   true,
			},
			want: "✓",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := tt.endpoint.StatusIcon()
			if got != tt.want {
				t.Errorf("StatusIcon() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestSyncSession_HasConflicts(t *testing.T) {
	tests := []struct {
		name    string
		session SyncSession
		want    bool
	}{
		{
			name:    "no conflicts",
			session: SyncSession{Conflicts: nil},
			want:    false,
		},
		{
			name:    "empty conflicts",
			session: SyncSession{Conflicts: []Conflict{}},
			want:    false,
		},
		{
			name: "has conflicts",
			session: SyncSession{
				Conflicts: []Conflict{{Root: "/path"}},
			},
			want: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := tt.session.HasConflicts()
			if got != tt.want {
				t.Errorf("HasConflicts() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestSyncSession_ConflictCount(t *testing.T) {
	tests := []struct {
		name    string
		session SyncSession
		want    int
	}{
		{
			name:    "no conflicts",
			session: SyncSession{Conflicts: nil},
			want:    0,
		},
		{
			name: "three conflicts",
			session: SyncSession{
				Conflicts: []Conflict{
					{Root: "/path1"},
					{Root: "/path2"},
					{Root: "/path3"},
				},
			},
			want: 3,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := tt.session.ConflictCount()
			if got != tt.want {
				t.Errorf("ConflictCount() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestSyncSession_StatusIcon(t *testing.T) {
	tests := []struct {
		name   string
		status string
		want   string
	}{
		{"watching", "Watching for changes", "👁"},
		{"scanning", "Scanning alpha", "🔍"},
		{"staging", "Staging files", "📦"},
		{"reconciling", "Reconciling changes", "⚖"},
		{"saving", "Saving state", "💾"},
		{"connecting", "Connecting to beta", "🔌"},
		{"transitioning", "Transitioning", "⏳"},
		{"halted", "Halted due to error", "⛔"},
		{"unknown", "SomeOtherStatus", "•"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			session := SyncSession{Status: tt.status}
			got := session.StatusIcon()
			if got != tt.want {
				t.Errorf("StatusIcon() for status %q = %q, want %q", tt.status, got, tt.want)
			}
		})
	}
}

func TestSyncSession_StatusText(t *testing.T) {
	tests := []struct {
		name   string
		status string
		want   string
	}{
		{"watching", "Watching for changes", "Watching"},
		{"scanning", "Scanning alpha", "Scanning"},
		{"staging", "Staging files", "Staging"},
		{"reconciling", "Reconciling changes", "Reconciling"},
		{"saving", "Saving state", "Saving"},
		{"connecting", "Connecting to beta", "Connecting"},
		{"transitioning", "Transitioning", "Transitioning"},
		{"halted", "Halted due to error", "Halted"},
		{"waiting", "Waiting for connection", "Waiting"},
		{"unknown", "SomeOtherStatus", "Unknown"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			session := SyncSession{Status: tt.status}
			got := session.StatusText()
			if got != tt.want {
				t.Errorf("StatusText() for status %q = %q, want %q", tt.status, got, tt.want)
			}
		})
	}
}

func TestSyncSession_AlphaDisplay(t *testing.T) {
	session := SyncSession{
		Alpha: Endpoint{Path: "/local/path"},
	}
	got := session.AlphaDisplay()
	if got != "/local/path" {
		t.Errorf("AlphaDisplay() = %q, want %q", got, "/local/path")
	}
}

func TestSyncSession_BetaDisplay(t *testing.T) {
	session := SyncSession{
		Beta: Endpoint{
			Path: "/remote/path",
			Host: strPtr("server"),
		},
	}
	got := session.BetaDisplay()
	want := "server:/remote/path"
	if got != want {
		t.Errorf("BetaDisplay() = %q, want %q", got, want)
	}
}

// Helper function
func strPtr(s string) *string {
	return &s
}
