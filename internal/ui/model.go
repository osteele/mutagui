package ui

import (
	"context"
	"fmt"
	"os"
	"strings"
	"time"

	"github.com/charmbracelet/bubbles/key"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/osteele/mutagui/internal/mutagen"
	"github.com/osteele/mutagui/internal/project"
)

// Modal represents which modal is currently shown.
type Modal int

const (
	ModalNone Modal = iota
	ModalHelp
	ModalConflicts
	ModalSyncStatus
)

// StatusMessageType represents the type of status message.
type StatusMessageType int

const (
	StatusInfo StatusMessageType = iota
	StatusWarning
	StatusError
)

// StatusMessage represents a status message to display.
type StatusMessage struct {
	Type StatusMessageType
	Text string
}

// Model is the Bubble Tea model for the application.
type Model struct {
	// UI state
	Theme       Theme
	Width       int
	Height      int
	ActiveModal Modal

	// Application state
	Projects      []*project.Project
	Selection     *SelectionManager
	StatusMessage *StatusMessage
	LastRefresh   *time.Time
	ShowPaths     bool

	// Async operation state
	IsLoading   bool
	LoadingText string

	// Callbacks for operations (set by main)
	OnRefresh          func(ctx context.Context) error
	OnStart            func(ctx context.Context)
	OnTerminate        func(ctx context.Context)
	OnFlush            func(ctx context.Context)
	OnPause            func(ctx context.Context)
	OnResume           func(ctx context.Context)
	OnPush             func(ctx context.Context)
	OnPushConflicts    func(ctx context.Context)
	OnToggleFold       func(projIdx int)
	OnOpenEditor       func(projIdx int) error
	GetConflicts       func() []SessionConflicts
	GetSelectedSession func() *mutagen.SyncSession

	// For terminal editor support
	SuspendAndRun func(func()) tea.Cmd
}

// KeyMap defines the key bindings.
type KeyMap struct {
	Up          key.Binding
	Down        key.Binding
	Left        key.Binding
	Right       key.Binding
	Enter       key.Binding
	Quit        key.Binding
	Help        key.Binding
	Refresh     key.Binding
	Start       key.Binding
	Terminate   key.Binding
	Flush       key.Binding
	Pause       key.Binding
	Resume      key.Binding
	Push        key.Binding
	Conflicts   key.Binding
	SyncStatus  key.Binding
	Edit        key.Binding
	ToggleMode  key.Binding
	PushToBeta  key.Binding
	Escape      key.Binding
}

// DefaultKeyMap returns the default key bindings.
func DefaultKeyMap() KeyMap {
	return KeyMap{
		Up: key.NewBinding(
			key.WithKeys("up", "k"),
			key.WithHelp("↑/k", "up"),
		),
		Down: key.NewBinding(
			key.WithKeys("down", "j"),
			key.WithHelp("↓/j", "down"),
		),
		Left: key.NewBinding(
			key.WithKeys("left", "h"),
			key.WithHelp("←/h", "fold"),
		),
		Right: key.NewBinding(
			key.WithKeys("right", "l"),
			key.WithHelp("→/l", "unfold"),
		),
		Enter: key.NewBinding(
			key.WithKeys("enter"),
			key.WithHelp("↵", "toggle fold"),
		),
		Quit: key.NewBinding(
			key.WithKeys("q", "ctrl+c"),
			key.WithHelp("q", "quit"),
		),
		Help: key.NewBinding(
			key.WithKeys("?"),
			key.WithHelp("?", "help"),
		),
		Refresh: key.NewBinding(
			key.WithKeys("r"),
			key.WithHelp("r", "refresh"),
		),
		Start: key.NewBinding(
			key.WithKeys("s"),
			key.WithHelp("s", "start"),
		),
		Terminate: key.NewBinding(
			key.WithKeys("t"),
			key.WithHelp("t", "terminate"),
		),
		Flush: key.NewBinding(
			key.WithKeys("f"),
			key.WithHelp("f", "flush"),
		),
		Pause: key.NewBinding(
			key.WithKeys("p", " "),
			key.WithHelp("p/space", "pause/resume"),
		),
		Resume: key.NewBinding(
			key.WithKeys("u"),
			key.WithHelp("u", "resume"),
		),
		Push: key.NewBinding(
			key.WithKeys("P"),
			key.WithHelp("P", "push"),
		),
		Conflicts: key.NewBinding(
			key.WithKeys("c"),
			key.WithHelp("c", "conflicts"),
		),
		SyncStatus: key.NewBinding(
			key.WithKeys("i"),
			key.WithHelp("i", "sync status"),
		),
		Edit: key.NewBinding(
			key.WithKeys("e"),
			key.WithHelp("e", "edit"),
		),
		ToggleMode: key.NewBinding(
			key.WithKeys("m"),
			key.WithHelp("m", "toggle mode"),
		),
		PushToBeta: key.NewBinding(
			key.WithKeys("b"),
			key.WithHelp("b", "push to beta"),
		),
		Escape: key.NewBinding(
			key.WithKeys("esc"),
			key.WithHelp("esc", "close"),
		),
	}
}

