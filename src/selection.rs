//! Selection state management for the TUI.
//!
//! This module encapsulates all selection logic, including navigation
//! and focus area management, in a single testable type.

/// Manages selection state across projects and sessions lists.
///
/// The selection model treats projects and sessions as a single unified list
/// where projects come first (indices 0..projects_len) followed by sessions
/// (indices projects_len..projects_len+sessions_len).
#[derive(Debug, Clone)]
pub struct SelectionManager {
    /// Number of projects in the list
    projects_len: usize,
    /// Number of sessions in the list
    sessions_len: usize,
    /// Currently selected index in the unified list
    selected_index: usize,
    /// Last selected index in the projects area (for focus restoration)
    last_project_index: Option<usize>,
    /// Last selected session index (relative, not absolute)
    last_session_index: Option<usize>,
    /// Current focus area
    focus_area: FocusArea,
}

/// Which area of the UI has focus
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusArea {
    Projects,
    Sessions,
}

impl Default for SelectionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectionManager {
    /// Create a new SelectionManager with empty lists.
    pub fn new() -> Self {
        Self {
            projects_len: 0,
            sessions_len: 0,
            selected_index: 0,
            last_project_index: None,
            last_session_index: None,
            focus_area: FocusArea::Projects,
        }
    }

    /// Update the list sizes and clamp selection if needed.
    pub fn update_sizes(&mut self, projects_len: usize, sessions_len: usize) {
        self.projects_len = projects_len;
        self.sessions_len = sessions_len;

        // Clamp selection to valid range
        let total = self.total_items();
        if total > 0 && self.selected_index >= total {
            self.selected_index = total - 1;
        }

        // Update focus area based on current selection
        if self.selected_index < projects_len {
            self.focus_area = FocusArea::Projects;
        } else if sessions_len > 0 {
            self.focus_area = FocusArea::Sessions;
        }
    }

    /// Get the total number of items (projects + sessions).
    pub fn total_items(&self) -> usize {
        self.projects_len + self.sessions_len
    }

    /// Get the raw selected index (for UI rendering).
    pub fn raw_index(&self) -> usize {
        self.selected_index
    }

    /// Get the currently selected project index, if a project is selected.
    pub fn selected_project(&self) -> Option<usize> {
        if self.selected_index < self.projects_len {
            Some(self.selected_index)
        } else {
            None
        }
    }

    /// Get the currently selected session index (relative to sessions list).
    pub fn selected_session(&self) -> Option<usize> {
        if self.selected_index >= self.projects_len && self.sessions_len > 0 {
            Some(self.selected_index - self.projects_len)
        } else {
            None
        }
    }

    /// Get the current focus area.
    pub fn focus_area(&self) -> FocusArea {
        self.focus_area
    }

    /// Move selection to the next item (wraps around).
    pub fn select_next(&mut self) {
        let total = self.total_items();
        if total > 0 {
            self.selected_index = (self.selected_index + 1) % total;
            self.update_focus_from_selection();
        }
    }

    /// Move selection to the previous item (wraps around).
    pub fn select_previous(&mut self) {
        let total = self.total_items();
        if total > 0 {
            if self.selected_index == 0 {
                self.selected_index = total - 1;
            } else {
                self.selected_index -= 1;
            }
            self.update_focus_from_selection();
        }
    }

