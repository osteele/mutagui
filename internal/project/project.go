// Package project manages Mutagen project files and sync specifications.
package project

import (
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/osteele/mutagui/internal/mutagen"
	"gopkg.in/yaml.v3"
)

// SyncSpecState represents the state of a sync spec.
type SyncSpecState int

const (
	// NotRunning means no session is running for this spec.
	NotRunning SyncSpecState = iota
	// RunningTwoWay means a two-way sync session is running.
	RunningTwoWay
	// RunningPush means a one-way push session is running.
	RunningPush
)

// SessionDefinition represents a session defined in a mutagen.yml file.
type SessionDefinition struct {
	Alpha  string                 `yaml:"alpha"`
	Beta   string                 `yaml:"beta"`
	Mode   *string                `yaml:"mode,omitempty"`
	Ignore *IgnoreConfig          `yaml:"ignore,omitempty"`
	Extra  map[string]interface{} `yaml:",inline"`
}

// IgnoreConfig represents ignore patterns for a session.
type IgnoreConfig struct {
	Paths []string `yaml:"paths,omitempty"`
	VCS   *bool    `yaml:"vcs,omitempty"`
}

// DefaultConfig represents default settings for sessions.
type DefaultConfig struct {
	Ignore *IgnoreConfig          `yaml:"ignore,omitempty"`
	Extra  map[string]interface{} `yaml:",inline"`
}

// ProjectFile represents a parsed mutagen.yml file.
type ProjectFile struct {
	Path       string                       `yaml:"-"`
	TargetName *string                      `yaml:"targetName,omitempty"`
	Sessions   map[string]SessionDefinition `yaml:"sync"`
	Defaults   *DefaultConfig               `yaml:"defaults,omitempty"`
}

// DisplayName returns a user-friendly name for the project file.
func (p *ProjectFile) DisplayName() string {
	if p.TargetName != nil && *p.TargetName != "" {
		return "mutagen-" + *p.TargetName
	}
	// Return the filename without .yml extension
	filename := filepath.Base(p.Path)
	if name := strings.TrimSuffix(filename, ".yml"); name != filename {
		return name
	}
	return filename
}

// SyncSpec represents a sync specification with its current state.
type SyncSpec struct {
	Name           string
	State          SyncSpecState
	RunningSession *mutagen.SyncSession
}

// IsRunning returns true if the spec has a running session.
func (s *SyncSpec) IsRunning() bool {
	return s.State != NotRunning
}

// IsPaused returns true if the spec is running but paused.
func (s *SyncSpec) IsPaused() bool {
	return s.RunningSession != nil && s.RunningSession.Paused
}

// Project represents a mutagen.yml file with its sync specs.
type Project struct {
	File   ProjectFile
	Specs  []SyncSpec
	Folded bool
}

// LoadProjectFile loads and parses a mutagen.yml file.
func LoadProjectFile(path string) (*ProjectFile, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}

	var pf ProjectFile
	if err := yaml.Unmarshal(data, &pf); err != nil {
		return nil, err
	}
	pf.Path = path

	// Extract defaults from sessions map if present (mutagen.yml has sync.defaults)
	if pf.Sessions != nil {
		if defaultSession, exists := pf.Sessions["defaults"]; exists {
			pf.Defaults = &DefaultConfig{
				Ignore: defaultSession.Ignore,
				Extra:  defaultSession.Extra,
			}
			delete(pf.Sessions, "defaults")
		}
	}

	return &pf, nil
}

// NewProject creates a Project from a ProjectFile.
func NewProject(file ProjectFile) *Project {
	specs := make([]SyncSpec, 0, len(file.Sessions))
	for name := range file.Sessions {
		specs = append(specs, SyncSpec{
			Name:  name,
			State: NotRunning,
		})
	}

	// Sort specs by name for deterministic ordering (map iteration is random)
	sort.Slice(specs, func(i, j int) bool {
		return specs[i].Name < specs[j].Name
	})

	return &Project{
		File:   file,
		Specs:  specs,
		Folded: true, // Start folded by default
	}
}

// UserConfigPaths returns paths that are always searched for mutagen project files.
// These are the standard user configuration directories.
func UserConfigPaths() []string {
	home, err := os.UserHomeDir()
	if err != nil {
		return []string{}
	}

	return []string{
		filepath.Join(home, ".config", "mutagen", "projects"),
		filepath.Join(home, ".mutagen", "projects"),
	}
}