var keys = DefaultKeyMap()

// NewModel creates a new Model with the given theme.
func NewModel(theme Theme) Model {
	return Model{
		Theme:     theme,
		Selection: NewSelectionManager(),
		Projects:  []*project.Project{},
	}
}

// Init implements tea.Model.
func (m Model) Init() tea.Cmd {
	return nil
}

// Message types for async operations.
type (
	RefreshDoneMsg     struct{ Err error }
	OperationDoneMsg   struct{ Err error }
	TickMsg            time.Time
	EditorSuspendMsg   struct{ ProjIdx int }
	ClearFlashMsg      struct{}
)

// Update implements tea.Model.
func (m Model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		return m.handleKeyPress(msg)

	case tea.MouseMsg:
		return m.handleMouseEvent(msg)

	case tea.WindowSizeMsg:
		m.Width = msg.Width
		m.Height = msg.Height
		return m, nil

	case RefreshDoneMsg:
		m.IsLoading = false
		m.LoadingText = ""
		if msg.Err != nil {
			m.StatusMessage = &StatusMessage{Type: StatusError, Text: msg.Err.Error()}
			return m, m.flashCmd()
		}
		return m, nil

	case OperationDoneMsg:
		m.IsLoading = false
		m.LoadingText = ""
		if msg.Err != nil {
			m.StatusMessage = &StatusMessage{Type: StatusError, Text: msg.Err.Error()}
		}
		return m, m.flashCmd()

	case ClearFlashMsg:
		// Clear non-error status messages after timeout
		if m.StatusMessage != nil && m.StatusMessage.Type == StatusInfo {
			m.StatusMessage = nil
		}
		return m, nil

	case TickMsg:
		// Auto-refresh tick
		if m.OnRefresh != nil {
			return m, m.refreshCmd()
		}
		return m, nil
	}

	return m, nil
}

func (m Model) handleMouseEvent(msg tea.MouseMsg) (tea.Model, tea.Cmd) {
	// Only handle clicks when no modal is open
	if m.ActiveModal != ModalNone {
		return m, nil
	}

	// Only handle left click
	if msg.Button != tea.MouseButtonLeft || msg.Action != tea.MouseActionRelease {
		return m, nil
	}

	// Calculate list area (accounting for header and border)
	// Header: 1 line (styled title)
	// List border top: 1 line
	// List title: 1 line
	// First item starts at row 3 (0-indexed)
	listTop := 3
	listBottom := m.Height - 6 // Status (3) + Help (3)

	// Check if click is in list area
	if msg.Y >= listTop && msg.Y < listBottom {
		clickedIndex := msg.Y - listTop
		if clickedIndex >= 0 && clickedIndex < m.Selection.TotalItems() {
			// Check if clicking on a project header to toggle fold
			item := m.Selection.ItemAt(clickedIndex)
			if item != nil && item.Type == SelectableProject {
				// If clicking on already selected project, toggle fold
				if clickedIndex == m.Selection.RawIndex() && m.OnToggleFold != nil {
					m.OnToggleFold(item.ProjectIndex)
					m.Selection.RebuildFromProjects(m.Projects)
					return m, nil
				}
			}
			// Select the clicked item
			m.Selection.SetIndex(clickedIndex)
		}
	}

	return m, nil
}

