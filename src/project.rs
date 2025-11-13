use anyhow::{Context, Result};
use glob::glob;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

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
    pub active_sessions: Vec<SyncSession>,
}

impl Project {
    pub fn is_active(&self) -> bool {
        !self.active_sessions.is_empty()
    }

    pub fn status_icon(&self) -> &str {
        if self.is_active() {
            "✓"
        } else {
            "○"
        }
    }
}

pub fn discover_project_files(base_dir: Option<&Path>) -> Result<Vec<ProjectFile>> {
    let mut files = Vec::new();
    let mut seen_paths = std::collections::HashSet::new();
    let search_paths = build_search_paths(base_dir);

    for pattern in search_paths {
        match glob(&pattern) {
            Ok(paths) => {
                for entry in paths.flatten() {
                    if entry.is_file() {
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

pub fn correlate_projects_with_sessions(
    project_files: Vec<ProjectFile>,
    sessions: &[SyncSession],
) -> Vec<Project> {
    let mut projects = Vec::new();

    for file in project_files {
        let mut active_sessions = Vec::new();

        for session in sessions {
            // Check if session name matches a key in the project file
            // Also check if session name is a push variant (ends with "-push") of a project session
            let session_name_matches = file.sessions.contains_key(&session.name)
                || (session.name.ends_with("-push")
                    && file
                        .sessions
                        .contains_key(&session.name[..session.name.len() - 5]));

            // Normalize session paths once for efficiency
            // Use display_path() to include host prefix (e.g., "cool30:/path") for remote endpoints
            let session_alpha_normalized = normalize_path(&session.alpha.display_path());
            let session_beta_normalized = normalize_path(&session.beta.display_path());

            // Check if any session definition in the project file matches this running session
            let alpha_path_matches = file.sessions.values().any(|def| {
                let def_alpha_normalized = normalize_path(&def.alpha);
                // Use exact equality now that paths are canonicalized
                def_alpha_normalized == session_alpha_normalized
            });

            let beta_path_matches = file.sessions.values().any(|def| {
                let def_beta_normalized = normalize_path(&def.beta);
                // Use exact equality now that paths are canonicalized
                def_beta_normalized == session_beta_normalized
            });

            if session_name_matches || (alpha_path_matches && beta_path_matches) {
                active_sessions.push(session.clone());
            }
        }

        // Sort active sessions alphabetically by name
        active_sessions.sort_by(|a, b| a.name.cmp(&b.name));

        projects.push(Project {
            file,
            active_sessions,
        });
    }

    projects
}

/// Normalizes a path for comparison by resolving it to an absolute canonical path.
/// Handles relative paths, strips trailing slashes, and resolves symlinks.
/// Returns the original path string if canonicalization fails (e.g., for remote paths).
fn normalize_path(path: &str) -> String {
    // Check if this is a Windows drive letter path (e.g., C:\, D:\)
    // Pattern: single letter followed by colon at position 1
    let is_windows_drive = path.len() >= 2
        && path.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && path.chars().nth(1) == Some(':');

    // Remote paths (host:path format) cannot be canonicalized locally
    // But exclude Windows drive letters which should be canonicalized
    if path.contains(':') && !is_windows_drive {
        return path
            .trim_end_matches('/')
            .trim_end_matches('\\')
            .to_string();
    }

    // Try to canonicalize the path
    match std::fs::canonicalize(path) {
        Ok(canonical) => {
            // Convert to string, preserving the path format
            canonical
                .to_string_lossy()
                .trim_end_matches('/')
                .trim_end_matches('\\')
                .to_string()
        }
        Err(_) => {
            // If canonicalization fails (path doesn't exist), try to resolve relative paths manually
            let path_buf = PathBuf::from(path);
            if path_buf.is_relative() {
                // Try to make it absolute relative to current directory
                std::env::current_dir()
                    .ok()
                    .and_then(|cwd| cwd.join(&path_buf).canonicalize().ok())
                    .map(|p| {
                        p.to_string_lossy()
                            .trim_end_matches('/')
                            .trim_end_matches('\\')
                            .to_string()
                    })
                    .unwrap_or_else(|| path.trim_end_matches('/').to_string())
            } else {
                // Already absolute, just normalize
                path.trim_end_matches('/').to_string()
            }
        }
    }
}