    /// Toggle focus between Projects and Sessions areas.
    ///
    /// This saves the current position before switching and tries to restore
    /// the previous position in the new area, or find a related item.
    pub fn toggle_focus(&mut self) {
        // Save current position
        self.save_current_position();

        // Toggle focus area
        self.focus_area = match self.focus_area {
            FocusArea::Projects => FocusArea::Sessions,
            FocusArea::Sessions => FocusArea::Projects,
        };

        // Determine new selection
        match self.focus_area {
            FocusArea::Projects => {
                if self.projects_len == 0 {
                    // No projects, stay in sessions
                    self.focus_area = FocusArea::Sessions;
                    return;
                }

                // Try to restore last position
                if let Some(last_idx) = self.last_project_index {
                    if last_idx < self.projects_len {
                        self.selected_index = last_idx;
                        return;
                    }
                }

                // Default to first project
                self.selected_index = 0;
            }
            FocusArea::Sessions => {
                if self.sessions_len == 0 {
                    // No sessions, stay in projects
                    self.focus_area = FocusArea::Projects;
                    return;
                }

                // Try to restore last position
                if let Some(last_idx) = self.last_session_index {
                    if last_idx < self.sessions_len {
                        self.selected_index = self.projects_len + last_idx;
                        return;
                    }
                }

                // Default to first session
                self.selected_index = self.projects_len;
            }
        }
    }

    /// Set selection directly by raw index.
    #[cfg(test)]
    pub fn set_index(&mut self, index: usize) {
        let total = self.total_items();
        if total > 0 {
            self.selected_index = index.min(total - 1);
            self.update_focus_from_selection();
        }
    }

    /// Try to select a specific project by index.
    pub fn select_project(&mut self, project_idx: usize) {
        if project_idx < self.projects_len {
            self.selected_index = project_idx;
            self.focus_area = FocusArea::Projects;
        }
    }

    /// Try to select a specific session by index.
    pub fn select_session(&mut self, session_idx: usize) {
        if session_idx < self.sessions_len {
            self.selected_index = self.projects_len + session_idx;
            self.focus_area = FocusArea::Sessions;
        }
    }

    /// Save current position for focus restoration.
    fn save_current_position(&mut self) {
        if let Some(proj_idx) = self.selected_project() {
            self.last_project_index = Some(proj_idx);
        } else if let Some(sess_idx) = self.selected_session() {
            self.last_session_index = Some(sess_idx);
        }
    }