func (m Model) handleKeyPress(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	// Handle escape to close modals
	if key.Matches(msg, keys.Escape) {
		if m.ActiveModal != ModalNone {
			m.ActiveModal = ModalNone
			return m, nil
		}
	}

	// Handle modal-specific keys
	if m.ActiveModal != ModalNone {
		return m.handleModalKeyPress(msg)
	}

	// Global keys
	switch {
	case key.Matches(msg, keys.Quit):
		return m, tea.Quit

	case key.Matches(msg, keys.Help):
		m.ActiveModal = ModalHelp
		return m, nil

	case key.Matches(msg, keys.Up):
		m.Selection.SelectPrevious()
		return m, nil

	case key.Matches(msg, keys.Down):
		m.Selection.SelectNext()
		return m, nil

	case key.Matches(msg, keys.Left), key.Matches(msg, keys.Right), key.Matches(msg, keys.Enter):
		if projIdx := m.Selection.SelectedProjectIndex(); projIdx >= 0 {
			if m.OnToggleFold != nil {
				m.OnToggleFold(projIdx)
				m.Selection.RebuildFromProjects(m.Projects)
			}
		}
		return m, nil

	case key.Matches(msg, keys.Refresh):
		if m.OnRefresh != nil {
			m.IsLoading = true
			m.LoadingText = "Refreshing..."
			return m, m.refreshCmd()
		}
		return m, nil

	case key.Matches(msg, keys.Start):
		if m.OnStart != nil {
			m.IsLoading = true
			m.LoadingText = "Starting..."
			return m, m.startCmd()
		}
		return m, nil

	case key.Matches(msg, keys.Terminate):
		if m.OnTerminate != nil {
			m.IsLoading = true
			m.LoadingText = "Terminating..."
			return m, m.terminateCmd()
		}
		return m, nil

	case key.Matches(msg, keys.Flush):
		if m.OnFlush != nil {
			m.IsLoading = true
			m.LoadingText = "Flushing..."
			return m, m.flushCmd()
		}
		return m, nil

	case key.Matches(msg, keys.Pause):
		if m.OnPause != nil {
			m.IsLoading = true
			m.LoadingText = "Toggling pause..."
			return m, m.pauseCmd()
		}
		return m, nil

	case key.Matches(msg, keys.Resume):
		if m.OnResume != nil {
			m.IsLoading = true
			m.LoadingText = "Resuming..."
			return m, m.resumeCmd()
		}
		return m, nil

	case key.Matches(msg, keys.Push):
		if m.OnPush != nil {
			m.IsLoading = true
			m.LoadingText = "Creating push session..."
			return m, m.pushCmd()
		}
		return m, nil

	case key.Matches(msg, keys.Conflicts):
		m.ActiveModal = ModalConflicts
		return m, nil

	case key.Matches(msg, keys.SyncStatus):
		m.ActiveModal = ModalSyncStatus
		return m, nil

	case key.Matches(msg, keys.Edit):
		if m.OnOpenEditor != nil {
			projIdx := m.Selection.SelectedProjectIndex()
			if projIdx >= 0 {
				err := m.OnOpenEditor(projIdx)
				if err != nil && m.SuspendAndRun != nil {
					// Terminal editor - need to suspend
					return m, m.SuspendAndRun(func() {
						m.runTerminalEditor(projIdx)
					})
				}
			}
		}
		return m, nil

	case key.Matches(msg, keys.ToggleMode):
		m.ShowPaths = !m.ShowPaths
		return m, nil
	}

	return m, nil
}

func (m Model) handleModalKeyPress(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch m.ActiveModal {
	case ModalHelp:
		if key.Matches(msg, keys.Help) || key.Matches(msg, keys.Escape) {
			m.ActiveModal = ModalNone
		}
		return m, nil

	case ModalConflicts:
		if key.Matches(msg, keys.Conflicts) || key.Matches(msg, keys.Escape) {
			m.ActiveModal = ModalNone
			return m, nil
		}
		if key.Matches(msg, keys.PushToBeta) && m.OnPushConflicts != nil {
			m.ActiveModal = ModalNone // Close the modal
			m.IsLoading = true
			m.LoadingText = "Pushing to beta..."
			return m, m.pushConflictsCmd()
		}
		return m, nil

	case ModalSyncStatus:
		if key.Matches(msg, keys.SyncStatus) || key.Matches(msg, keys.Escape) {
			m.ActiveModal = ModalNone
		}
		return m, nil
	}

	return m, nil
}

// Command functions
func (m Model) refreshCmd() tea.Cmd {
	return func() tea.Msg {
		ctx := context.Background()
		err := m.OnRefresh(ctx)
		return RefreshDoneMsg{Err: err}
	}
}

func (m Model) startCmd() tea.Cmd {
	return func() tea.Msg {
		ctx := context.Background()
		m.OnStart(ctx)
		if m.OnRefresh != nil {
			m.OnRefresh(ctx)
		}
		return OperationDoneMsg{}
	}
}

func (m Model) terminateCmd() tea.Cmd {
	return func() tea.Msg {
		ctx := context.Background()
		m.OnTerminate(ctx)
		if m.OnRefresh != nil {
			m.OnRefresh(ctx)
		}
		return OperationDoneMsg{}
	}
}

func (m Model) flushCmd() tea.Cmd {
	return func() tea.Msg {
		ctx := context.Background()
		m.OnFlush(ctx)
		if m.OnRefresh != nil {
			m.OnRefresh(ctx)
		}
		return OperationDoneMsg{}
	}
}

func (m Model) pauseCmd() tea.Cmd {
	return func() tea.Msg {
		ctx := context.Background()
		m.OnPause(ctx)
		if m.OnRefresh != nil {
			m.OnRefresh(ctx)
		}
		return OperationDoneMsg{}
	}
}

func (m Model) resumeCmd() tea.Cmd {
	return func() tea.Msg {
		ctx := context.Background()
		m.OnResume(ctx)
		if m.OnRefresh != nil {
			m.OnRefresh(ctx)
		}
		return OperationDoneMsg{}
	}
}

func (m Model) pushCmd() tea.Cmd {
	return func() tea.Msg {
		ctx := context.Background()
		m.OnPush(ctx)
		if m.OnRefresh != nil {
			m.OnRefresh(ctx)
		}
		return OperationDoneMsg{}
	}
}

func (m Model) pushConflictsCmd() tea.Cmd {
	return func() tea.Msg {
		ctx := context.Background()
		m.OnPushConflicts(ctx)
		if m.OnRefresh != nil {
			m.OnRefresh(ctx)
		}
		return OperationDoneMsg{}
	}
}

