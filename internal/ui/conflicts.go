package ui

import "github.com/osteele/mutagui/internal/mutagen"

// SessionConflicts represents the conflicts associated with a spec/session.
type SessionConflicts struct {
	SpecName  string
	Session   *mutagen.SyncSession
	Conflicts []mutagen.Conflict
}
