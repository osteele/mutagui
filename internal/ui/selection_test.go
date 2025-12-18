package ui

import (
	"testing"

	"github.com/osteele/mutagui/internal/mutagen"
	"github.com/osteele/mutagui/internal/project"
)

func makeTestProject(name string, specCount int, folded bool) *project.Project {
	specs := make([]project.SyncSpec, specCount)
	sessions := make(map[string]project.SessionDefinition)

	for i := 0; i < specCount; i++ {
		specName := "spec-" + string(rune('a'+i))
		specs[i] = project.SyncSpec{
			Name:  specName,
			State: project.NotRunning,
		}
		sessions[specName] = project.SessionDefinition{
			Alpha: "/local",
			Beta:  "server:/remote",
		}
	}

	return &project.Project{
		File: project.ProjectFile{
			Path:     "/test/" + name + ".yml",
			Sessions: sessions,
		},
		Specs:  specs,
		Folded: folded,
	}
}

func TestNewSelectionManager(t *testing.T) {
	sm := NewSelectionManager()

	if sm.RawIndex() != 0 {
		t.Errorf("RawIndex() = %d, want 0", sm.RawIndex())
	}
	if sm.TotalItems() != 0 {
		t.Errorf("TotalItems() = %d, want 0", sm.TotalItems())
	}
	if sm.SelectedItem() != nil {
		t.Errorf("SelectedItem() = %v, want nil", sm.SelectedItem())
	}
}

func TestSelectionManager_RebuildFromProjects_Folded(t *testing.T) {
	sm := NewSelectionManager()
	projects := []*project.Project{
		makeTestProject("p1", 2, true), // Folded
		makeTestProject("p2", 3, true), // Folded
	}

	sm.RebuildFromProjects(projects)

	// Only project headers should be in items
	if sm.TotalItems() != 2 {
		t.Errorf("TotalItems() = %d, want 2", sm.TotalItems())
	}

	item := sm.SelectedItem()
	if item == nil {
		t.Fatal("SelectedItem() = nil, want non-nil")
	}
	if item.Type != SelectableProject || item.ProjectIndex != 0 {
		t.Errorf("SelectedItem() = {Type: %v, ProjectIndex: %d}, want Project at 0",
			item.Type, item.ProjectIndex)
	}
}

func TestSelectionManager_RebuildFromProjects_Unfolded(t *testing.T) {
	sm := NewSelectionManager()
	projects := []*project.Project{
		makeTestProject("p1", 2, false), // Unfolded with 2 specs
		makeTestProject("p2", 1, true),  // Folded with 1 spec
	}

	sm.RebuildFromProjects(projects)

	// p1 header + 2 specs + p2 header = 4 items
	if sm.TotalItems() != 4 {
		t.Errorf("TotalItems() = %d, want 4", sm.TotalItems())
	}

	items := sm.Items()
	// Items should be: Project(0), Spec(0,0), Spec(0,1), Project(1)
	expected := []SelectableItem{
		{Type: SelectableProject, ProjectIndex: 0},
		{Type: SelectableSpec, ProjectIndex: 0, SpecIndex: 0},
		{Type: SelectableSpec, ProjectIndex: 0, SpecIndex: 1},
		{Type: SelectableProject, ProjectIndex: 1},
	}

	for i, want := range expected {
		got := items[i]
		if got.Type != want.Type || got.ProjectIndex != want.ProjectIndex || got.SpecIndex != want.SpecIndex {
			t.Errorf("items[%d] = %+v, want %+v", i, got, want)
		}
	}
}

func TestSelectionManager_SelectNext_Wraps(t *testing.T) {
	sm := NewSelectionManager()
	projects := []*project.Project{
		makeTestProject("p1", 2, false), // 3 items total
	}
	sm.RebuildFromProjects(projects)

	// Start at 0
	if sm.RawIndex() != 0 {
		t.Errorf("Initial RawIndex() = %d, want 0", sm.RawIndex())
	}

	sm.SelectNext()
	if sm.RawIndex() != 1 {
		t.Errorf("After 1st SelectNext(), RawIndex() = %d, want 1", sm.RawIndex())
	}

	sm.SelectNext()
	if sm.RawIndex() != 2 {
		t.Errorf("After 2nd SelectNext(), RawIndex() = %d, want 2", sm.RawIndex())
	}

	// Should wrap to 0
	sm.SelectNext()
	if sm.RawIndex() != 0 {
		t.Errorf("After wrap, RawIndex() = %d, want 0", sm.RawIndex())
	}
}

func TestSelectionManager_SelectPrevious_Wraps(t *testing.T) {
	sm := NewSelectionManager()
	projects := []*project.Project{
		makeTestProject("p1", 2, false), // 3 items total
	}
	sm.RebuildFromProjects(projects)

	// Start at 0, go backwards
	sm.SelectPrevious()
	if sm.RawIndex() != 2 {
		t.Errorf("After SelectPrevious() from 0, RawIndex() = %d, want 2", sm.RawIndex())
	}
}