// flashCmd returns a command that clears the status message after a delay.
func (m Model) flashCmd() tea.Cmd {
	return tea.Tick(3*time.Second, func(t time.Time) tea.Msg {
		return ClearFlashMsg{}
	})
}

func (m Model) runTerminalEditor(projIdx int) {
	if projIdx < 0 || projIdx >= len(m.Projects) {
		return
	}
	proj := m.Projects[projIdx]
	editor := os.Getenv("EDITOR")
	if editor == "" {
		editor = "vim"
	}

	// This runs in the suspended terminal
	fmt.Printf("Opening %s in %s...\n", proj.File.Path, editor)
}

// TickCmd returns a command that sends tick messages for auto-refresh.
func TickCmd(interval time.Duration) tea.Cmd {
	return tea.Tick(interval, func(t time.Time) tea.Msg {
		return TickMsg(t)
	})
}

// View implements tea.Model.
func (m Model) View() string {
	if m.Width == 0 || m.Height == 0 {
		return "Loading..."
	}

	// Calculate available height for list
	headerHeight := 3
	statusHeight := 3
	helpHeight := 3
	listHeight := m.Height - headerHeight - statusHeight - helpHeight

	// Build sections
	header := m.renderHeader()
	list := m.renderList(listHeight)
	status := m.renderStatus()
	help := m.renderHelp()

	// Main view
	mainView := lipgloss.JoinVertical(lipgloss.Left,
		header,
		list,
		status,
		help,
	)

	// Overlay modal if active
	if m.ActiveModal != ModalNone {
		modal := m.renderModal()
		mainView = m.overlayModal(mainView, modal)
	}

	return mainView
}

func (m Model) renderHeader() string {
	title := m.Theme.HeaderTitle.Render("Mutagen TUI")
	return m.Theme.Header.Width(m.Width - 2).Render(title)
}

func (m Model) renderList(height int) string {
	// Calculate counts
	totalSpecs := 0
	for _, proj := range m.Projects {
		totalSpecs += len(proj.Specs)
	}

	title := fmt.Sprintf(" Sync Projects (%d projects, %d specs) ", len(m.Projects), totalSpecs)

	// Available width for content (account for border padding)
	contentWidth := m.Width - 6
	if contentWidth < 40 {
		contentWidth = 40
	}

	// Build list items
	var items []string
	for i, item := range m.Selection.Items() {
		selected := i == m.Selection.RawIndex()
		var line string
		switch item.Type {
		case SelectableProject:
			proj := m.Projects[item.ProjectIndex]
			line = m.renderProjectHeader(proj, contentWidth, selected)
		case SelectableSpec:
			proj := m.Projects[item.ProjectIndex]
			spec := &proj.Specs[item.SpecIndex]
			line = m.renderSpecRow(proj, spec, contentWidth, selected)
		}

		// Apply selection styling with full width
		if selected {
			// Use Width() on style to ensure full-width background
			line = m.Theme.SelectedItem.Width(contentWidth).Render(line)
		}
		items = append(items, line)
	}

	// Join items and pad to fill height
	content := strings.Join(items, "\n")
	innerHeight := height - 2 // Account for border
	lines := strings.Split(content, "\n")
	for len(lines) < innerHeight {
		lines = append(lines, "")
	}
	if len(lines) > innerHeight {
		lines = lines[:innerHeight]
	}
	content = strings.Join(lines, "\n")

	return m.Theme.ListBorder.
		Width(m.Width - 2).
		Height(height - 2).
		Render(m.Theme.ListTitle.Render(title) + "\n" + content)
}

func (m Model) renderProjectHeader(proj *project.Project, maxWidth int, selected bool) string {
	foldIcon := "▼"
	if proj.Folded {
		foldIcon = "▶"
	}

	// Count running/paused specs
	runningCount := 0
	pausedCount := 0
	conflictCount := 0
	disconnectedCount := 0

	for _, spec := range proj.Specs {
		if spec.IsRunning() {
			runningCount++
		}
		if spec.IsPaused() {
			pausedCount++
		}
		if spec.RunningSession != nil {
			conflictCount += spec.RunningSession.ConflictCount()
			if !spec.RunningSession.Alpha.Connected || !spec.RunningSession.Beta.Connected {
				disconnectedCount++
			}
		}
	}

	// Status icon
	statusIcon := "○"
	statusStyle := m.Theme.StatusNotRunning
	if runningCount > 0 {
		statusIcon = "✓"
		statusStyle = m.Theme.StatusRunning
	}

	// Build status text
	var statusText string
	if runningCount == 0 {
		statusText = "Not running"
	} else if pausedCount == runningCount {
		statusText = "Paused"
	} else if runningCount == len(proj.Specs) {
		statusText = "Running"
	} else {
		statusText = fmt.Sprintf("%d/%d running", runningCount, len(proj.Specs))
	}

	if disconnectedCount > 0 {
		statusText += fmt.Sprintf(", %d waiting", disconnectedCount)
	}

	// Conflict suffix
	conflictSuffix := ""
	if conflictCount > 0 {
		conflictText := "conflict"
		if conflictCount > 1 {
			conflictText = "conflicts"
		}
		conflictSuffix = fmt.Sprintf("  ⚠ %d %s", conflictCount, conflictText)
	}

	// Build line with fixed-width name column
	name := fmt.Sprintf("%-26s", truncateString(proj.File.DisplayName(), 26))

	// Compose line - use plain text when selected so background applies uniformly
	var line string
	if selected {
		line = fmt.Sprintf("%s %s %s  %s%s",
			foldIcon,
			statusIcon,
			name,
			statusText,
			conflictSuffix,
		)
	} else {
		line = fmt.Sprintf("%s %s %s  %s%s",
			foldIcon,
			statusStyle.Render(statusIcon),
			m.Theme.SessionName.Bold(true).Render(name),
			statusText,
			m.Theme.StatusPaused.Bold(true).Render(conflictSuffix),
		)
	}

	return truncateLine(line, maxWidth)
}

