package mutagen

import (
	"encoding/json"
	"testing"
)

// ============ Session JSON parsing tests ============

func TestParseSessionsJSON_EmptyArray(t *testing.T) {
	input := "[]"
	var sessions []SyncSession
	err := json.Unmarshal([]byte(input), &sessions)
	if err != nil {
		t.Fatalf("Failed to parse empty array: %v", err)
	}
	if len(sessions) != 0 {
		t.Errorf("Expected 0 sessions, got %d", len(sessions))
	}
}

func TestParseSessionsJSON_SingleSession(t *testing.T) {
	input := `[{
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
	}]`

	var sessions []SyncSession
	err := json.Unmarshal([]byte(input), &sessions)
	if err != nil {
		t.Fatalf("Failed to parse single session: %v", err)
	}

	if len(sessions) != 1 {
		t.Fatalf("Expected 1 session, got %d", len(sessions))
	}

	s := sessions[0]
	if s.Name != "test-session" {
		t.Errorf("Name = %q, want %q", s.Name, "test-session")
	}
	if s.Identifier != "session-123" {
		t.Errorf("Identifier = %q, want %q", s.Identifier, "session-123")
	}
	if s.Paused {
		t.Error("Paused = true, want false")
	}
	if s.Alpha.Path != "/local/path" {
		t.Errorf("Alpha.Path = %q, want %q", s.Alpha.Path, "/local/path")
	}
	if s.Alpha.Protocol != "local" {
		t.Errorf("Alpha.Protocol = %q, want %q", s.Alpha.Protocol, "local")
	}
	if s.Beta.Host == nil || *s.Beta.Host != "server.example.com" {
		t.Errorf("Beta.Host = %v, want %q", s.Beta.Host, "server.example.com")
	}
}

func TestParseSessionsJSON_WithMode(t *testing.T) {
	input := `[{
		"name": "push-session",
		"identifier": "session-456",
		"alpha": {"protocol": "local", "path": "/src"},
		"beta": {"protocol": "ssh", "path": "/dst", "host": "remote"},
		"status": "watching",
		"paused": false,
		"mode": "one-way-replica",
		"conflicts": []
	}]`

	var sessions []SyncSession
	err := json.Unmarshal([]byte(input), &sessions)
	if err != nil {
		t.Fatalf("Failed to parse session with mode: %v", err)
	}

	if sessions[0].Mode == nil {
		t.Fatal("Mode is nil, expected one-way-replica")
	}
	if *sessions[0].Mode != "one-way-replica" {
		t.Errorf("Mode = %q, want %q", *sessions[0].Mode, "one-way-replica")
	}
}

func TestParseSessionsJSON_WithLabels(t *testing.T) {
	input := `[{
		"name": "labeled-session",
		"identifier": "session-789",
		"labels": {
			"project": "/path/to/mutagen.yml",
			"environment": "production"
		},
		"alpha": {"protocol": "local", "path": "/src"},
		"beta": {"protocol": "ssh", "path": "/dst", "host": "remote"},
		"status": "watching",
		"paused": false,
		"conflicts": []
	}]`

	var sessions []SyncSession
	err := json.Unmarshal([]byte(input), &sessions)
	if err != nil {
		t.Fatalf("Failed to parse session with labels: %v", err)
	}

	s := sessions[0]
	if s.GetLabel("project") != "/path/to/mutagen.yml" {
		t.Errorf("Label project = %q, want %q", s.GetLabel("project"), "/path/to/mutagen.yml")
	}
	if s.GetLabel("environment") != "production" {
		t.Errorf("Label environment = %q, want %q", s.GetLabel("environment"), "production")
	}
	if s.GetLabel("nonexistent") != "" {
		t.Errorf("Label nonexistent = %q, want empty string", s.GetLabel("nonexistent"))
	}
}

func TestParseSessionsJSON_WithSuccessfulCycles(t *testing.T) {
	input := `[{
		"name": "test",
		"identifier": "id",
		"alpha": {"protocol": "local", "path": "/a"},
		"beta": {"protocol": "local", "path": "/b"},
		"status": "watching",
		"paused": false,
		"successfulCycles": 42,
		"conflicts": []
	}]`

	var sessions []SyncSession
	err := json.Unmarshal([]byte(input), &sessions)
	if err != nil {
		t.Fatalf("Failed to parse session with successful cycles: %v", err)
	}

	if sessions[0].SuccessfulCycles == nil {
		t.Fatal("SuccessfulCycles is nil")
	}
	if *sessions[0].SuccessfulCycles != 42 {
		t.Errorf("SuccessfulCycles = %d, want 42", *sessions[0].SuccessfulCycles)
	}
}