// FindProjects searches for mutagen.yml files starting from baseDir and additional search paths.
// Uses a limited depth search to avoid scanning the entire filesystem.
// baseDir is searched first (like --project-dir), then additional config search paths,
// and finally the user config directories (~/.config/mutagen/projects, ~/.mutagen/projects)
// which are always searched.
func FindProjects(baseDir string, configSearchPaths []string, excludePatterns []string) ([]*Project, error) {
	var projects []*Project
	seen := make(map[string]bool)

	// Build the search paths list
	var searchPaths []string

	// Start with base directory (current dir or --project-dir)
	if baseDir != "" {
		searchPaths = append(searchPaths, baseDir)
	}

	// Add config search paths from config file
	searchPaths = append(searchPaths, configSearchPaths...)

	// User config directories where any .yml file is a project
	userConfigDirs := UserConfigPaths()

	// Always add user config directories (these are always searched)
	searchPaths = append(searchPaths, userConfigDirs...)

	// Build set of expanded user config directories for special handling
	userConfigSet := make(map[string]bool)
	for _, ucp := range userConfigDirs {
		expanded := expandPath(ucp)
		if expanded != "" {
			userConfigSet[expanded] = true
		}
	}

	for _, searchPath := range searchPaths {
		searchPath = expandPath(searchPath)
		if searchPath == "" {
			continue
		}

		// Check if path exists
		if _, err := os.Stat(searchPath); os.IsNotExist(err) {
			continue
		}

		// In user config directories, accept any .yml file
		// Elsewhere, only accept mutagen*.yml patterns
		acceptAnyYml := userConfigSet[searchPath]

		// Search with limited depth (max 4 levels)
		findProjectsInDir(searchPath, excludePatterns, &projects, seen, 0, 4, acceptAnyYml)
	}

	return projects, nil
}

// expandPath expands ~ to home directory in a path.
func expandPath(path string) string {
	if strings.HasPrefix(path, "~/") {
		home, err := os.UserHomeDir()
		if err != nil {
			return ""
		}
		return filepath.Join(home, path[2:])
	} else if path == "~" {
		home, err := os.UserHomeDir()
		if err != nil {
			return ""
		}
		return home
	}
	return path
}

// findProjectsInDir searches for mutagen.yml files with depth limiting.
// If acceptAnyYml is true, any .yml file is considered a project file.
func findProjectsInDir(dir string, excludePatterns []string, projects *[]*Project, seen map[string]bool, depth, maxDepth int, acceptAnyYml bool) {
	if depth > maxDepth {
		return
	}

	// Check exclude patterns against directory basename only (not full path)
	// This prevents false positives like "/Users/me/targeting-app" matching pattern "target"
	dirBase := filepath.Base(dir)
	for _, pattern := range excludePatterns {
		if dirBase == pattern {
			return
		}
	}

	entries, err := os.ReadDir(dir)
	if err != nil {
		return
	}

	for _, entry := range entries {
		name := entry.Name()
		path := filepath.Join(dir, name)

		if entry.IsDir() {
			// Skip hidden directories (except .config and .mutagen)
			if strings.HasPrefix(name, ".") && name != ".config" && name != ".mutagen" {
				continue
			}
			// Recurse into subdirectories (don't propagate acceptAnyYml to subdirs)
			findProjectsInDir(path, excludePatterns, projects, seen, depth+1, maxDepth, false)
		} else {
			// Skip lock files
			if strings.HasSuffix(name, ".lock") {
				continue
			}

			// Check if this is a project file
			isProjectFile := false
			if acceptAnyYml {
				// In user config directories, accept any .yml/.yaml file
				isProjectFile = strings.HasSuffix(name, ".yml") || strings.HasSuffix(name, ".yaml")
			} else {
				// Elsewhere, only accept mutagen*.yml patterns
				isProjectFile = name == "mutagen.yml" || name == "mutagen.yaml" ||
					strings.HasPrefix(name, "mutagen-") && (strings.HasSuffix(name, ".yml") || strings.HasSuffix(name, ".yaml"))
			}

			if isProjectFile {
				// Skip if already seen
				absPath, err := filepath.Abs(path)
				if err != nil {
					continue
				}
				if seen[absPath] {
					continue
				}
				seen[absPath] = true

				pf, err := LoadProjectFile(path)
				if err != nil {
					continue // Skip invalid files
				}
				*projects = append(*projects, NewProject(*pf))
			}
		}
	}
}

// UpdateFromSessions updates the project's spec states based on running sessions.
// Matches sessions by name (spec name for two-way, spec-name-push for push sessions).
func (p *Project) UpdateFromSessions(sessions []mutagen.SyncSession) {
	// Create maps of session names to sessions
	sessionByName := make(map[string]*mutagen.SyncSession)
	for i := range sessions {
		session := &sessions[i]
		sessionByName[session.Name] = session
	}

	// Update each spec
	for i := range p.Specs {
		spec := &p.Specs[i]

		// Look for two-way session (exact name match, not one-way-replica mode)
		if session, exists := sessionByName[spec.Name]; exists {
			if session.Mode == nil || *session.Mode != "one-way-replica" {
				spec.RunningSession = session
				spec.State = RunningTwoWay
				continue
			}
		}

		// Look for push session (name-push suffix with one-way-replica mode)
		pushName := spec.Name + "-push"
		if session, exists := sessionByName[pushName]; exists {
			if session.Mode != nil && *session.Mode == "one-way-replica" {
				spec.RunningSession = session
				spec.State = RunningPush
				continue
			}
		}

		// Also check if the exact name is a one-way-replica (legacy push format)
		if session, exists := sessionByName[spec.Name]; exists {
			if session.Mode != nil && *session.Mode == "one-way-replica" {
				spec.RunningSession = session
				spec.State = RunningPush
				continue
			}
		}

		// No matching session found
		spec.State = NotRunning
		spec.RunningSession = nil
	}
}