func (m Model) renderSpecRow(proj *project.Project, spec *project.SyncSpec, maxWidth int, selected bool) string {
	indent := "    "

	switch spec.State {
	case project.NotRunning:
		sessionDef, exists := proj.File.Sessions[spec.Name]
		name := fmt.Sprintf("%-28s", truncateString(spec.Name, 28))

		if !exists || !m.ShowPaths {
			var line string
			if selected {
				line = fmt.Sprintf("%s%s %s Not running", indent, "○", name)
			} else {
				line = fmt.Sprintf("%s%s %s Not running",
					indent,
					m.Theme.StatusNotRunning.Render("○"),
					m.Theme.SessionName.Render(name),
				)
			}
			return truncateLine(line, maxWidth)
		}

		var line string
		if selected {
			line = fmt.Sprintf("%s%s %s %s ⇄ %s",
				indent, "○", name,
				applyTilde(sessionDef.Alpha),
				applyTilde(sessionDef.Beta),
			)
		} else {
			line = fmt.Sprintf("%s%s %s %s ⇄ %s",
				indent,
				m.Theme.StatusNotRunning.Render("○"),
				m.Theme.SessionName.Render(name),
				m.Theme.SessionAlpha.Render(applyTilde(sessionDef.Alpha)),
				m.Theme.SessionBeta.Render(applyTilde(sessionDef.Beta)),
			)
		}
		return truncateLine(line, maxWidth)

	case project.RunningTwoWay, project.RunningPush:
		if spec.RunningSession == nil {
			var line string
			if selected {
				line = fmt.Sprintf("%s%s %s", indent, "▶", spec.Name)
			} else {
				line = fmt.Sprintf("%s%s %s",
					indent,
					m.Theme.StatusRunning.Render("▶"),
					m.Theme.SessionName.Render(spec.Name),
				)
			}
			return truncateLine(line, maxWidth)
		}

		session := spec.RunningSession

		// Use ▶ for running, ⚠ for conflicts (replaces status icon)
		statusIcon := "▶"
		statusStyle := m.Theme.StatusRunning
		if session.HasConflicts() {
			statusIcon = "⚠"
			statusStyle = m.Theme.StatusPaused
		} else if session.Paused {
			statusIcon = "⏸"
			statusStyle = m.Theme.StatusPaused
		}

		nameWithMode := spec.Name
		if spec.State == project.RunningPush {
			nameWithMode = spec.Name + " (one-way)"
		}
		name := fmt.Sprintf("%-28s", truncateString(nameWithMode, 28))

		var line string
		if m.ShowPaths {
			arrow := "⇄"
			if spec.State == project.RunningPush {
				arrow = "⬆"
			}

			alphaPath := session.Alpha.StatusIcon() + session.AlphaDisplay()
			betaPath := session.Beta.StatusIcon() + session.BetaDisplay()

			if selected {
				line = fmt.Sprintf("%s%s %s %s %s %s %s",
					indent, statusIcon, name,
					session.StatusIcon(),
					alphaPath, arrow, betaPath,
				)
			} else {
				line = fmt.Sprintf("%s%s %s %s %s %s %s",
					indent,
					statusStyle.Render(statusIcon),
					m.Theme.SessionName.Render(name),
					session.StatusIcon(),
					m.Theme.SessionAlpha.Render(alphaPath),
					arrow,
					m.Theme.SessionBeta.Render(betaPath),
				)
			}
		} else {
			statusText := session.StatusText()
			cyclesInfo := ""
			if session.SuccessfulCycles != nil && *session.SuccessfulCycles > 0 {
				cyclesInfo = fmt.Sprintf(" (%d cycles)", *session.SuccessfulCycles)
			}
			if selected {
				line = fmt.Sprintf("%s%s %s %s %s%s",
					indent, statusIcon, name,
					session.StatusIcon(),
					statusText, cyclesInfo,
				)
			} else {
				line = fmt.Sprintf("%s%s %s %s %s%s",
					indent,
					statusStyle.Render(statusIcon),
					m.Theme.SessionName.Render(name),
					session.StatusIcon(),
					statusText,
					cyclesInfo,
				)
			}
		}

		// Add conflict count at end if present
		if session.HasConflicts() {
			conflictText := "conflict"
			if session.ConflictCount() > 1 {
				conflictText = "conflicts"
			}
			if selected {
				line += fmt.Sprintf(" %d %s", session.ConflictCount(), conflictText)
			} else {
				line += m.Theme.StatusPaused.Bold(true).Render(fmt.Sprintf(" %d %s", session.ConflictCount(), conflictText))
			}
		}

		return truncateLine(line, maxWidth)
	}

	return truncateLine(indent+spec.Name, maxWidth)
}