func TestParseSessionsJSON_WithEndpointStats(t *testing.T) {
	input := `[{
		"name": "test",
		"identifier": "id",
		"alpha": {
			"protocol": "local",
			"path": "/src",
			"connected": true,
			"scanned": true,
			"directories": 100,
			"files": 500,
			"symbolicLinks": 10,
			"totalFileSize": 1048576
		},
		"beta": {"protocol": "ssh", "path": "/dst", "host": "server"},
		"status": "watching",
		"paused": false,
		"conflicts": []
	}]`

	var sessions []SyncSession
	err := json.Unmarshal([]byte(input), &sessions)
	if err != nil {
		t.Fatalf("Failed to parse session with endpoint stats: %v", err)
	}

	a := sessions[0].Alpha
	if a.Directories == nil || *a.Directories != 100 {
		t.Errorf("Alpha.Directories = %v, want 100", a.Directories)
	}
	if a.Files == nil || *a.Files != 500 {
		t.Errorf("Alpha.Files = %v, want 500", a.Files)
	}
	if a.SymbolicLinks == nil || *a.SymbolicLinks != 10 {
		t.Errorf("Alpha.SymbolicLinks = %v, want 10", a.SymbolicLinks)
	}
	if a.TotalFileSize == nil || *a.TotalFileSize != 1048576 {
		t.Errorf("Alpha.TotalFileSize = %v, want 1048576", a.TotalFileSize)
	}
}

func TestParseSessionsJSON_WithStagingProgress(t *testing.T) {
	input := `[{
		"name": "test",
		"identifier": "id",
		"alpha": {
			"protocol": "local",
			"path": "/src",
			"stagingProgress": {
				"path": "large-file.bin",
				"receivedSize": 1024,
				"expectedSize": 4096,
				"receivedFiles": 1,
				"expectedFiles": 10,
				"totalReceivedSize": 2048
			}
		},
		"beta": {"protocol": "ssh", "path": "/dst", "host": "server"},
		"status": "staging",
		"paused": false,
		"conflicts": []
	}]`

	var sessions []SyncSession
	err := json.Unmarshal([]byte(input), &sessions)
	if err != nil {
		t.Fatalf("Failed to parse session with staging progress: %v", err)
	}

	sp := sessions[0].Alpha.StagingProgress
	if sp == nil {
		t.Fatal("StagingProgress is nil")
	}
	if sp.Path == nil || *sp.Path != "large-file.bin" {
		t.Errorf("StagingProgress.Path = %v, want large-file.bin", sp.Path)
	}
	if sp.ReceivedSize == nil || *sp.ReceivedSize != 1024 {
		t.Errorf("StagingProgress.ReceivedSize = %v, want 1024", sp.ReceivedSize)
	}
	if sp.ExpectedSize == nil || *sp.ExpectedSize != 4096 {
		t.Errorf("StagingProgress.ExpectedSize = %v, want 4096", sp.ExpectedSize)
	}
}

func TestParseSessionsJSON_InvalidJSON(t *testing.T) {
	input := "not valid json"
	var sessions []SyncSession
	err := json.Unmarshal([]byte(input), &sessions)
	if err == nil {
		t.Error("Expected error parsing invalid JSON, got nil")
	}
}

// ============ Conflict JSON parsing tests ============

func TestParseConflictJSON_WithDigest(t *testing.T) {
	input := `{
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
	}`

	var conflict Conflict
	err := json.Unmarshal([]byte(input), &conflict)
	if err != nil {
		t.Fatalf("Failed to parse conflict with digest: %v", err)
	}

	if conflict.Root != "test.txt" {
		t.Errorf("Root = %q, want %q", conflict.Root, "test.txt")
	}
	if len(conflict.AlphaChanges) != 1 {
		t.Fatalf("AlphaChanges length = %d, want 1", len(conflict.AlphaChanges))
	}
	if len(conflict.BetaChanges) != 1 {
		t.Fatalf("BetaChanges length = %d, want 1", len(conflict.BetaChanges))
	}

	alphaChange := conflict.AlphaChanges[0]
	if alphaChange.Path != "test.txt" {
		t.Errorf("AlphaChanges[0].Path = %q, want %q", alphaChange.Path, "test.txt")
	}
	if alphaChange.Old != nil {
		t.Errorf("AlphaChanges[0].Old = %v, want nil", alphaChange.Old)
	}
	if alphaChange.New == nil {
		t.Fatal("AlphaChanges[0].New is nil")
	}
	if alphaChange.New.Kind != "file" {
		t.Errorf("AlphaChanges[0].New.Kind = %q, want %q", alphaChange.New.Kind, "file")
	}
	if alphaChange.New.Digest == nil || *alphaChange.New.Digest != "fee7d500607ccbc550c97bd094ddfd2d5f170d0b" {
		t.Errorf("AlphaChanges[0].New.Digest = %v, want fee7d500607ccbc550c97bd094ddfd2d5f170d0b", alphaChange.New.Digest)
	}
}

