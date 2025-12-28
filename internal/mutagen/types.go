// Package mutagen provides types and a client for interacting with the Mutagen CLI.
package mutagen

import (
	"os"
	"strings"
)

// SyncTime represents when the last sync occurred.
type SyncTime int

const (
	// SyncTimeNever means no syncs have occurred yet.
	SyncTimeNever SyncTime = iota
	// SyncTimeUnknown means sync history is unknown (pre-existing session).
	SyncTimeUnknown
	// SyncTimeAt means a sync was observed.
	SyncTimeAt
)

// FileState represents the state of a file in a conflict.
type FileState struct {
	Kind   string  `json:"kind"`
	Digest *string `json:"digest,omitempty"`
}

// Change represents a file change in a conflict.
type Change struct {
	Path string     `json:"path"`
	Old  *FileState `json:"old,omitempty"`
	New  *FileState `json:"new,omitempty"`
}

// Conflict represents a sync conflict between endpoints.
type Conflict struct {
	Root         string   `json:"root"`
	AlphaChanges []Change `json:"alphaChanges"`
	BetaChanges  []Change `json:"betaChanges"`
}

// StagingProgress represents the progress of staging operations.
type StagingProgress struct {
	Path              *string `json:"path,omitempty"`
	ReceivedSize      *uint64 `json:"receivedSize,omitempty"`
	ExpectedSize      *uint64 `json:"expectedSize,omitempty"`
	ReceivedFiles     *uint64 `json:"receivedFiles,omitempty"`
	ExpectedFiles     *uint64 `json:"expectedFiles,omitempty"`
	TotalReceivedSize *uint64 `json:"totalReceivedSize,omitempty"`
}

// Endpoint represents a sync endpoint (local or remote).
type Endpoint struct {
	Protocol        string           `json:"protocol"`
	Path            string           `json:"path"`
	Host            *string          `json:"host,omitempty"`
	Connected       bool             `json:"connected"`
	Scanned         bool             `json:"scanned"`
	Directories     *uint64          `json:"directories,omitempty"`
	Files           *uint64          `json:"files,omitempty"`
	SymbolicLinks   *uint64          `json:"symbolicLinks,omitempty"`
	TotalFileSize   *uint64          `json:"totalFileSize,omitempty"`
	StagingProgress *StagingProgress `json:"stagingProgress,omitempty"`
}

// DisplayPath returns the endpoint path with host prefix if remote.
func (e *Endpoint) DisplayPath() string {
	path := e.PathWithTilde()
	if e.Host != nil {
		return *e.Host + ":" + path
	}
	return path
}

// PathWithTilde replaces the home directory prefix with ~ for display.
func (e *Endpoint) PathWithTilde() string {
	home := homeDir()
	if home != "" && len(e.Path) >= len(home) && e.Path[:len(home)] == home {
		return "~" + e.Path[len(home):]
	}
	return e.Path
}

// StatusIcon returns a visual indicator for the endpoint connection status.
func (e *Endpoint) StatusIcon() string {
	if !e.Connected {
		return "⊗"
	}
	if !e.Scanned {
		return "⟳"
	}
	return "✓"
}

// SyncSession represents a Mutagen sync session.
type SyncSession struct {
	Name             string            `json:"name"`
	Identifier       string            `json:"identifier"`
	Labels           map[string]string `json:"labels"`
	Alpha            Endpoint          `json:"alpha"`
	Beta             Endpoint          `json:"beta"`
	Status           string            `json:"status"`
	Paused           bool              `json:"paused"`
	Mode             *string           `json:"mode,omitempty"`
	CreationTime     *string           `json:"creationTime,omitempty"`
	SuccessfulCycles *uint64           `json:"successfulCycles,omitempty"`
	Conflicts        []Conflict        `json:"conflicts"`
	SyncTime         SyncTime          `json:"-"` // Not from JSON, tracked internally
}

// GetLabel returns the value of a label, or empty string if not found.
func (s *SyncSession) GetLabel(key string) string {
	if s.Labels == nil {
		return ""
	}
	return s.Labels[key]
}

// HasConflicts returns true if the session has any conflicts.
func (s *SyncSession) HasConflicts() bool {
	return len(s.Conflicts) > 0
}

// ConflictCount returns the number of conflicts.
func (s *SyncSession) ConflictCount() int {
	return len(s.Conflicts)
}

// AlphaDisplay returns the alpha endpoint display string.
func (s *SyncSession) AlphaDisplay() string {
	return s.Alpha.DisplayPath()
}

// BetaDisplay returns the beta endpoint display string.
func (s *SyncSession) BetaDisplay() string {
	return s.Beta.DisplayPath()
}