func (m Model) renderStatus() string {
	var text string
	style := m.Theme.StatusMessage

	if m.IsLoading {
		text = "⏳ " + m.LoadingText
	} else if m.StatusMessage != nil {
		text = m.StatusMessage.Text
		switch m.StatusMessage.Type {
		case StatusWarning:
			style = m.Theme.StatusWarning
		case StatusError:
			style = m.Theme.StatusError
		}
	} else {
		text = "Ready"
	}

	if m.LastRefresh != nil {
		text += fmt.Sprintf(" | Last refresh: %s", m.LastRefresh.Format("15:04:05"))
	}

	return m.Theme.StatusBar.Width(m.Width - 2).Render(style.Render(text))
}

func (m Model) renderHelp() string {
	var items []string

	items = append(items,
		m.Theme.HelpKey.Render("↑/↓/j/k")+" Nav",
		m.Theme.HelpKey.Render("h/l/↵")+" Fold",
		m.Theme.HelpKey.Render("r")+" Refresh",
		m.Theme.HelpKey.Render("?")+" Help",
	)

	if m.Selection.IsProjectSelected() {
		items = append(items,
			m.Theme.HelpKey.Render("e")+" Edit",
			m.Theme.HelpKey.Render("s")+" Start",
			m.Theme.HelpKey.Render("t")+" Terminate",
			m.Theme.HelpKey.Render("p")+" Pause/Resume",
		)
	} else if m.Selection.IsSpecSelected() {
		items = append(items,
			m.Theme.HelpKey.Render("s")+" Start",
			m.Theme.HelpKey.Render("t")+" Terminate",
			m.Theme.HelpKey.Render("f")+" Flush",
			m.Theme.HelpKey.Render("p")+" Pause/Resume",
			m.Theme.HelpKey.Render("c")+" Conflicts",
		)
	}

	items = append(items, m.Theme.HelpKey.Render("q")+" Quit")

	sep := m.Theme.HelpSep.Render(" | ")
	return m.Theme.HelpBar.Width(m.Width - 2).Render(strings.Join(items, sep))
}

func (m Model) renderModal() string {
	switch m.ActiveModal {
	case ModalHelp:
		return m.renderHelpModal()
	case ModalConflicts:
		return m.renderConflictModal()
	case ModalSyncStatus:
		return m.renderSyncStatusModal()
	}
	return ""
}

func (m Model) renderHelpModal() string {
	content := m.Theme.ModalTitle.Render("NAVIGATION") + "\n"
	content += "  ↑/k, ↓/j        Move selection up/down\n"
	content += "  h/←, l/→/↵      Fold/unfold project\n"
	content += "\n"
	content += m.Theme.ModalTitle.Render("GLOBAL ACTIONS") + "\n"
	content += "  r               Refresh session list\n"
	content += "  m               Toggle display mode\n"
	content += "  q, Ctrl-C       Quit application\n"
	content += "  ?               Toggle this help screen\n"
	content += "\n"
	content += m.Theme.ModalTitle.Render("PROJECT ACTIONS") + "\n"
	content += "  e               Edit project configuration\n"
	content += "  s               Start all specs\n"
	content += "  t               Terminate all specs\n"
	content += "  f               Flush all specs\n"
	content += "  P               Create push sessions\n"
	content += "  p/Space         Pause/resume all specs\n"
	content += "\n"
	content += m.Theme.ModalTitle.Render("SPEC ACTIONS") + "\n"
	content += "  s               Start this spec\n"
	content += "  t               Terminate this spec\n"
	content += "  f               Flush this spec\n"
	content += "  P               Create push session\n"
	content += "  p/Space         Pause/resume spec\n"
	content += "  c               View conflicts\n"
	content += "\n"
	content += m.Theme.ModalHelp.Render("Press ? or Esc to close")

	return m.Theme.ModalBorder.Render(
		m.Theme.ModalTitle.Render(" Mutagen TUI - Keyboard Commands ") + "\n\n" + content,
	)
}