func TestParseConflictJSON_WithoutDigest(t *testing.T) {
	input := `{
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
	}`

	var conflict Conflict
	err := json.Unmarshal([]byte(input), &conflict)
	if err != nil {
		t.Fatalf("Failed to parse conflict without digest: %v", err)
	}

	if conflict.Root != "config" {
		t.Errorf("Root = %q, want %q", conflict.Root, "config")
	}

	betaChange := conflict.BetaChanges[0]
	if betaChange.Path != "config/mutagen/mutagen-cool30.yml.lock" {
		t.Errorf("BetaChanges[0].Path = %q, want config/mutagen/mutagen-cool30.yml.lock", betaChange.Path)
	}
	if betaChange.New == nil {
		t.Fatal("BetaChanges[0].New is nil")
	}
	if betaChange.New.Kind != "untracked" {
		t.Errorf("BetaChanges[0].New.Kind = %q, want %q", betaChange.New.Kind, "untracked")
	}
	if betaChange.New.Digest != nil {
		t.Errorf("BetaChanges[0].New.Digest = %v, want nil (untracked files have no digest)", betaChange.New.Digest)
	}
}

// ============ Real mutagen output parsing test ============

func TestParseSessionsJSON_RealMutagenOutput(t *testing.T) {
	// This is real output from `mutagen sync list --template '{{json .}}'`
	input := `[{
		"identifier": "sync_57LG6c0xgObFZlWfvrZd8J7c5ia21hA91oa8gWr0G5W",
		"version": 1,
		"creationTime": "2025-12-17T05:24:51.866018Z",
		"creatingVersion": "0.18.1",
		"alpha": {
			"protocol": "local",
			"path": "/Users/osteele/code/research",
			"ignore": {},
			"symlink": {},
			"watch": {},
			"permissions": {},
			"compression": {},
			"connected": true,
			"scanned": true,
			"directories": 4277,
			"files": 74713,
			"symbolicLinks": 10,
			"totalFileSize": 9458538761,
			"scanProblems": [
				{
					"path": "bucket-ans-gpu/.direnv/python-3.11/bin/python3",
					"error": "invalid symbolic link: target is absolute"
				}
			]
		},
		"beta": {
			"protocol": "ssh",
			"host": "studio",
			"path": "~/code/research",
			"ignore": {},
			"symlink": {},
			"watch": {},
			"permissions": {},
			"compression": {},
			"connected": true,
			"scanned": true,
			"directories": 4277,
			"files": 74713,
			"symbolicLinks": 10,
			"totalFileSize": 9458538761
		},
		"mode": "one-way-replica",
		"ignore": {
			"paths": [".env", ".venv", "node_modules", "__pycache__"]
		},
		"symlink": {},
		"watch": {},
		"permissions": {},
		"compression": {},
		"name": "studio-research-push",
		"paused": false,
		"status": "watching",
		"successfulCycles": 323
	}]`

	var sessions []SyncSession
	err := json.Unmarshal([]byte(input), &sessions)
	if err != nil {
		t.Fatalf("Failed to parse real mutagen output: %v", err)
	}

	if len(sessions) != 1 {
		t.Fatalf("Expected 1 session, got %d", len(sessions))
	}

	s := sessions[0]

	// Check basic fields
	if s.Name != "studio-research-push" {
		t.Errorf("Name = %q, want %q", s.Name, "studio-research-push")
	}
	if s.Identifier != "sync_57LG6c0xgObFZlWfvrZd8J7c5ia21hA91oa8gWr0G5W" {
		t.Errorf("Identifier mismatch")
	}
	if s.Status != "watching" {
		t.Errorf("Status = %q, want %q", s.Status, "watching")
	}
	if s.Paused {
		t.Error("Paused = true, want false")
	}

	// Check mode
	if s.Mode == nil || *s.Mode != "one-way-replica" {
		t.Errorf("Mode = %v, want one-way-replica", s.Mode)
	}

	// Check alpha endpoint
	if s.Alpha.Protocol != "local" {
		t.Errorf("Alpha.Protocol = %q, want local", s.Alpha.Protocol)
	}
	if s.Alpha.Path != "/Users/osteele/code/research" {
		t.Errorf("Alpha.Path = %q, want /Users/osteele/code/research", s.Alpha.Path)
	}
	if !s.Alpha.Connected {
		t.Error("Alpha.Connected = false, want true")
	}
	if !s.Alpha.Scanned {
		t.Error("Alpha.Scanned = false, want true")
	}
	if s.Alpha.Directories == nil || *s.Alpha.Directories != 4277 {
		t.Errorf("Alpha.Directories = %v, want 4277", s.Alpha.Directories)
	}
	if s.Alpha.Files == nil || *s.Alpha.Files != 74713 {
		t.Errorf("Alpha.Files = %v, want 74713", s.Alpha.Files)
	}

	// Check beta endpoint
	if s.Beta.Protocol != "ssh" {
		t.Errorf("Beta.Protocol = %q, want ssh", s.Beta.Protocol)
	}
	if s.Beta.Host == nil || *s.Beta.Host != "studio" {
		t.Errorf("Beta.Host = %v, want studio", s.Beta.Host)
	}
	if s.Beta.Path != "~/code/research" {
		t.Errorf("Beta.Path = %q, want ~/code/research", s.Beta.Path)
	}

	// Check successful cycles
	if s.SuccessfulCycles == nil || *s.SuccessfulCycles != 323 {
		t.Errorf("SuccessfulCycles = %v, want 323", s.SuccessfulCycles)
	}

	// Check creation time
	if s.CreationTime == nil || *s.CreationTime != "2025-12-17T05:24:51.866018Z" {
		t.Errorf("CreationTime = %v, want 2025-12-17T05:24:51.866018Z", s.CreationTime)
	}
}

