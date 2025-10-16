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

#[derive(Debug, Clone)]
pub struct ProjectFile {
    pub path: PathBuf,
    pub target_name: Option<String>,
    pub sessions: HashMap<String, SessionDefinition>,
}

impl ProjectFile {
    pub fn from_path(path: PathBuf) -> Result<Self> {
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let yml: MutagenYml = serde_yaml::from_str(&contents)
            .with_context(|| format!("Failed to parse {}", path.display()))?;

        let target_name = extract_target_name(&path);

        let sessions = yml
            .sync
            .and_then(|sync| {
                let mut filtered = HashMap::new();
                for (key, value) in sync.sessions {
                    if key != "defaults" {
                        filtered.insert(key, value);
                    }
                }
                Some(filtered)
            })
            .unwrap_or_default();

        Ok(ProjectFile {
            path,
            target_name,
            sessions,
        })
    }

    pub fn display_name(&self) -> String {
        if let Some(target) = &self.target_name {
            format!("mutagen-{}.yml", target)
        } else {
            self.path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("mutagen.yml")
                .to_string()
        }
    }
}

fn extract_target_name(path: &Path) -> Option<String> {
    path.file_name()
        .and_then(|n| n.to_str())
        .and_then(|name| {
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

pub fn discover_project_files() -> Result<Vec<ProjectFile>> {
    let mut files = Vec::new();
    let search_paths = build_search_paths();

    for pattern in search_paths {
        match glob(&pattern) {
            Ok(paths) => {
                for entry in paths.flatten() {
                    if entry.is_file() {
                        match ProjectFile::from_path(entry.clone()) {
                            Ok(project_file) => files.push(project_file),
                            Err(e) => {
                                eprintln!("Warning: Failed to parse {}: {}", entry.display(), e);
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

fn build_search_paths() -> Vec<String> {
    let mut paths = Vec::new();
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("~"));

    // Current directory patterns
    paths.push(String::from("./mutagen.yml"));
    paths.push(String::from("./mutagen-*.yml"));
    paths.push(String::from("./.mutagen.yml"));
    paths.push(String::from("./.mutagen-*.yml"));

    // Current directory subdirectories
    paths.push(String::from("./mutagen/*.yml"));
    paths.push(String::from("./.mutagen/*.yml"));
    paths.push(String::from("./config/*.yml"));
    paths.push(String::from("./conf/*.yml"));

    // Walk up directory tree looking for project subdirectories
    if let Ok(current_dir) = std::env::current_dir() {
        let mut dir = current_dir.as_path();

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
                if parent == Path::new("/") || dir == Path::new(&home) {
                    break;
                }
                dir = parent;
            } else {
                break;
            }
        }
    }

    // User config directories
    paths.push(format!("{}/.config/mutagen/projects/*.yml", home));
    paths.push(format!("{}/.mutagen/projects/*.yml", home));

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
            let session_name_matches = file.sessions.contains_key(&session.name);

            let alpha_path_matches = file.sessions.values().any(|def| {
                normalize_path(&def.alpha) == normalize_path(&session.alpha.path)
                    || session.alpha.path.contains(&normalize_path(&def.alpha))
            });

            let beta_path_matches = file.sessions.values().any(|def| {
                normalize_path(&def.beta).contains(&normalize_path(&session.beta.path))
                    || session.beta.path.contains(&normalize_path(&def.beta))
            });

            if session_name_matches || (alpha_path_matches && beta_path_matches) {
                active_sessions.push(session.clone());
            }
        }

        projects.push(Project {
            file,
            active_sessions,
        });
    }

    projects
}

fn normalize_path(path: &str) -> String {
    path.trim_end_matches('/').to_string()
}
