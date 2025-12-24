package ui

import "github.com/osteele/mutagui/internal/project"

// SelectableItemType represents the type of item in the selection list.
type SelectableItemType int

const (
	// SelectableProject represents a project header.
	SelectableProject SelectableItemType = iota
	// SelectableSpec represents a sync spec within a project.
	SelectableSpec
)

// SelectableItem represents an item that can be selected in the unified panel.
type SelectableItem struct {
	Type         SelectableItemType
	ProjectIndex int
	SpecIndex    int // Only valid when Type == SelectableSpec
}

// SelectionManager manages selection state in the unified project/spec tree.
type SelectionManager struct {
	items         []SelectableItem
	selectedIndex int
}

// NewSelectionManager creates a new SelectionManager.
func NewSelectionManager() *SelectionManager {
	return &SelectionManager{
		items:         []SelectableItem{},
		selectedIndex: 0,
	}
}

// RebuildFromProjects rebuilds the items list from projects.
func (sm *SelectionManager) RebuildFromProjects(projects []*project.Project) {
	sm.items = sm.items[:0] // Clear but keep capacity

	for projIdx, proj := range projects {
		// Add project header
		sm.items = append(sm.items, SelectableItem{
			Type:         SelectableProject,
			ProjectIndex: projIdx,
		})

		// Add specs if unfolded
		if !proj.Folded {
			for specIdx := range proj.Specs {
				sm.items = append(sm.items, SelectableItem{
					Type:         SelectableSpec,
					ProjectIndex: projIdx,
					SpecIndex:    specIdx,
				})
			}
		}
	}

	// Clamp selection to valid range
	if len(sm.items) > 0 && sm.selectedIndex >= len(sm.items) {
		sm.selectedIndex = len(sm.items) - 1
	} else if len(sm.items) == 0 {
		sm.selectedIndex = 0
	}
}

// TotalItems returns the total number of items.
func (sm *SelectionManager) TotalItems() int {
	return len(sm.items)
}

// RawIndex returns the raw selected index (for UI rendering).
func (sm *SelectionManager) RawIndex() int {
	return sm.selectedIndex
}

// Items returns the list of selectable items.
func (sm *SelectionManager) Items() []SelectableItem {
	return sm.items
}

// SelectedItem returns the currently selected item, or nil if none.
func (sm *SelectionManager) SelectedItem() *SelectableItem {
	if sm.selectedIndex >= 0 && sm.selectedIndex < len(sm.items) {
		return &sm.items[sm.selectedIndex]
	}
	return nil
}

// SelectedProjectIndex returns the index of the selected project.
// For specs, returns the parent project index.
func (sm *SelectionManager) SelectedProjectIndex() int {
	item := sm.SelectedItem()
	if item == nil {
		return -1
	}
	return item.ProjectIndex
}

// SelectedSpec returns the project and spec indices if a spec is selected.
// Returns (-1, -1) if no spec is selected.
func (sm *SelectionManager) SelectedSpec() (int, int) {
	item := sm.SelectedItem()
	if item == nil || item.Type != SelectableSpec {
		return -1, -1
	}
	return item.ProjectIndex, item.SpecIndex
}

// IsProjectSelected returns true if a project is selected (not a spec).
func (sm *SelectionManager) IsProjectSelected() bool {
	item := sm.SelectedItem()
	return item != nil && item.Type == SelectableProject
}

// IsSpecSelected returns true if a spec is selected.
func (sm *SelectionManager) IsSpecSelected() bool {
	item := sm.SelectedItem()
	return item != nil && item.Type == SelectableSpec
}

// SelectNext moves selection to the next item (wraps around).
func (sm *SelectionManager) SelectNext() {
	total := len(sm.items)
	if total > 0 {
		sm.selectedIndex = (sm.selectedIndex + 1) % total
	}
}

// SelectPrevious moves selection to the previous item (wraps around).
func (sm *SelectionManager) SelectPrevious() {
	total := len(sm.items)
	if total > 0 {
		if sm.selectedIndex == 0 {
			sm.selectedIndex = total - 1
		} else {
			sm.selectedIndex--
		}
	}
}

// SetIndex sets the selection directly by raw index.
func (sm *SelectionManager) SetIndex(index int) {
	total := len(sm.items)
	if total > 0 {
		if index >= total {
			sm.selectedIndex = total - 1
		} else if index < 0 {
			sm.selectedIndex = 0
		} else {
			sm.selectedIndex = index
		}
	}
}

// ItemAt returns the item at the given index, or nil if out of bounds.
func (sm *SelectionManager) ItemAt(index int) *SelectableItem {
	if index >= 0 && index < len(sm.items) {
		return &sm.items[index]
	}
	return nil
}