func TestParseSessionsJSON_MultipleSessions(t *testing.T) {
	input := `[
		{
			"name": "session1",
			"identifier": "id1",
			"alpha": {"protocol": "local", "path": "/a"},
			"beta": {"protocol": "local", "path": "/b"},
			"status": "watching",
			"paused": false,
			"conflicts": []
		},
		{
			"name": "session2",
			"identifier": "id2",
			"alpha": {"protocol": "local", "path": "/c"},
			"beta": {"protocol": "ssh", "path": "/d", "host": "remote"},
			"status": "scanning",
			"paused": true,
			"conflicts": []
		}
	]`

	var sessions []SyncSession
	err := json.Unmarshal([]byte(input), &sessions)
	if err != nil {
		t.Fatalf("Failed to parse multiple sessions: %v", err)
	}

	if len(sessions) != 2 {
		t.Fatalf("Expected 2 sessions, got %d", len(sessions))
	}

	if sessions[0].Name != "session1" {
		t.Errorf("sessions[0].Name = %q, want session1", sessions[0].Name)
	}
	if sessions[1].Name != "session2" {
		t.Errorf("sessions[1].Name = %q, want session2", sessions[1].Name)
	}
	if sessions[0].Paused {
		t.Error("sessions[0].Paused = true, want false")
	}
	if !sessions[1].Paused {
		t.Error("sessions[1].Paused = false, want true")
	}
}

// ============ Session with conflicts test ============

func TestParseSessionsJSON_WithConflicts(t *testing.T) {
	input := `[{
		"name": "conflicted-session",
		"identifier": "id",
		"alpha": {"protocol": "local", "path": "/a"},
		"beta": {"protocol": "local", "path": "/b"},
		"status": "watching",
		"paused": false,
		"conflicts": [
			{
				"root": "file1.txt",
				"alphaChanges": [{"path": "file1.txt", "new": {"kind": "file"}}],
				"betaChanges": [{"path": "file1.txt", "new": {"kind": "file"}}]
			},
			{
				"root": "file2.txt",
				"alphaChanges": [{"path": "file2.txt", "new": {"kind": "file"}}],
				"betaChanges": [{"path": "file2.txt", "new": {"kind": "file"}}]
			}
		]
	}]`

	var sessions []SyncSession
	err := json.Unmarshal([]byte(input), &sessions)
	if err != nil {
		t.Fatalf("Failed to parse session with conflicts: %v", err)
	}

	s := sessions[0]
	if !s.HasConflicts() {
		t.Error("HasConflicts() = false, want true")
	}
	if s.ConflictCount() != 2 {
		t.Errorf("ConflictCount() = %d, want 2", s.ConflictCount())
	}
	if s.Conflicts[0].Root != "file1.txt" {
		t.Errorf("Conflicts[0].Root = %q, want file1.txt", s.Conflicts[0].Root)
	}
}