    /// Update focus area based on current selection.
    fn update_focus_from_selection(&mut self) {
        if self.selected_index < self.projects_len {
            self.focus_area = FocusArea::Projects;
        } else {
            self.focus_area = FocusArea::Sessions;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_selection_manager() {
        let sel = SelectionManager::new();
        assert_eq!(sel.raw_index(), 0);
        assert_eq!(sel.selected_project(), None);
        assert_eq!(sel.selected_session(), None);
    }

    #[test]
    fn test_update_sizes() {
        let mut sel = SelectionManager::new();
        sel.update_sizes(3, 5);

        assert_eq!(sel.total_items(), 8);
        assert_eq!(sel.selected_project(), Some(0));
        assert_eq!(sel.selected_session(), None);
    }

    #[test]
    fn test_select_next_wraps() {
        let mut sel = SelectionManager::new();
        sel.update_sizes(2, 3); // 5 total items

        // Start at 0
        assert_eq!(sel.raw_index(), 0);

        // Move to end
        for _ in 0..4 {
            sel.select_next();
        }
        assert_eq!(sel.raw_index(), 4);

        // Should wrap to 0
        sel.select_next();
        assert_eq!(sel.raw_index(), 0);
    }

    #[test]
    fn test_select_previous_wraps() {
        let mut sel = SelectionManager::new();
        sel.update_sizes(2, 3);

        // Start at 0, go backwards
        sel.select_previous();
        assert_eq!(sel.raw_index(), 4); // Wrapped to last item
    }

    #[test]
    fn test_selected_project_vs_session() {
        let mut sel = SelectionManager::new();
        sel.update_sizes(2, 3);

        // Index 0, 1 = projects
        assert_eq!(sel.selected_project(), Some(0));
        assert_eq!(sel.selected_session(), None);

        sel.select_next();
        assert_eq!(sel.selected_project(), Some(1));
        assert_eq!(sel.selected_session(), None);

        sel.select_next();
        // Index 2 = first session
        assert_eq!(sel.selected_project(), None);
        assert_eq!(sel.selected_session(), Some(0));

        sel.select_next();
        assert_eq!(sel.selected_session(), Some(1));
    }

    #[test]
    fn test_clamp_on_shrink() {
        let mut sel = SelectionManager::new();
        sel.update_sizes(5, 5);
        sel.set_index(8); // Valid in 10-item list

        sel.update_sizes(2, 2); // Shrink to 4 items

        assert_eq!(sel.raw_index(), 3); // Clamped to max
    }

    #[test]
    fn test_toggle_focus_saves_position() {
        let mut sel = SelectionManager::new();
        sel.update_sizes(3, 3);

        // Start in projects, move to index 2
        sel.set_index(2);
        assert_eq!(sel.selected_project(), Some(2));

        // Toggle to sessions
        sel.toggle_focus();
        assert_eq!(sel.focus_area(), FocusArea::Sessions);
        assert_eq!(sel.selected_session(), Some(0));

        // Toggle back to projects
        sel.toggle_focus();
        assert_eq!(sel.focus_area(), FocusArea::Projects);
        assert_eq!(sel.selected_project(), Some(2)); // Restored position
    }

    #[test]
    fn test_toggle_focus_empty_sessions() {
        let mut sel = SelectionManager::new();
        sel.update_sizes(3, 0); // No sessions

        sel.toggle_focus(); // Try to go to sessions

        // Should stay in projects since sessions is empty
        assert_eq!(sel.focus_area(), FocusArea::Projects);
    }

    #[test]
    fn test_toggle_focus_empty_projects() {
        let mut sel = SelectionManager::new();
        sel.update_sizes(0, 3); // No projects

        // Force focus to sessions first
        sel.set_index(0);
        sel.toggle_focus(); // Try to go to projects

        // Should stay in sessions since projects is empty
        assert_eq!(sel.focus_area(), FocusArea::Sessions);
    }

    #[test]
    fn test_select_project_by_index() {
        let mut sel = SelectionManager::new();
        sel.update_sizes(3, 3);
        sel.set_index(5); // Start in sessions

        sel.select_project(1);

        assert_eq!(sel.raw_index(), 1);
        assert_eq!(sel.focus_area(), FocusArea::Projects);
    }

    #[test]
    fn test_select_session_by_index() {
        let mut sel = SelectionManager::new();
        sel.update_sizes(3, 3);
        sel.set_index(0); // Start in projects

        sel.select_session(2);

        assert_eq!(sel.raw_index(), 5); // 3 projects + 2
        assert_eq!(sel.focus_area(), FocusArea::Sessions);
    }

    #[test]
    fn test_focus_updates_on_navigation() {
        let mut sel = SelectionManager::new();
        sel.update_sizes(2, 2);

        assert_eq!(sel.focus_area(), FocusArea::Projects);

        sel.select_next();
        sel.select_next(); // Now at first session
        assert_eq!(sel.focus_area(), FocusArea::Sessions);

        sel.select_previous(); // Back to last project
        assert_eq!(sel.focus_area(), FocusArea::Projects);
    }

    #[test]
    fn test_empty_list_navigation() {
        let mut sel = SelectionManager::new();
        sel.update_sizes(0, 0);

        // Navigation should not panic with empty lists
        sel.select_next();
        sel.select_previous();
        sel.toggle_focus();

        assert_eq!(sel.raw_index(), 0);
    }

    #[test]
    fn test_only_projects() {
        let mut sel = SelectionManager::new();
        sel.update_sizes(3, 0);

        sel.select_next();
        sel.select_next();
        assert_eq!(sel.selected_project(), Some(2));
        assert_eq!(sel.selected_session(), None);
    }

    #[test]
    fn test_only_sessions() {
        let mut sel = SelectionManager::new();
        sel.update_sizes(0, 3);

        assert_eq!(sel.selected_project(), None);
        assert_eq!(sel.selected_session(), Some(0));

        sel.select_next();
        assert_eq!(sel.selected_session(), Some(1));
    }
}
