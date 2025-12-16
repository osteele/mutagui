//! Selection state management for the TUI.
//!
//! This module encapsulates all selection logic, including navigation
//! in a unified panel that shows projects with their sync specs.

use crate::project::Project;

/// Item that can be selected in the unified panel
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectableItem {
    /// A project header (can be folded/unfolded)
    Project { index: usize },
    /// A sync spec within a project
    Spec {
        project_index: usize,
        spec_index: usize,
    },
}

/// Manages selection state in the unified project/spec tree.
///
/// The selection model maintains a flattened list of selectable items
/// (projects and their specs) that gets rebuilt when fold states change.
#[derive(Debug, Clone)]
pub struct SelectionManager {
    /// Flattened list of selectable items (projects and their specs)
    items: Vec<SelectableItem>,
    /// Currently selected index into items
    selected_index: usize,
}

impl Default for SelectionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectionManager {
    /// Create a new SelectionManager with empty list.
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            selected_index: 0,
        }
    }

    /// Rebuild items list from projects
    pub fn rebuild_from_projects(&mut self, projects: &[Project]) {
        self.items.clear();

        for (proj_idx, project) in projects.iter().enumerate() {
            // Add project header
            self.items.push(SelectableItem::Project { index: proj_idx });

            // Add specs if unfolded
            if !project.folded {
                for spec_idx in 0..project.specs.len() {
                    self.items.push(SelectableItem::Spec {
                        project_index: proj_idx,
                        spec_index: spec_idx,
                    });
                }
            }
        }

        // Clamp selection to valid range
        if !self.items.is_empty() && self.selected_index >= self.items.len() {
            self.selected_index = self.items.len() - 1;
        } else if self.items.is_empty() {
            self.selected_index = 0;
        }
    }

    /// Get the total number of items.
    pub fn total_items(&self) -> usize {
        self.items.len()
    }

    /// Get the raw selected index (for UI rendering).
    pub fn raw_index(&self) -> usize {
        self.selected_index
    }

    /// Get an iterator over all items (for UI rendering).
    pub fn items(&self) -> impl Iterator<Item = &SelectableItem> {
        self.items.iter()
    }

    /// Get currently selected item
    pub fn selected_item(&self) -> Option<&SelectableItem> {
        self.items.get(self.selected_index)
    }

    /// Get selected project index (either directly or parent of selected spec)
    pub fn selected_project_index(&self) -> Option<usize> {
        match self.selected_item()? {
            SelectableItem::Project { index } => Some(*index),
            SelectableItem::Spec { project_index, .. } => Some(*project_index),
        }
    }

    /// Get selected spec if any (returns (project_index, spec_index))
    pub fn selected_spec(&self) -> Option<(usize, usize)> {
        match self.selected_item()? {
            SelectableItem::Spec {
                project_index,
                spec_index,
            } => Some((*project_index, *spec_index)),
            _ => None,
        }
    }

    /// Check if a project is selected (not a spec)
    pub fn is_project_selected(&self) -> bool {
        matches!(self.selected_item(), Some(SelectableItem::Project { .. }))
    }

    /// Check if a spec is selected
    pub fn is_spec_selected(&self) -> bool {
        matches!(self.selected_item(), Some(SelectableItem::Spec { .. }))
    }

    /// Move selection to the next item (wraps around).
    pub fn select_next(&mut self) {
        let total = self.total_items();
        if total > 0 {
            self.selected_index = (self.selected_index + 1) % total;
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
        }
    }

    /// Set selection directly by raw index.
    #[cfg(test)]
    pub fn set_index(&mut self, index: usize) {
        let total = self.total_items();
        if total > 0 {
            self.selected_index = index.min(total - 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{Project, ProjectFile, SessionDefinition, SyncSpec, SyncSpecState};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn make_test_project(name: &str, spec_count: usize, folded: bool) -> Project {
        let mut sessions = HashMap::new();
        let mut specs = Vec::new();

        for i in 0..spec_count {
            let spec_name = format!("spec-{}", i);
            sessions.insert(
                spec_name.clone(),
                SessionDefinition {
                    alpha: "/local".to_string(),
                    beta: "server:/remote".to_string(),
                    mode: None,
                    ignore: None,
                },
            );
            specs.push(SyncSpec {
                name: spec_name,
                state: SyncSpecState::NotRunning,
                running_session: None,
            });
        }

        Project {
            file: ProjectFile {
                path: PathBuf::from(format!("/test/{}.yml", name)),
                target_name: None,
                sessions,
                defaults: None,
            },
            specs,
            folded,
        }
    }

    #[test]
    fn test_new_selection_manager() {
        let sel = SelectionManager::new();
        assert_eq!(sel.raw_index(), 0);
        assert_eq!(sel.selected_item(), None);
    }

    #[test]
    fn test_rebuild_from_projects_folded() {
        let mut sel = SelectionManager::new();
        let projects = vec![
            make_test_project("p1", 2, true), // Folded
            make_test_project("p2", 3, true), // Folded
        ];

        sel.rebuild_from_projects(&projects);

        // Only project headers should be in items
        assert_eq!(sel.total_items(), 2);
        assert_eq!(
            sel.selected_item(),
            Some(&SelectableItem::Project { index: 0 })
        );
    }

    #[test]
    fn test_rebuild_from_projects_unfolded() {
        let mut sel = SelectionManager::new();
        let projects = vec![
            make_test_project("p1", 2, false), // Unfolded with 2 specs
            make_test_project("p2", 1, true),  // Folded with 1 spec
        ];

        sel.rebuild_from_projects(&projects);

        // p1 header + 2 specs + p2 header = 4 items
        assert_eq!(sel.total_items(), 4);

        // Items should be: Project(0), Spec(0,0), Spec(0,1), Project(1)
        assert_eq!(sel.items[0], SelectableItem::Project { index: 0 });
        assert_eq!(
            sel.items[1],
            SelectableItem::Spec {
                project_index: 0,
                spec_index: 0
            }
        );
        assert_eq!(
            sel.items[2],
            SelectableItem::Spec {
                project_index: 0,
                spec_index: 1
            }
        );
        assert_eq!(sel.items[3], SelectableItem::Project { index: 1 });
    }

    #[test]
    fn test_select_next_wraps() {
        let mut sel = SelectionManager::new();
        let projects = vec![
            make_test_project("p1", 2, false), // 3 items total
        ];

        sel.rebuild_from_projects(&projects);

        // Start at 0
        assert_eq!(sel.raw_index(), 0);

        sel.select_next();
        assert_eq!(sel.raw_index(), 1);

        sel.select_next();
        assert_eq!(sel.raw_index(), 2);

        // Should wrap to 0
        sel.select_next();
        assert_eq!(sel.raw_index(), 0);
    }

    #[test]
    fn test_select_previous_wraps() {
        let mut sel = SelectionManager::new();
        let projects = vec![make_test_project("p1", 2, false)];

        sel.rebuild_from_projects(&projects);

        // Start at 0, go backwards
        sel.select_previous();
        assert_eq!(sel.raw_index(), 2); // Wrapped to last item
    }

    #[test]
    fn test_selected_project_index() {
        let mut sel = SelectionManager::new();
        let projects = vec![
            make_test_project("p1", 2, false),
            make_test_project("p2", 1, false),
        ];

        sel.rebuild_from_projects(&projects);

        // At project 0 header
        assert_eq!(sel.selected_project_index(), Some(0));

        sel.select_next(); // At spec 0,0
        assert_eq!(sel.selected_project_index(), Some(0)); // Still project 0

        sel.select_next(); // At spec 0,1
        assert_eq!(sel.selected_project_index(), Some(0)); // Still project 0

        sel.select_next(); // At project 1 header
        assert_eq!(sel.selected_project_index(), Some(1));
    }

    #[test]
    fn test_selected_spec() {
        let mut sel = SelectionManager::new();
        let projects = vec![make_test_project("p1", 2, false)];

        sel.rebuild_from_projects(&projects);

        // At project header
        assert_eq!(sel.selected_spec(), None);

        sel.select_next(); // At spec 0,0
        assert_eq!(sel.selected_spec(), Some((0, 0)));

        sel.select_next(); // At spec 0,1
        assert_eq!(sel.selected_spec(), Some((0, 1)));
    }

    #[test]
    fn test_is_project_selected() {
        let mut sel = SelectionManager::new();
        let projects = vec![make_test_project("p1", 1, false)];

        sel.rebuild_from_projects(&projects);

        assert!(sel.is_project_selected());
        assert!(!sel.is_spec_selected());

        sel.select_next();
        assert!(!sel.is_project_selected());
        assert!(sel.is_spec_selected());
    }

    #[test]
    fn test_clamp_on_shrink() {
        let mut sel = SelectionManager::new();
        let projects = vec![make_test_project("p1", 5, false)];

        sel.rebuild_from_projects(&projects);
        sel.set_index(5); // Valid in 6-item list

        // Rebuild with fewer specs
        let projects = vec![make_test_project("p1", 2, false)];
        sel.rebuild_from_projects(&projects);

        assert_eq!(sel.raw_index(), 2); // Clamped to max
    }

    #[test]
    fn test_empty_list_navigation() {
        let mut sel = SelectionManager::new();
        sel.rebuild_from_projects(&[]);

        // Navigation should not panic with empty lists
        sel.select_next();
        sel.select_previous();

        assert_eq!(sel.raw_index(), 0);
        assert_eq!(sel.selected_item(), None);
    }

    #[test]
    fn test_rebuild_preserves_selection_where_possible() {
        let mut sel = SelectionManager::new();
        let projects = vec![make_test_project("p1", 3, false)];

        sel.rebuild_from_projects(&projects);
        sel.set_index(2); // Select spec 0,1

        // Rebuild with same structure
        sel.rebuild_from_projects(&projects);

        // Selection should still be at index 2
        assert_eq!(sel.raw_index(), 2);
        assert_eq!(sel.selected_spec(), Some((0, 1)));
    }
}