func (m Model) renderConflictModal() string {
	if m.GetConflicts == nil {
		return m.Theme.ModalBorder.Render("No conflicts")
	}

	conflicts := m.GetConflicts()
	totalConflicts := 0
	for _, sc := range conflicts {
		totalConflicts += len(sc.Conflicts)
	}

	if totalConflicts == 0 {
		return m.Theme.ModalBorder.Render(
			m.Theme.ModalTitle.Render(" Conflict Details ") + "\n\n" +
				"No conflicts found\n\n" +
				m.Theme.ModalHelp.Render("Press Esc or 'c' to close"),
		)
	}

	var content strings.Builder
	content.WriteString(m.Theme.HelpKey.Render("Press 'b' to push all conflicts to beta (alpha → beta copy)") + "\n")
	content.WriteString("Press Esc or 'c' to close this view\n\n")

	for _, sc := range conflicts {
		if len(sc.Conflicts) == 0 {
			continue
		}
		if sc.SpecName != "" {
			content.WriteString(m.Theme.SessionName.Bold(true).Render(sc.SpecName) + "\n")
		}
		for _, conflict := range sc.Conflicts {
			m.appendConflictDetails(&content, conflict, sc.Session)
			content.WriteString("\n")
		}
		content.WriteString("\n")
	}

	return m.Theme.ModalBorder.Render(
		m.Theme.ModalTitle.Render(" Conflict Details ") + "\n\n" + content.String(),
	)
}

func (m Model) appendConflictDetails(sb *strings.Builder, conflict mutagen.Conflict, session *mutagen.SyncSession) {
	if session != nil {
		alphaPath := session.AlphaDisplay()
		betaPath := session.BetaDisplay()
		if conflict.Root != "" && conflict.Root != "." {
			if !strings.HasSuffix(alphaPath, "/") {
				alphaPath += "/"
			}
			if !strings.HasSuffix(betaPath, "/") {
				betaPath += "/"
			}
			alphaPath += conflict.Root
			betaPath += conflict.Root
		}

		sb.WriteString(m.Theme.ConflictAlpha.Bold(true).Render("Alpha (α): ") +
			m.Theme.ConflictAlpha.Render(alphaPath) + "\n")
		sb.WriteString(m.Theme.ConflictBeta.Bold(true).Render("Beta (β):  ") +
			m.Theme.ConflictBeta.Render(betaPath) + "\n")
	} else {
		sb.WriteString(m.Theme.SessionName.Bold(true).Render("Root: ") +
			m.Theme.ConflictAlpha.Render(conflict.Root) + "\n")
	}

	sb.WriteString(fmt.Sprintf("  α %d / β %d changes\n",
		len(conflict.AlphaChanges), len(conflict.BetaChanges)))

	if len(conflict.AlphaChanges) > 0 {
		sb.WriteString(m.Theme.ConflictAlpha.Bold(true).Render("  α ") +
			summarizeChanges(conflict.AlphaChanges) + "\n")
	}
	if len(conflict.BetaChanges) > 0 {
		sb.WriteString(m.Theme.ConflictBeta.Bold(true).Render("  β ") +
			summarizeChanges(conflict.BetaChanges) + "\n")
	}
}

func (m Model) renderSyncStatusModal() string {
	if m.GetSelectedSession == nil {
		return m.Theme.ModalBorder.Render("No session selected")
	}

	session := m.GetSelectedSession()
	if session == nil {
		return m.Theme.ModalBorder.Render(
			m.Theme.ModalTitle.Render(" Sync Status ") + "\n\n" +
				"No session selected or session not running\n\n" +
				m.Theme.ModalHelp.Render("Press Esc or 'i' to close"),
		)
	}

	var content strings.Builder
	content.WriteString(m.Theme.HelpKey.Render("Session: ") + session.Name + "\n")
	content.WriteString(m.Theme.HelpKey.Render("Status: ") + session.StatusIcon() + " " + session.Status + "\n")
	if session.Mode != nil {
		content.WriteString(m.Theme.HelpKey.Render("Mode: ") + *session.Mode + "\n")
	}
	content.WriteString(m.Theme.HelpKey.Render("Paused: ") + fmt.Sprintf("%v", session.Paused) + "\n\n")

	// Alpha endpoint
	content.WriteString(m.Theme.ConflictAlpha.Bold(true).Render("Alpha (α):") + "\n")
	content.WriteString(m.formatEndpointDetails(&session.Alpha))

	// Beta endpoint
	content.WriteString(m.Theme.ConflictBeta.Bold(true).Render("Beta (β):") + "\n")
	content.WriteString(m.formatEndpointDetails(&session.Beta))

	// Conflicts
	if session.HasConflicts() {
		content.WriteString(m.Theme.StatusError.Bold(true).Render(fmt.Sprintf("\nConflicts: %d\n", session.ConflictCount())))
	}

	// Successful cycles
	if session.SuccessfulCycles != nil {
		content.WriteString(m.Theme.HelpKey.Render(fmt.Sprintf("\nSuccessful Cycles: %d\n", *session.SuccessfulCycles)))
	}

	content.WriteString("\n" + m.Theme.ModalHelp.Render("Press Esc or 'i' to close"))

	return m.Theme.ModalBorder.Render(
		m.Theme.ModalTitle.Render(" Sync Status ") + "\n\n" + content.String(),
	)
}