func TestSelectionManager_SelectedProjectIndex(t *testing.T) {
	sm := NewSelectionManager()
	projects := []*project.Project{
		makeTestProject("p1", 2, false),
		makeTestProject("p2", 1, false),
	}
	sm.RebuildFromProjects(projects)

	// At project 0 header
	if idx := sm.SelectedProjectIndex(); idx != 0 {
		t.Errorf("At project header, SelectedProjectIndex() = %d, want 0", idx)
	}

	sm.SelectNext() // At spec 0,0
	if idx := sm.SelectedProjectIndex(); idx != 0 {
		t.Errorf("At spec 0,0, SelectedProjectIndex() = %d, want 0", idx)
	}

	sm.SelectNext() // At spec 0,1
	if idx := sm.SelectedProjectIndex(); idx != 0 {
		t.Errorf("At spec 0,1, SelectedProjectIndex() = %d, want 0", idx)
	}

	sm.SelectNext() // At project 1 header
	if idx := sm.SelectedProjectIndex(); idx != 1 {
		t.Errorf("At project 1 header, SelectedProjectIndex() = %d, want 1", idx)
	}
}

func TestSelectionManager_SelectedSpec(t *testing.T) {
	sm := NewSelectionManager()
	projects := []*project.Project{
		makeTestProject("p1", 2, false),
	}
	sm.RebuildFromProjects(projects)

	// At project header
	projIdx, specIdx := sm.SelectedSpec()
	if projIdx != -1 || specIdx != -1 {
		t.Errorf("At project header, SelectedSpec() = (%d, %d), want (-1, -1)", projIdx, specIdx)
	}

	sm.SelectNext() // At spec 0,0
	projIdx, specIdx = sm.SelectedSpec()
	if projIdx != 0 || specIdx != 0 {
		t.Errorf("At spec 0,0, SelectedSpec() = (%d, %d), want (0, 0)", projIdx, specIdx)
	}

	sm.SelectNext() // At spec 0,1
	projIdx, specIdx = sm.SelectedSpec()
	if projIdx != 0 || specIdx != 1 {
		t.Errorf("At spec 0,1, SelectedSpec() = (%d, %d), want (0, 1)", projIdx, specIdx)
	}
}

func TestSelectionManager_IsProjectSelected(t *testing.T) {
	sm := NewSelectionManager()
	projects := []*project.Project{
		makeTestProject("p1", 1, false),
	}
	sm.RebuildFromProjects(projects)

	if !sm.IsProjectSelected() {
		t.Error("At project header, IsProjectSelected() = false, want true")
	}
	if sm.IsSpecSelected() {
		t.Error("At project header, IsSpecSelected() = true, want false")
	}

	sm.SelectNext()
	if sm.IsProjectSelected() {
		t.Error("At spec, IsProjectSelected() = true, want false")
	}
	if !sm.IsSpecSelected() {
		t.Error("At spec, IsSpecSelected() = false, want true")
	}
}

func TestSelectionManager_ClampOnShrink(t *testing.T) {
	sm := NewSelectionManager()
	projects := []*project.Project{
		makeTestProject("p1", 5, false), // 6 items total
	}
	sm.RebuildFromProjects(projects)
	sm.SetIndex(5) // Valid in 6-item list

	// Rebuild with fewer specs
	projects = []*project.Project{
		makeTestProject("p1", 2, false), // 3 items total
	}
	sm.RebuildFromProjects(projects)

	if sm.RawIndex() != 2 {
		t.Errorf("After shrink, RawIndex() = %d, want 2 (clamped to max)", sm.RawIndex())
	}
}

func TestSelectionManager_EmptyListNavigation(t *testing.T) {
	sm := NewSelectionManager()
	sm.RebuildFromProjects([]*project.Project{})

	// Navigation should not panic with empty lists
	sm.SelectNext()
	sm.SelectPrevious()

	if sm.RawIndex() != 0 {
		t.Errorf("After navigation on empty list, RawIndex() = %d, want 0", sm.RawIndex())
	}
	if sm.SelectedItem() != nil {
		t.Errorf("On empty list, SelectedItem() = %v, want nil", sm.SelectedItem())
	}
}

func TestSelectionManager_SetIndex(t *testing.T) {
	sm := NewSelectionManager()
	projects := []*project.Project{
		makeTestProject("p1", 3, false), // 4 items total
	}
	sm.RebuildFromProjects(projects)

	sm.SetIndex(2)
	if sm.RawIndex() != 2 {
		t.Errorf("SetIndex(2), RawIndex() = %d, want 2", sm.RawIndex())
	}

	// Out of bounds should clamp
	sm.SetIndex(100)
	if sm.RawIndex() != 3 {
		t.Errorf("SetIndex(100), RawIndex() = %d, want 3 (clamped)", sm.RawIndex())
	}

	sm.SetIndex(-5)
	if sm.RawIndex() != 0 {
		t.Errorf("SetIndex(-5), RawIndex() = %d, want 0 (clamped)", sm.RawIndex())
	}
}

// Test that specs with running sessions are handled correctly
func TestSelectionManager_WithRunningSessions(t *testing.T) {
	sm := NewSelectionManager()

	proj := makeTestProject("p1", 2, false)
	// Simulate a running session
	proj.Specs[0].State = project.RunningTwoWay
	proj.Specs[0].RunningSession = &mutagen.SyncSession{
		Name:   "spec-a",
		Status: "Watching",
	}

	projects := []*project.Project{proj}
	sm.RebuildFromProjects(projects)

	// Should still have 3 items: project + 2 specs
	if sm.TotalItems() != 3 {
		t.Errorf("TotalItems() = %d, want 3", sm.TotalItems())
	}

	sm.SelectNext() // Move to first spec
	projIdx, specIdx := sm.SelectedSpec()
	if projIdx != 0 || specIdx != 0 {
		t.Errorf("SelectedSpec() = (%d, %d), want (0, 0)", projIdx, specIdx)
	}
}