// StatusIcon returns a compact icon representing the session status.
func (s *SyncSession) StatusIcon() string {
	status := strings.ToLower(s.Status)
	switch {
	case strings.Contains(status, "watching"):
		return "👁"
	case strings.Contains(status, "scanning"):
		return "🔍"
	case strings.Contains(status, "staging"):
		return "📦"
	case strings.Contains(status, "reconcil"):
		return "⚖"
	case strings.Contains(status, "saving"):
		return "💾"
	case strings.Contains(status, "connect"):
		return "🔌"
	case strings.Contains(status, "transition"):
		return "⏳"
	case strings.Contains(status, "halt"):
		return "⛔"
	default:
		return "•"
	}
}

// StatusText returns a human-readable status description.
func (s *SyncSession) StatusText() string {
	status := strings.ToLower(s.Status)
	switch {
	case strings.Contains(status, "watching"):
		return "Watching"
	case strings.Contains(status, "scanning"):
		return s.scanningStatusText()
	case strings.Contains(status, "staging"):
		return s.stagingStatusText()
	case strings.Contains(status, "reconcil"):
		return "Reconciling"
	case strings.Contains(status, "saving"):
		return "Saving"
	case strings.Contains(status, "waiting"):
		return "Waiting"
	case strings.Contains(status, "connect"):
		return "Connecting"
	case strings.Contains(status, "transition"):
		return "Transitioning"
	case strings.Contains(status, "halt"):
		return "Halted"
	default:
		return "Unknown"
	}
}

// scanningStatusText returns a detailed scanning status including which endpoint and file count.
func (s *SyncSession) scanningStatusText() string {
	status := strings.ToLower(s.Status)

	// Determine which endpoint is being scanned
	endpoint := ""
	var ep *Endpoint
	if strings.Contains(status, "alpha") {
		endpoint = "α"
		ep = &s.Alpha
	} else if strings.Contains(status, "beta") {
		endpoint = "β"
		ep = &s.Beta
	}

	// Build status with file count if available
	if ep != nil && ep.Files != nil && *ep.Files > 0 {
		return formatScanProgress(endpoint, *ep.Files)
	}
	if endpoint != "" {
		return "Scanning " + endpoint
	}
	return "Scanning"
}

// stagingStatusText returns a detailed staging status including progress if available.
func (s *SyncSession) stagingStatusText() string {
	status := strings.ToLower(s.Status)

	// Determine which endpoint is staging
	endpoint := ""
	var ep *Endpoint
	if strings.Contains(status, "alpha") {
		endpoint = "α"
		ep = &s.Alpha
	} else if strings.Contains(status, "beta") {
		endpoint = "β"
		ep = &s.Beta
	}

	// Check for staging progress
	if ep != nil && ep.StagingProgress != nil {
		prog := ep.StagingProgress
		if prog.ReceivedFiles != nil && prog.ExpectedFiles != nil && *prog.ExpectedFiles > 0 {
			return formatStagingProgress(endpoint, *prog.ReceivedFiles, *prog.ExpectedFiles)
		}
	}
	if endpoint != "" {
		return "Staging " + endpoint
	}
	return "Staging"
}

// formatScanProgress formats a scanning progress message.
func formatScanProgress(endpoint string, files uint64) string {
	if files >= 1000 {
		return "Scanning " + endpoint + " (" + formatNumber(files) + " files)"
	}
	return "Scanning " + endpoint
}

// formatStagingProgress formats a staging progress message.
func formatStagingProgress(endpoint string, received, expected uint64) string {
	pct := (received * 100) / expected
	return "Staging " + endpoint + " (" + formatNumber(received) + "/" + formatNumber(expected) + " " + formatNumber(pct) + "%)"
}

// formatNumber formats a number with commas for readability.
func formatNumber(n uint64) string {
	s := ""
	for n > 0 {
		if s != "" {
			s = "," + s
		}
		if n >= 1000 {
			s = padLeft(n%1000, 3) + s
		} else {
			s = uintToString(n%1000) + s
		}
		n /= 1000
	}
	if s == "" {
		return "0"
	}
	return s
}

func uintToString(n uint64) string {
	if n == 0 {
		return "0"
	}
	digits := ""
	for n > 0 {
		digits = string(rune('0'+n%10)) + digits
		n /= 10
	}
	return digits
}

func padLeft(n uint64, width int) string {
	s := uintToString(n)
	for len(s) < width {
		s = "0" + s
	}
	return s
}

// homeDir returns the user's home directory.
func homeDir() string {
	home, _ := os.UserHomeDir()
	return home
}