func (m Model) formatEndpointDetails(e *mutagen.Endpoint) string {
	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("  %s %s\n", e.StatusIcon(), e.DisplayPath()))
	sb.WriteString(fmt.Sprintf("  Connected: %v, Scanned: %v\n", e.Connected, e.Scanned))

	if e.Directories != nil || e.Files != nil || e.SymbolicLinks != nil {
		counts := "  "
		if e.Directories != nil {
			counts += fmt.Sprintf("Dirs: %d  ", *e.Directories)
		}
		if e.Files != nil {
			counts += fmt.Sprintf("Files: %d  ", *e.Files)
		}
		if e.SymbolicLinks != nil {
			counts += fmt.Sprintf("Links: %d", *e.SymbolicLinks)
		}
		sb.WriteString(counts + "\n")
	}

	if e.TotalFileSize != nil {
		sb.WriteString(fmt.Sprintf("  Total Size: %s\n", formatBytes(*e.TotalFileSize)))
	}

	sb.WriteString("\n")
	return sb.String()
}

func (m Model) overlayModal(base, modal string) string {
	// Center the modal on the screen
	modalLines := strings.Split(modal, "\n")
	modalHeight := len(modalLines)
	modalWidth := 0
	for _, line := range modalLines {
		if lipgloss.Width(line) > modalWidth {
			modalWidth = lipgloss.Width(line)
		}
	}

	// Calculate position
	x := (m.Width - modalWidth) / 2
	y := (m.Height - modalHeight) / 2

	if x < 0 {
		x = 0
	}
	if y < 0 {
		y = 0
	}

	// Build overlay
	baseLines := strings.Split(base, "\n")
	for i := 0; i < modalHeight && y+i < len(baseLines); i++ {
		if y+i >= 0 && y+i < len(baseLines) {
			line := baseLines[y+i]
			modalLine := ""
			if i < len(modalLines) {
				modalLine = modalLines[i]
			}

			// Pad modal line to x position
			prefix := ""
			if x > 0 && x < lipgloss.Width(line) {
				prefix = line[:min(x, len(line))]
			}

			baseLines[y+i] = prefix + modalLine
		}
	}

	return strings.Join(baseLines, "\n")
}

// Helper functions
func applyTilde(endpoint string) string {
	home, err := os.UserHomeDir()
	if err != nil {
		return endpoint
	}

	// Handle SSH-style endpoints
	if colonPos := strings.LastIndex(endpoint, ":"); colonPos >= 0 {
		prefix := endpoint[:colonPos+1]
		path := endpoint[colonPos+1:]
		return prefix + applyTildeToPath(path, home)
	}

	return applyTildeToPath(endpoint, home)
}

func applyTildeToPath(path string, home string) string {
	if home != "" && strings.HasPrefix(path, home) {
		return "~" + path[len(home):]
	}

	if strings.HasPrefix(path, "/Users/") {
		rest := path[7:]
		if slashPos := strings.Index(rest, "/"); slashPos >= 0 {
			username := rest[:slashPos]
			remainder := rest[slashPos:]
			return "~" + username + remainder
		}
		return "~" + rest
	}

	if strings.HasPrefix(path, "/home/") {
		rest := path[6:]
		if slashPos := strings.Index(rest, "/"); slashPos >= 0 {
			username := rest[:slashPos]
			remainder := rest[slashPos:]
			return "~" + username + remainder
		}
		return "~" + rest
	}

	return path
}

func summarizeChanges(changes []mutagen.Change) string {
	const maxDisplay = 3
	if len(changes) == 0 {
		return "No changes"
	}

	var parts []string
	for i, change := range changes {
		if i >= maxDisplay {
			parts = append(parts, fmt.Sprintf("+%d more", len(changes)-maxDisplay))
			break
		}
		parts = append(parts, change.Path)
	}

	return strings.Join(parts, ", ")
}

func formatBytes(b uint64) string {
	const unit = 1024
	if b < unit {
		return fmt.Sprintf("%d B", b)
	}
	div, exp := uint64(unit), 0
	for n := b / unit; n >= unit; n /= unit {
		div *= unit
		exp++
	}
	return fmt.Sprintf("%.1f %cB", float64(b)/float64(div), "KMGTPE"[exp])
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}

// truncateString truncates a string to maxLen characters, adding ... if needed.
func truncateString(s string, maxLen int) string {
	if len(s) <= maxLen {
		return s
	}
	if maxLen <= 3 {
		return s[:maxLen]
	}
	return s[:maxLen-3] + "..."
}

// truncateLine truncates a line to fit within maxWidth, accounting for ANSI codes.
// Adds ellipsis (…) when truncation occurs.
func truncateLine(line string, maxWidth int) string {
	width := lipgloss.Width(line)
	if width <= maxWidth {
		return line
	}

	// Need to truncate - leave room for ellipsis
	targetWidth := maxWidth - 1 // Reserve 1 char for …
	if targetWidth < 1 {
		return "…"
	}

	// Find truncation point
	runes := []rune(line)
	for i := len(runes) - 1; i >= 0; i-- {
		truncated := string(runes[:i])
		if lipgloss.Width(truncated) <= targetWidth {
			return truncated + "…"
		}
	}
	return "…"
}
