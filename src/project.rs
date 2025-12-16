use anyhow::{Context, Result};
use glob::glob;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::ProjectConfig;
use crate::mutagen::SyncSession;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutagenYml {
    pub sync: Option<SyncDefinitions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncDefinitions {
    #[serde(default)]
    pub defaults: HashMap<String, serde_yaml::Value>,
    #[serde(flatten)]
    pub sessions: HashMap<String, SessionDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDefinition {
    pub alpha: String,
    pub beta: String,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub ignore: Option<serde_yaml::Value>,
}

impl SessionDefinition {
    /// Extract ignore patterns from this session definition, merging with defaults if provided.
    /// Supports:
    /// - Simple list: `ignore: [pattern1, pattern2]`
    /// - Object with paths: `ignore: { paths: [pattern1, pattern2] }`
    /// - VCS ignore: `ignore: { vcs: true }`
    pub fn get_ignore_patterns(&self, defaults: Option<&serde_yaml::Value>) -> Vec<String> {
        let mut patterns = Vec::new();

        // First, extract patterns from defaults if provided
        if let Some(defaults_value) = defaults {
            if let Some(default_ignore) = defaults_value.get("ignore") {
                extract_patterns_from_value(default_ignore, &mut patterns);
            }
        }

        // Then, extract patterns from this session (session-specific overrides defaults)
        if let Some(ignore_value) = &self.ignore {
            extract_patterns_from_value(ignore_value, &mut patterns);
        }

        patterns
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncSpecState {
    /// Spec defined but no session running
    NotRunning,
    /// Running as two-way sync
    RunningTwoWay,
    /// Running as one-way-replica (push)
    RunningPush,
}

/// Represents a sync specification that may or may not be running
#[derive(Debug, Clone)]
pub struct SyncSpec {
    /// Name of the sync spec (from project file key)
    pub name: String,
    /// Current materialization state
    pub state: SyncSpecState,
    /// Link to running session if materialized
    pub running_session: Option<SyncSession>,
}

impl SyncSpec {
    /// Check if this spec has a running session
    pub fn is_running(&self) -> bool {
        self.running_session.is_some()
    }

    /// Get conflicts from running session if any
    pub fn conflicts(&self) -> Option<&Vec<crate::mutagen::Conflict>> {
        self.running_session.as_ref().map(|s| &s.conflicts)
    }

    /// Check if spec has conflicts
    pub fn has_conflicts(&self) -> bool {
        self.conflicts().map(|c| !c.is_empty()).unwrap_or(false)
    }

    /// Check if session is paused
    pub fn is_paused(&self) -> bool {
        self.running_session
            .as_ref()
            .map(|s| s.paused)
            .unwrap_or(false)
    }
}

/// Extract ignore patterns from a YAML value, handling multiple formats
fn extract_patterns_from_value(value: &serde_yaml::Value, patterns: &mut Vec<String>) {
    match value {
        // Simple list: [pattern1, pattern2, ...]
        serde_yaml::Value::Sequence(seq) => {
            for item in seq {
                if let Some(pattern) = item.as_str() {
                    if !patterns.contains(&pattern.to_string()) {
                        patterns.push(pattern.to_string());
                    }
                }
            }
        }
        // Object format: { paths: [...], vcs: true }
        serde_yaml::Value::Mapping(map) => {
            // Handle vcs: true flag
            if let Some(serde_yaml::Value::Bool(true)) = map.get("vcs") {
                // Add common VCS directories (matches Mutagen's behavior)
                for vcs_dir in &[".git", ".svn", ".hg", ".bzr", "_darcs", ".fossil-settings"] {
                    let pattern = vcs_dir.to_string();
                    if !patterns.contains(&pattern) {
                        patterns.push(pattern);
                    }
                }
            }

            // Extract from 'paths' key
            if let Some(serde_yaml::Value::Sequence(paths)) = map.get("paths") {
                for item in paths {
                    if let Some(pattern) = item.as_str() {
                        if !patterns.contains(&pattern.to_string()) {
                            patterns.push(pattern.to_string());
                        }
                    }
                }
            }
            // Note: We don't handle 'regex' patterns here as they require different handling
        }
        _ => {}
    }
}

#[derive(Debug, Clone)]
pub struct ProjectFile {
    pub path: PathBuf,
    pub target_name: Option<String>,
    pub sessions: HashMap<String, SessionDefinition>,
    pub defaults: Option<HashMap<String, serde_yaml::Value>>,
}

impl ProjectFile {
    pub fn from_path(path: PathBuf) -> Result<Self> {
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let yml: MutagenYml = serde_yaml::from_str(&contents)
            .with_context(|| format!("Failed to parse {}", path.display()))?;

        let target_name = extract_target_name(&path);

        let (sessions, defaults) = yml
            .sync
            .map(|sync| {
                let mut filtered = HashMap::new();
                for (key, value) in sync.sessions {
                    if key != "defaults" {
                        filtered.insert(key, value);
                    }
                }
                let defaults = if sync.defaults.is_empty() {
                    None
                } else {
                    Some(sync.defaults)
                };
                (filtered, defaults)
            })
            .unwrap_or_default();

        Ok(ProjectFile {
            path,
            target_name,
            sessions,
            defaults,
        })
    }

    pub fn display_name(&self) -> String {
        if let Some(target) = &self.target_name {
            format!("mutagen-{}", target)
        } else {
            self.path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("mutagen.yml")
                .strip_suffix(".yml")
                .unwrap_or("mutagen")
                .to_string()
        }
    }
}

fn extract_target_name(path: &Path) -> Option<String> {
    path.file_name().and_then(|n| n.to_str()).and_then(|name| {
        if name.starts_with("mutagen-") && name.ends_with(".yml") {
            let target = name
                .strip_prefix("mutagen-")
                .and_then(|s| s.strip_suffix(".yml"));
            target.map(String::from)
        } else if name.starts_with(".mutagen-") && name.ends_with(".yml") {
            let target = name
                .strip_prefix(".mutagen-")
                .and_then(|s| s.strip_suffix(".yml"));
            target.map(String::from)
        } else {
            None
        }
    })
}

#[derive(Debug, Clone)]
pub struct Project {
    pub file: ProjectFile,
    /// All sync specs from project file (both running and not running)
    pub specs: Vec<SyncSpec>,
    /// Whether project tree is folded (collapsed)
    pub folded: bool,
}

pub fn discover_project_files(
    base_dir: Option<&Path>,
    config: Option<&ProjectConfig>,
) -> Result<Vec<ProjectFile>> {
    let mut files = Vec::new();
    let mut seen_paths = std::collections::HashSet::new();
    let mut search_paths = build_search_paths(base_dir);

    // Add custom search paths from config
    if let Some(cfg) = config {
        for path in &cfg.search_paths {
            // Expand tilde in config paths
            let expanded = expand_tilde_in_path(path);
            let path_str = expanded.to_string_lossy();
            search_paths.push(format!("{}/mutagen.yml", path_str));
            search_paths.push(format!("{}/mutagen-*.yml", path_str));
            search_paths.push(format!("{}/.mutagen.yml", path_str));
            search_paths.push(format!("{}/.mutagen-*.yml", path_str));
        }
    }

    // Get exclude patterns from config
    let exclude_patterns: Vec<&str> = config
        .map(|c| c.exclude_patterns.iter().map(|s| s.as_str()).collect())
        .unwrap_or_default();

    for pattern in search_paths {
        match glob(&pattern) {
            Ok(paths) => {
                for entry in paths.flatten() {
                    if entry.is_file() {
                        // Check if path matches any exclude pattern
                        if should_exclude(&entry, &exclude_patterns) {
                            continue;
                        }

                        // Canonicalize path to avoid duplicates from symlinks or different representations
                        let canonical_path = entry.canonicalize().unwrap_or_else(|_| entry.clone());

                        if !seen_paths.contains(&canonical_path) {
                            seen_paths.insert(canonical_path.clone());
                            match ProjectFile::from_path(entry.clone()) {
                                Ok(project_file) => files.push(project_file),
                                Err(e) => {
                                    eprintln!(
                                        "Warning: Failed to parse {}: {}",
                                        entry.display(),
                                        e
                                    );
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Warning: Failed to glob pattern {}: {}", pattern, e);
            }
        }
    }

    Ok(files)
}

/// Expand tilde (~) in a path to the user's home directory.
fn expand_tilde_in_path(path: &Path) -> PathBuf {
    let path_str = path.to_string_lossy();
    if path_str.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            if path_str == "~" {
                return home;
            } else if let Some(rest) = path_str.strip_prefix("~/") {
                return home.join(rest);
            }
        }
    }
    path.to_path_buf()
}

/// Check if a path should be excluded based on patterns.
fn should_exclude(path: &Path, patterns: &[&str]) -> bool {
    if patterns.is_empty() {
        return false;
    }

    // Check if any component of the path matches an exclude pattern
    for component in path.components() {
        let name = component.as_os_str().to_string_lossy();
        for pattern in patterns {
            // Support simple glob-like matching
            if pattern.contains('*') {
                // Simple wildcard matching
                let parts: Vec<&str> = pattern.split('*').collect();
                if parts.len() == 2 {
                    let (prefix, suffix) = (parts[0], parts[1]);
                    if name.starts_with(prefix) && name.ends_with(suffix) {
                        return true;
                    }
                }
            } else if name.contains(*pattern) {
                return true;
            }
        }
    }
    false
}

fn build_search_paths(base_dir: Option<&Path>) -> Vec<String> {
    let mut paths = Vec::new();
    let home = std::env::var("HOME").ok();

    let start_dir = base_dir.unwrap_or_else(|| Path::new("."));
    let start_dir_str = start_dir.to_str().unwrap_or(".");

    // Base directory patterns
    paths.push(format!("{}/mutagen.yml", start_dir_str));
    paths.push(format!("{}/mutagen-*.yml", start_dir_str));
    paths.push(format!("{}/.mutagen.yml", start_dir_str));
    paths.push(format!("{}/.mutagen-*.yml", start_dir_str));

    // Base directory subdirectories - common config locations
    paths.push(format!("{}/mutagen/*.yml", start_dir_str));
    paths.push(format!("{}/.mutagen/*.yml", start_dir_str));
    paths.push(format!("{}/config/mutagen/*.yml", start_dir_str));
    paths.push(format!("{}/conf/mutagen/*.yml", start_dir_str));

    // Direct children only (1 level deep) - for multi-project directories like ~/code
    // This allows finding projects in subdirectories without deep traversal
    paths.push(format!("{}/*/mutagen.yml", start_dir_str));
    paths.push(format!("{}/*/mutagen-*.yml", start_dir_str));
    paths.push(format!("{}/*/.mutagen.yml", start_dir_str));
    paths.push(format!("{}/*/.mutagen-*.yml", start_dir_str));

    // Walk up directory tree looking for project subdirectories
    let walk_start = if let Some(base) = base_dir {
        base.to_path_buf()
    } else if let Ok(current_dir) = std::env::current_dir() {
        current_dir
    } else {
        return paths;
    };

    let mut dir = walk_start.as_path();

    // Walk up to root or home
    loop {
        for subdir in &["mutagen", ".mutagen", "config", "conf"] {
            let subdir_path = dir.join(subdir);
            if subdir_path.is_dir() {
                if let Some(path_str) = subdir_path.to_str() {
                    paths.push(format!("{}/*.yml", path_str));
                }
            }
        }

        // Stop at filesystem root or home directory
        if let Some(parent) = dir.parent() {
            let at_root = parent == Path::new("/");
            let at_home = home.as_ref().is_some_and(|h| dir == Path::new(h));
            if at_root || at_home {
                break;
            }
            dir = parent;
        } else {
            break;
        }
    }

    // User config directories (only if HOME is set)
    if let Some(home_dir) = home {
        paths.push(format!("{}/.config/mutagen/projects/*.yml", home_dir));
        paths.push(format!("{}/.mutagen/projects/*.yml", home_dir));
    }

    paths
}

/// Build sync specs from project file and running sessions
pub fn build_sync_specs(project_file: &ProjectFile, sessions: &[SyncSession]) -> Vec<SyncSpec> {
    let mut specs = Vec::new();

    for name in project_file.sessions.keys() {
        // Find matching running session(s)
        let two_way_session = sessions
            .iter()
            .find(|s| s.name == *name && s.mode.as_deref() != Some("one-way-replica"));

        let push_session = sessions.iter().find(|s| {
            s.name == format!("{}-push", name) && s.mode.as_deref() == Some("one-way-replica")
        });

        // Determine state and attach session
        let (state, running_session) = if let Some(session) = two_way_session {
            (SyncSpecState::RunningTwoWay, Some(session.clone()))
        } else if let Some(session) = push_session {
            (SyncSpecState::RunningPush, Some(session.clone()))
        } else {
            (SyncSpecState::NotRunning, None)
        };

        specs.push(SyncSpec {
            name: name.clone(),
            state,
            running_session,
        });
    }

    // Sort alphabetically
    specs.sort_by(|a, b| a.name.cmp(&b.name));
    specs
}

/// Helper to check if specs should auto-unfold (used during construction)
fn should_auto_unfold_specs(specs: &[SyncSpec]) -> bool {
    // Auto-unfold if any spec has conflicts
    if specs.iter().any(|s| s.has_conflicts()) {
        return true;
    }

    // Auto-unfold if specs are in different states (some running, some not)
    let running_count = specs.iter().filter(|s| s.is_running()).count();
    if running_count > 0 && running_count < specs.len() {
        return true;
    }

    // Auto-unfold if running specs have different modes
    let two_way_count = specs
        .iter()
        .filter(|s| s.state == SyncSpecState::RunningTwoWay)
        .count();
    let push_count = specs
        .iter()
        .filter(|s| s.state == SyncSpecState::RunningPush)
        .count();
    if two_way_count > 0 && push_count > 0 {
        return true;
    }

    false
}

pub fn correlate_projects_with_sessions(
    project_files: Vec<ProjectFile>,
    sessions: &[SyncSession],
) -> Vec<Project> {
    project_files
        .into_iter()
        .map(|file| {
            let specs = build_sync_specs(&file, sessions);
            let should_unfold = should_auto_unfold_specs(&specs);

            Project {
                file,
                specs,
                folded: !should_unfold, // Start unfolded if auto-unfold conditions met
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    // ============ extract_target_name tests ============

    #[test]
    fn test_extract_target_name_standard() {
        let path = Path::new("/some/dir/mutagen-cool30.yml");
        assert_eq!(extract_target_name(path), Some("cool30".to_string()));
    }

    #[test]
    fn test_extract_target_name_hidden() {
        let path = Path::new("/some/dir/.mutagen-server.yml");
        assert_eq!(extract_target_name(path), Some("server".to_string()));
    }

    #[test]
    fn test_extract_target_name_plain_mutagen() {
        let path = Path::new("/some/dir/mutagen.yml");
        assert_eq!(extract_target_name(path), None);
    }

    #[test]
    fn test_extract_target_name_hidden_plain() {
        let path = Path::new("/some/dir/.mutagen.yml");
        assert_eq!(extract_target_name(path), None);
    }

    #[test]
    fn test_extract_target_name_not_yml() {
        let path = Path::new("/some/dir/mutagen-test.yaml");
        assert_eq!(extract_target_name(path), None);
    }

    // ============ ProjectFile tests ============

    #[test]
    fn test_project_file_display_name_with_target() {
        let project = ProjectFile {
            path: PathBuf::from("/test/mutagen-cool30.yml"),
            target_name: Some("cool30".to_string()),
            sessions: HashMap::new(),
            defaults: None,
        };
        assert_eq!(project.display_name(), "mutagen-cool30");
    }

    #[test]
    fn test_project_file_display_name_without_target() {
        let project = ProjectFile {
            path: PathBuf::from("/test/mutagen.yml"),
            target_name: None,
            sessions: HashMap::new(),
            defaults: None,
        };
        assert_eq!(project.display_name(), "mutagen");
    }

    // ============ SessionDefinition tests ============

    #[test]
    fn test_session_definition_get_ignore_patterns_simple_list() {
        let yaml = r#"
            alpha: /local/path
            beta: server:/remote/path
            ignore:
              - "*.log"
              - ".git"
        "#;
        let session: SessionDefinition = serde_yaml::from_str(yaml).unwrap();
        let patterns = session.get_ignore_patterns(None);
        assert_eq!(patterns, vec!["*.log", ".git"]);
    }

    #[test]
    fn test_session_definition_get_ignore_patterns_object_format() {
        let yaml = r#"
            alpha: /local/path
            beta: server:/remote/path
            ignore:
              vcs: true
              paths:
                - "node_modules"
                - "target"
        "#;
        let session: SessionDefinition = serde_yaml::from_str(yaml).unwrap();
        let patterns = session.get_ignore_patterns(None);

        // Should include VCS directories and custom paths
        assert!(patterns.contains(&".git".to_string()));
        assert!(patterns.contains(&".svn".to_string()));
        assert!(patterns.contains(&"node_modules".to_string()));
        assert!(patterns.contains(&"target".to_string()));
    }

    #[test]
    fn test_session_definition_get_ignore_patterns_with_defaults() {
        let yaml = r#"
            alpha: /local/path
            beta: server:/remote/path
            ignore:
              - "session_specific"
        "#;
        let session: SessionDefinition = serde_yaml::from_str(yaml).unwrap();

        let defaults_yaml = r#"
            ignore:
              - "from_defaults"
        "#;
        let defaults: serde_yaml::Value = serde_yaml::from_str(defaults_yaml).unwrap();

        let patterns = session.get_ignore_patterns(Some(&defaults));

        // Should include both default and session-specific patterns
        assert!(patterns.contains(&"from_defaults".to_string()));
        assert!(patterns.contains(&"session_specific".to_string()));
    }

    // ============ discover_project_files tests (using temp directories) ============
    //
    // Note: discover_project_files searches multiple locations including home directories,
    // so these tests check that files ARE found in the temp directory rather than exact counts.

    #[test]
    fn test_discover_project_files_finds_mutagen_yml() {
        let temp_dir = tempfile::tempdir().unwrap();
        let yml_path = temp_dir.path().join("mutagen.yml");

        let mut file = fs::File::create(&yml_path).unwrap();
        writeln!(
            file,
            r#"
sync:
  test-session:
    alpha: /local
    beta: server:/remote
"#
        )
        .unwrap();

        let files = discover_project_files(Some(temp_dir.path()), None).unwrap();
        // Check that our file is found (there may be others from home directories)
        let found = files
            .iter()
            .any(|f| f.path.file_name().unwrap().to_str().unwrap() == "mutagen.yml");
        assert!(found, "Should find mutagen.yml in temp directory");
    }

    #[test]
    fn test_discover_project_files_finds_named_variants() {
        let temp_dir = tempfile::tempdir().unwrap();

        // Create mutagen-server.yml
        let yml_path = temp_dir.path().join("mutagen-server.yml");
        let mut file = fs::File::create(&yml_path).unwrap();
        writeln!(
            file,
            r#"
sync:
  server-session:
    alpha: /local
    beta: server:/remote
"#
        )
        .unwrap();

        let files = discover_project_files(Some(temp_dir.path()), None).unwrap();
        // Check that our named variant is found
        let found = files
            .iter()
            .any(|f| f.target_name.as_deref() == Some("server"));
        assert!(found, "Should find mutagen-server.yml in temp directory");
    }

    #[test]
    fn test_discover_project_files_deduplicates() {
        let temp_dir = tempfile::tempdir().unwrap();

        // Create a single file
        let yml_path = temp_dir.path().join("mutagen.yml");
        let mut file = fs::File::create(&yml_path).unwrap();
        writeln!(
            file,
            r#"
sync:
  test:
    alpha: /local
    beta: server:/remote
"#
        )
        .unwrap();

        let files = discover_project_files(Some(temp_dir.path()), None).unwrap();

        // Count how many times our temp directory file appears (should be exactly 1)
        let temp_file_count = files
            .iter()
            .filter(|f| f.path.starts_with(temp_dir.path()))
            .count();
        assert_eq!(
            temp_file_count, 1,
            "Should find exactly one file from temp directory (deduplication)"
        );
    }

    #[test]
    fn test_discover_project_files_empty_temp_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let files = discover_project_files(Some(temp_dir.path()), None).unwrap();

        // Check that no files from the temp directory are found
        let temp_files: Vec<_> = files
            .iter()
            .filter(|f| f.path.starts_with(temp_dir.path()))
            .collect();
        assert!(
            temp_files.is_empty(),
            "Should find no mutagen files in empty temp directory"
        );
    }

    #[test]
    fn test_discover_project_files_with_exclude_patterns() {
        let temp_dir = tempfile::tempdir().unwrap();

        // Create mutagen.yml in base directory
        let yml_path = temp_dir.path().join("mutagen.yml");
        let mut file = fs::File::create(&yml_path).unwrap();
        writeln!(
            file,
            "sync:\n  test:\n    alpha: /local\n    beta: server:/remote"
        )
        .unwrap();

        // Create a "backup" subdirectory with another mutagen.yml
        let backup_dir = temp_dir.path().join("backup");
        fs::create_dir(&backup_dir).unwrap();
        let backup_yml = backup_dir.join("mutagen.yml");
        let mut backup_file = fs::File::create(&backup_yml).unwrap();
        writeln!(
            backup_file,
            "sync:\n  backup:\n    alpha: /local\n    beta: server:/remote"
        )
        .unwrap();

        // Discover without exclude - should find both
        let files_no_exclude = discover_project_files(Some(temp_dir.path()), None).unwrap();
        let temp_files_no_exclude: Vec<_> = files_no_exclude
            .iter()
            .filter(|f| f.path.starts_with(temp_dir.path()))
            .collect();
        assert!(
            temp_files_no_exclude.len() >= 2,
            "Without exclude, should find at least 2 files"
        );

        // Discover with exclude pattern for "backup"
        let config = ProjectConfig {
            search_paths: vec![],
            exclude_patterns: vec!["backup".to_string()],
        };
        let files_with_exclude =
            discover_project_files(Some(temp_dir.path()), Some(&config)).unwrap();
        let temp_files_with_exclude: Vec<_> = files_with_exclude
            .iter()
            .filter(|f| f.path.starts_with(temp_dir.path()))
            .collect();

        // Should not find the backup directory file
        let has_backup = temp_files_with_exclude
            .iter()
            .any(|f| f.path.to_string_lossy().contains("backup"));
        assert!(
            !has_backup,
            "With exclude pattern, should not find files in backup directory"
        );
    }

    #[test]
    fn test_discover_project_files_with_custom_search_paths() {
        let temp_dir = tempfile::tempdir().unwrap();
        let custom_dir = temp_dir.path().join("custom-projects");
        fs::create_dir(&custom_dir).unwrap();

        // Create mutagen.yml in custom directory
        let yml_path = custom_dir.join("mutagen.yml");
        let mut file = fs::File::create(&yml_path).unwrap();
        writeln!(
            file,
            "sync:\n  custom:\n    alpha: /local\n    beta: server:/remote"
        )
        .unwrap();

        // Discover without custom path - should not find it (searching from temp_dir root)
        let empty_subdir = temp_dir.path().join("empty");
        fs::create_dir(&empty_subdir).unwrap();
        let files_no_custom = discover_project_files(Some(&empty_subdir), None).unwrap();
        let found_custom = files_no_custom
            .iter()
            .any(|f| f.path.to_string_lossy().contains("custom-projects"));
        assert!(
            !found_custom,
            "Without custom path, should not find custom-projects directory"
        );

        // Discover with custom search path
        let config = ProjectConfig {
            search_paths: vec![custom_dir.clone()],
            exclude_patterns: vec![],
        };
        let files_with_custom = discover_project_files(Some(&empty_subdir), Some(&config)).unwrap();
        let found_custom_with_config = files_with_custom
            .iter()
            .any(|f| f.path.to_string_lossy().contains("custom-projects"));
        assert!(
            found_custom_with_config,
            "With custom search path, should find custom-projects directory"
        );
    }

    // ============ correlate_projects_with_sessions tests ============

    fn make_test_session(name: &str, alpha_path: &str, beta_path: &str) -> SyncSession {
        use crate::mutagen::{Endpoint, SyncTime};

        SyncSession {
            name: name.to_string(),
            identifier: format!("id-{}", name),
            alpha: Endpoint {
                protocol: "local".to_string(),
                path: alpha_path.to_string(),
                host: None,
                connected: true,
                scanned: true,
                directories: None,
                files: None,
                symbolic_links: None,
                total_file_size: None,
                staging_progress: None,
            },
            beta: Endpoint {
                protocol: "ssh".to_string(),
                path: beta_path.to_string(),
                host: Some("server".to_string()),
                connected: true,
                scanned: true,
                directories: None,
                files: None,
                symbolic_links: None,
                total_file_size: None,
                staging_progress: None,
            },
            status: "Watching for changes".to_string(),
            paused: false,
            mode: None,
            creation_time: None,
            successful_cycles: None,
            conflicts: vec![],
            sync_time: SyncTime::Unknown,
        }
    }

    #[test]
    fn test_correlate_by_session_name() {
        let mut sessions_map = HashMap::new();
        sessions_map.insert(
            "my-session".to_string(),
            SessionDefinition {
                alpha: "/local/path".to_string(),
                beta: "server:/remote/path".to_string(),
                mode: None,
                ignore: None,
            },
        );

        let project_file = ProjectFile {
            path: PathBuf::from("/test/mutagen.yml"),
            target_name: None,
            sessions: sessions_map,
            defaults: None,
        };

        let running_session =
            make_test_session("my-session", "/different/local", "/different/remote");

        let projects = correlate_projects_with_sessions(vec![project_file], &[running_session]);

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].specs.len(), 1);
        assert_eq!(projects[0].specs[0].name, "my-session");
        assert!(projects[0].specs[0].is_running());
        assert_eq!(projects[0].specs[0].state, SyncSpecState::RunningTwoWay);
    }

    #[test]
    fn test_correlate_by_push_session_name() {
        let mut sessions_map = HashMap::new();
        sessions_map.insert(
            "my-session".to_string(),
            SessionDefinition {
                alpha: "/local/path".to_string(),
                beta: "server:/remote/path".to_string(),
                mode: None,
                ignore: None,
            },
        );

        let project_file = ProjectFile {
            path: PathBuf::from("/test/mutagen.yml"),
            target_name: None,
            sessions: sessions_map,
            defaults: None,
        };

        // Push sessions have "-push" suffix and mode "one-way-replica"
        let mut running_session =
            make_test_session("my-session-push", "/different/local", "/different/remote");
        running_session.mode = Some("one-way-replica".to_string());

        let projects = correlate_projects_with_sessions(vec![project_file], &[running_session]);

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].specs.len(), 1);
        assert_eq!(projects[0].specs[0].name, "my-session");
        assert!(projects[0].specs[0].is_running());
        assert_eq!(projects[0].specs[0].state, SyncSpecState::RunningPush);
    }

    #[test]
    fn test_correlate_no_match() {
        let mut sessions_map = HashMap::new();
        sessions_map.insert(
            "project-session".to_string(),
            SessionDefinition {
                alpha: "/local/path".to_string(),
                beta: "server:/remote/path".to_string(),
                mode: None,
                ignore: None,
            },
        );

        let project_file = ProjectFile {
            path: PathBuf::from("/test/mutagen.yml"),
            target_name: None,
            sessions: sessions_map,
            defaults: None,
        };

        // Different session name and paths
        let running_session =
            make_test_session("unrelated-session", "/other/local", "/other/remote");

        let projects = correlate_projects_with_sessions(vec![project_file], &[running_session]);

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].specs.len(), 1);
        assert_eq!(projects[0].specs[0].name, "project-session");
        assert!(!projects[0].specs[0].is_running());
        assert_eq!(projects[0].specs[0].state, SyncSpecState::NotRunning);
    }

    #[test]
    fn test_correlate_sorts_sessions_alphabetically() {
        let mut sessions_map = HashMap::new();
        sessions_map.insert(
            "zebra".to_string(),
            SessionDefinition {
                alpha: "/local".to_string(),
                beta: "server:/remote".to_string(),
                mode: None,
                ignore: None,
            },
        );
        sessions_map.insert(
            "alpha".to_string(),
            SessionDefinition {
                alpha: "/local".to_string(),
                beta: "server:/remote".to_string(),
                mode: None,
                ignore: None,
            },
        );

        let project_file = ProjectFile {
            path: PathBuf::from("/test/mutagen.yml"),
            target_name: None,
            sessions: sessions_map,
            defaults: None,
        };

        let sessions = vec![
            make_test_session("zebra", "/local", "/remote"),
            make_test_session("alpha", "/local", "/remote"),
        ];

        let projects = correlate_projects_with_sessions(vec![project_file], &sessions);

        assert_eq!(projects[0].specs.len(), 2);
        assert_eq!(projects[0].specs[0].name, "alpha");
        assert_eq!(projects[0].specs[1].name, "zebra");
    }

    // ============ Project tests ============

    #[test]
    fn test_project_is_active() {
        let session = make_test_session("test", "/local", "/remote");
        let spec = SyncSpec {
            name: "test".to_string(),
            state: SyncSpecState::RunningTwoWay,
            running_session: Some(session),
        };

        let project = Project {
            file: ProjectFile {
                path: PathBuf::from("/test/mutagen.yml"),
                target_name: None,
                sessions: HashMap::new(),
                defaults: None,
            },
            specs: vec![spec],
            folded: false,
        };
        assert!(project.specs.iter().any(|s| s.is_running()));
    }

    #[test]
    fn test_project_is_inactive() {
        let spec = SyncSpec {
            name: "test".to_string(),
            state: SyncSpecState::NotRunning,
            running_session: None,
        };

        let project = Project {
            file: ProjectFile {
                path: PathBuf::from("/test/mutagen.yml"),
                target_name: None,
                sessions: HashMap::new(),
                defaults: None,
            },
            specs: vec![spec],
            folded: false,
        };
        assert!(!project.specs.iter().any(|s| s.is_running()));
    }
}
