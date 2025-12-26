package ui

import (
	"fmt"
	"os"
	"strings"
	"time"

	"github.com/gdamore/tcell/v2"
	"github.com/osteele/mutagui/internal/mutagen"
	"github.com/osteele/mutagui/internal/project"
	"github.com/rivo/tview"
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

// AppState holds the state needed by the View (to avoid import cycle with app package).
type AppState struct {
	ColorScheme       ColorScheme
	Projects          []*project.Project
	Selection         *SelectionManager
	StatusMessage     *StatusMessage
	LastRefresh       *time.Time
	ViewingConflicts  bool
	ViewingSyncStatus bool
	ViewingHelp       bool
	ShowPaths         bool
}

// MouseClickHandler is called when a list item is clicked.
// Returns true if the click was handled (e.g., fold toggle).
type MouseClickHandler func(index int) bool

// View manages the TUI display.
type View struct {
	App    *tview.Application
	State  *AppState
	Layout *tview.Flex
	List   *tview.List
	Status *tview.TextView
	Help   *tview.TextView
	Header *tview.TextView
	Modal  *tview.Modal
	Pages  *tview.Pages

	// Callbacks
	onSelectionChanged func(index int)
	onMouseClick       MouseClickHandler

	// Internal state
	isRefreshing bool // Flag to ignore selection changes during refresh
}

// NewView creates a new View.
func NewView(state *AppState) *View {
	// Set global tview styles based on theme
	// This must be done before creating any widgets
	applyGlobalStyles(state.ColorScheme)

	v := &View{
		App:   tview.NewApplication(),
		State: state,
	}

	// Enable mouse support
	v.App.EnableMouse(true)

	v.setupUI()
	return v
}

// applyGlobalStyles sets tview's global Styles based on the color scheme.
func applyGlobalStyles(scheme ColorScheme) {
	// Check if this is a light theme by looking at the text color
	// Light theme uses black text, dark theme uses white text
	if scheme.SessionNameFG == tcell.ColorBlack {
		// Light theme
		tview.Styles.PrimitiveBackgroundColor = tcell.ColorDefault
		tview.Styles.ContrastBackgroundColor = tcell.NewRGBColor(240, 240, 240)
		tview.Styles.MoreContrastBackgroundColor = tcell.NewRGBColor(220, 220, 220)
		tview.Styles.BorderColor = tcell.ColorDarkGray
		tview.Styles.TitleColor = tcell.ColorBlack
		tview.Styles.PrimaryTextColor = tcell.ColorBlack
		tview.Styles.SecondaryTextColor = tcell.ColorDarkGray
		tview.Styles.TertiaryTextColor = tcell.ColorDarkGray
		tview.Styles.InverseTextColor = tcell.ColorWhite
		tview.Styles.ContrastSecondaryTextColor = tcell.ColorDarkGray
	} else {
		// Dark theme - use tview defaults but with ColorDefault background
		tview.Styles.PrimitiveBackgroundColor = tcell.ColorDefault
		tview.Styles.ContrastBackgroundColor = tcell.ColorNavy
		tview.Styles.MoreContrastBackgroundColor = tcell.ColorNavy
		tview.Styles.BorderColor = tcell.ColorWhite
		tview.Styles.TitleColor = tcell.ColorWhite
		tview.Styles.PrimaryTextColor = tcell.ColorWhite
		tview.Styles.SecondaryTextColor = tcell.ColorYellow
		tview.Styles.TertiaryTextColor = tcell.ColorLime
		tview.Styles.InverseTextColor = tcell.ColorBlack
		tview.Styles.ContrastSecondaryTextColor = tcell.ColorAqua
	}
}

// setupUI initializes the UI components.
func (v *View) setupUI() {
	theme := v.State.ColorScheme

	// Header
	v.Header = tview.NewTextView().
		SetDynamicColors(true).
		SetText("[::b]Mutagen TUI[::-]")
	v.Header.SetBorder(true)

	// Main list
	v.List = tview.NewList().
		ShowSecondaryText(false).
		SetHighlightFullLine(true)
	v.List.SetBorder(true).SetTitle(" Sync Projects ")
	v.List.SetSelectedBackgroundColor(theme.SelectionBG)

	// Status bar
	v.Status = tview.NewTextView().
		SetDynamicColors(true).
		SetText("Ready")
	v.Status.SetBorder(true).SetTitle("Status")

	// Help bar
	v.Help = tview.NewTextView().
		SetDynamicColors(true)
	v.Help.SetBorder(true).SetTitle("Help")
	v.UpdateHelpText()

	// Layout
	v.Layout = tview.NewFlex().
		SetDirection(tview.FlexRow).
		AddItem(v.Header, 3, 0, false).
		AddItem(v.List, 0, 1, true).
		AddItem(v.Status, 5, 0, false).
		AddItem(v.Help, 3, 0, false)

	// Pages for modal overlays
	v.Pages = tview.NewPages().
		AddPage("main", v.Layout, true, true)

	v.App.SetRoot(v.Pages, true)

	// Set up list selection change handler
	v.List.SetChangedFunc(func(index int, mainText string, secondaryText string, shortcut rune) {
		// Ignore selection changes during refresh (caused by List.Clear())
		if v.isRefreshing {
			return
		}
		if v.onSelectionChanged != nil {
			v.onSelectionChanged(index)
		}
	})

	// Set up mouse handler for clicks
	v.List.SetMouseCapture(func(action tview.MouseAction, event *tcell.EventMouse) (tview.MouseAction, *tcell.EventMouse) {
		if action == tview.MouseLeftClick {
			// Get the clicked row
			x, y := event.Position()
			// Check if click is within list bounds
			rectX, rectY, width, height := v.List.GetInnerRect()
			if x >= rectX && x < rectX+width && y >= rectY && y < rectY+height {
				clickedIndex := y - rectY
				if clickedIndex >= 0 && clickedIndex < v.List.GetItemCount() {
					// Call the mouse click handler
					if v.onMouseClick != nil {
						if v.onMouseClick(clickedIndex) {
							// Click was handled (e.g., fold toggle), consume event
							return tview.MouseConsumed, nil
						}
					}
				}
			}
		}
		// Let default handler process the event
		return action, event
	})
}

// SetSelectionChangedFunc sets the callback for when list selection changes via mouse.
func (v *View) SetSelectionChangedFunc(handler func(index int)) {
	v.onSelectionChanged = handler
}

// SetMouseClickFunc sets the callback for mouse clicks on list items.
// The handler receives the clicked index and returns true if the click was handled.
func (v *View) SetMouseClickFunc(handler MouseClickHandler) {
	v.onMouseClick = handler
}

// UpdateHelpText updates the help bar based on current selection.
func (v *View) UpdateHelpText() {
	theme := v.State.ColorScheme
	var items []string

	items = append(items, fmt.Sprintf("[%s]↑/↓/j/k[-] Nav", colorToTag(theme.HelpKeyFG)))
	items = append(items, fmt.Sprintf("[%s]h/l/↵[-] Fold", colorToTag(theme.HelpKeyFG)))
	items = append(items, fmt.Sprintf("[%s]r[-] Refresh", colorToTag(theme.HelpKeyFG)))
	items = append(items, fmt.Sprintf("[%s]?[-] Help", colorToTag(theme.HelpKeyFG)))

	if v.State.Selection.IsProjectSelected() {
		items = append(items, fmt.Sprintf("[%s]e[-] Edit", colorToTag(theme.HelpKeyFG)))
		items = append(items, fmt.Sprintf("[%s]s[-] Start", colorToTag(theme.HelpKeyFG)))
		items = append(items, fmt.Sprintf("[%s]t[-] Terminate", colorToTag(theme.HelpKeyFG)))
		items = append(items, fmt.Sprintf("[%s]p[-] Pause/Resume", colorToTag(theme.HelpKeyFG)))
	} else if v.State.Selection.IsSpecSelected() {
		items = append(items, fmt.Sprintf("[%s]s[-] Start", colorToTag(theme.HelpKeyFG)))
		items = append(items, fmt.Sprintf("[%s]t[-] Terminate", colorToTag(theme.HelpKeyFG)))
		items = append(items, fmt.Sprintf("[%s]f[-] Flush", colorToTag(theme.HelpKeyFG)))
		items = append(items, fmt.Sprintf("[%s]p[-] Pause/Resume", colorToTag(theme.HelpKeyFG)))
		items = append(items, fmt.Sprintf("[%s]c[-] Conflicts", colorToTag(theme.HelpKeyFG)))
	}

	items = append(items, fmt.Sprintf("[%s]q[-] Quit", colorToTag(theme.HelpKeyFG)))

	v.Help.SetText(strings.Join(items, " | "))
}

// RefreshList rebuilds the list from the current project state.
func (v *View) RefreshList() {
	// Set flag to ignore selection change callbacks during refresh
	v.isRefreshing = true
	defer func() { v.isRefreshing = false }()

	v.List.Clear()
	theme := v.State.ColorScheme

	totalSpecs := 0
	for _, proj := range v.State.Projects {
		totalSpecs += len(proj.Specs)
	}

	v.List.SetTitle(fmt.Sprintf(" Sync Projects (%d projects, %d specs) ",
		len(v.State.Projects), totalSpecs))

	for _, item := range v.State.Selection.Items() {
		var text string
		switch item.Type {
		case SelectableProject:
			proj := v.State.Projects[item.ProjectIndex]
			text = v.renderProjectHeader(proj, theme)
		case SelectableSpec:
			proj := v.State.Projects[item.ProjectIndex]
			spec := &proj.Specs[item.SpecIndex]
			text = v.renderSpecRow(proj, spec, theme)
		}
		v.List.AddItem(text, "", 0, nil)
	}

	// Set selection
	v.List.SetCurrentItem(v.State.Selection.RawIndex())
	v.UpdateHelpText()
}

// renderProjectHeader renders a project header line.
func (v *View) renderProjectHeader(proj *project.Project, theme ColorScheme) string {
	foldIcon := "▼"
	if proj.Folded {
		foldIcon = "▶"
	}

	// Check if any specs are running
	isActive := false
	runningCount := 0
	pausedCount := 0
	pushCount := 0
	conflictCount := 0
	disconnectedCount := 0

	for _, spec := range proj.Specs {
		if spec.IsRunning() {
			isActive = true
			runningCount++
			if spec.State == project.RunningPush {
				pushCount++
			}
		}
		if spec.IsPaused() {
			pausedCount++
		}
		if spec.RunningSession != nil {
			conflictCount += spec.RunningSession.ConflictCount()
			// Count specs with disconnected endpoints
			if !spec.RunningSession.Alpha.Connected || !spec.RunningSession.Beta.Connected {
				disconnectedCount++
			}
		}
	}

	statusIcon := "○"
	statusColor := theme.StatusPausedFG
	if isActive {
		statusIcon = "✓"
		statusColor = theme.StatusRunningFG
	}

	// Build status text
	var statusText string
	allPaused := runningCount > 0 && pausedCount == runningCount

	if runningCount == 0 {
		statusText = "Not running"
	} else if allPaused {
		if runningCount == len(proj.Specs) {
			statusText = "Paused"
		} else {
			statusText = fmt.Sprintf("%d/%d paused", runningCount, len(proj.Specs))
		}
	} else if runningCount == len(proj.Specs) {
		if pushCount > 0 {
			if pushCount == runningCount {
				statusText = "Running (all one-way)"
			} else {
				statusText = fmt.Sprintf("Running (%d one-way)", pushCount)
			}
		} else {
			statusText = "Running"
		}
	} else {
		if pushCount > 0 {
			statusText = fmt.Sprintf("%d/%d running (%d one-way)", runningCount, len(proj.Specs), pushCount)
		} else {
			statusText = fmt.Sprintf("%d/%d running", runningCount, len(proj.Specs))
		}
	}

	// Append connection issues to status
	if disconnectedCount > 0 {
		statusText += fmt.Sprintf(", %d waiting", disconnectedCount)
	}

	result := fmt.Sprintf("[%s]%s[-] [%s]%s[-] [%s::b]%-30s[-:-:-]  %s",
		colorToTag(theme.SessionNameFG), foldIcon,
		colorToTag(statusColor), statusIcon,
		colorToTag(theme.SessionNameFG), proj.File.DisplayName(),
		statusText)

	if conflictCount > 0 {
		conflictText := "conflict"
		if conflictCount > 1 {
			conflictText = "conflicts"
		}
		result += fmt.Sprintf("  [%s::b]⚠ %d %s[-:-:-]",
			colorToTag(theme.StatusPausedFG), conflictCount, conflictText)
	}

	return result
}

// renderSpecRow renders a spec row line.
func (v *View) renderSpecRow(proj *project.Project, spec *project.SyncSpec, theme ColorScheme) string {
	indent := "    "

	switch spec.State {
	case project.NotRunning:
		sessionDef, exists := proj.File.Sessions[spec.Name]
		if !exists {
			return fmt.Sprintf("%s[%s]○[-] %s Not running",
				indent, colorToTag(theme.StatusPausedFG), spec.Name)
		}
		if v.State.ShowPaths {
			return fmt.Sprintf("%s[%s]○[-] [%s]%-32s[-] [%s]%s[-] ⇄ [%s]%s[-]",
				indent,
				colorToTag(theme.StatusPausedFG),
				colorToTag(theme.SessionNameFG), spec.Name,
				colorToTag(theme.SessionAlphaFG), applyTilde(sessionDef.Alpha),
				colorToTag(theme.SessionBetaFG), applyTilde(sessionDef.Beta))
		}
		return fmt.Sprintf("%s[%s]○[-] [%s]%-32s[-] Not running",
			indent,
			colorToTag(theme.StatusPausedFG),
			colorToTag(theme.SessionNameFG), spec.Name)

	case project.RunningTwoWay, project.RunningPush:
		if spec.RunningSession == nil {
			return fmt.Sprintf("%s[%s]●[-] %s",
				indent, colorToTag(theme.StatusRunningFG), spec.Name)
		}

		session := spec.RunningSession
		statusIcon := "●"
		statusColor := theme.StatusRunningFG
		if session.Paused {
			statusIcon = "⏸"
			statusColor = theme.StatusPausedFG
		}

		nameWithMode := spec.Name
		if spec.State == project.RunningPush {
			nameWithMode = spec.Name + " (one-way)"
		}

		conflictIcon := ""
		if session.HasConflicts() {
			conflictIcon = fmt.Sprintf(" [%s::b]⚠[-:-:-]",
				colorToTag(theme.StatusPausedFG))
		}

		var result string
		if v.State.ShowPaths {
			// Show endpoint paths
			alphaStatus := session.Alpha.StatusIcon()
			alphaColor := theme.StatusRunningFG
			if !session.Alpha.Connected {
				alphaColor = theme.StatusPausedFG
			}

			betaStatus := session.Beta.StatusIcon()
			betaColor := theme.StatusRunningFG
			if !session.Beta.Connected {
				betaColor = theme.StatusPausedFG
			}

			arrow := "⇄"
			if spec.State == project.RunningPush {
				arrow = "⬆"
			}

			result = fmt.Sprintf("%s[%s]%s[-]%s [%s::b]%-32s[-:-:-] %s  [%s]%s[-][%s]%s[-] %s [%s]%s[-][%s]%s[-]",
				indent,
				colorToTag(statusColor), statusIcon,
				conflictIcon,
				colorToTag(theme.SessionNameFG), nameWithMode,
				session.StatusIcon(),
				colorToTag(alphaColor), alphaStatus,
				colorToTag(theme.SessionAlphaFG), session.AlphaDisplay(),
				arrow,
				colorToTag(betaColor), betaStatus,
				colorToTag(theme.SessionBetaFG), session.BetaDisplay())
		} else {
			// Show status and last sync info
			statusText := session.StatusText()
			cyclesInfo := ""
			if session.SuccessfulCycles != nil && *session.SuccessfulCycles > 0 {
				cyclesInfo = fmt.Sprintf(" (%d cycles)", *session.SuccessfulCycles)
			}
			result = fmt.Sprintf("%s[%s]%s[-]%s [%s::b]%-32s[-:-:-] %s %s%s",
				indent,
				colorToTag(statusColor), statusIcon,
				conflictIcon,
				colorToTag(theme.SessionNameFG), nameWithMode,
				session.StatusIcon(),
				statusText,
				cyclesInfo)
		}

		if session.HasConflicts() {
			conflictText := "conflict"
			if session.ConflictCount() > 1 {
				conflictText = "conflicts"
			}
			result += fmt.Sprintf(" [%s::b]⚠ %d %s[-:-:-]",
				colorToTag(theme.StatusPausedFG), session.ConflictCount(), conflictText)
		}

		return result
	}

	return indent + spec.Name
}

// UpdateStatus updates the status bar.
func (v *View) UpdateStatus() {
	theme := v.State.ColorScheme

	var text string
	var color tcell.Color = theme.StatusMessageFG

	// Check if we have a spec selected with a running session
	projIdx, specIdx := v.State.Selection.SelectedSpec()
	hasSelectedSpec := projIdx >= 0 && specIdx >= 0

	// Determine if status message is "important" (should override session status)
	hasImportantStatus := v.State.StatusMessage != nil &&
		(v.State.StatusMessage.Type == StatusError ||
			v.State.StatusMessage.Type == StatusWarning ||
			(v.State.StatusMessage.Type == StatusInfo && v.State.StatusMessage.Text != "Sessions refreshed"))

	if hasImportantStatus {
		// Show important status messages
		text = v.State.StatusMessage.Text
		switch v.State.StatusMessage.Type {
		case StatusError:
			color = theme.StatusErrorFG
		case StatusWarning:
			color = theme.StatusPausedFG
		}
	} else if hasSelectedSpec {
		// Show detailed session status when spec is selected
		proj := v.State.Projects[projIdx]
		spec := &proj.Specs[specIdx]
		if spec.RunningSession != nil {
			text = v.buildSessionStatus(spec.RunningSession)
		} else {
			text = fmt.Sprintf("%s: Not running", spec.Name)
		}
	} else if v.State.StatusMessage != nil {
		// Show unimportant status messages when nothing important to show
		text = v.State.StatusMessage.Text
	} else {
		text = "Ready"
	}

	if v.State.LastRefresh != nil {
		text += fmt.Sprintf(" | Last refresh: %s", v.State.LastRefresh.Format("15:04:05"))
	}

	v.Status.SetText(fmt.Sprintf("[%s]%s[-]", colorToTag(color), text))
}

// buildSessionStatus builds detailed status text for a running session,
// including direction, percentage, file progress, and conflicts.
func (v *View) buildSessionStatus(session *mutagen.SyncSession) string {
	var parts []string

	// Start with session name and status
	parts = append(parts, session.Name, ": ", session.StatusText())

	// Determine direction and staging progress
	var staging *mutagen.StagingProgress
	var direction string

	if session.Alpha.StagingProgress != nil {
		staging = session.Alpha.StagingProgress
		direction = "↓" // Downloading to local (alpha)
	} else if session.Beta.StagingProgress != nil {
		staging = session.Beta.StagingProgress
		direction = "↑" // Uploading to remote (beta)
	}

	// Add direction indicator
	if direction != "" {
		parts = append(parts, " ", direction)
	}

	// Add staging progress details if available
	if staging != nil {
		// Calculate percentage - prefer byte-based for granular progress on large files
		var pct *uint64
		if staging.ReceivedSize != nil && staging.ExpectedSize != nil && *staging.ExpectedSize > 0 {
			percentage := (*staging.ReceivedSize * 100) / *staging.ExpectedSize
			if percentage > 100 {
				percentage = 100
			}
			pct = &percentage
		} else if staging.ReceivedFiles != nil && staging.ExpectedFiles != nil && *staging.ExpectedFiles > 0 {
			percentage := (*staging.ReceivedFiles * 100) / *staging.ExpectedFiles
			if percentage > 100 {
				percentage = 100
			}
			pct = &percentage
		}

		if pct != nil {
			parts = append(parts, fmt.Sprintf(" (%d%%)", *pct))
		}

		// Show current file being copied
		if staging.Path != nil && *staging.Path != "" {
			// Extract just the filename for brevity
			filename := *staging.Path
			if lastSlash := strings.LastIndex(filename, "/"); lastSlash >= 0 {
				filename = filename[lastSlash+1:]
			}
			parts = append(parts, " | ", filename)
		}

		// Show file size if available
		if staging.ReceivedSize != nil && staging.ExpectedSize != nil && *staging.ExpectedSize > 0 {
			parts = append(parts, fmt.Sprintf(" [%s/%s]",
				formatBytes(*staging.ReceivedSize),
				formatBytes(*staging.ExpectedSize)))
		}

		// Show file count progress
		if staging.ReceivedFiles != nil && staging.ExpectedFiles != nil && *staging.ExpectedFiles > 0 {
			parts = append(parts, fmt.Sprintf(" | %d/%d files",
				*staging.ReceivedFiles, *staging.ExpectedFiles))
		}
	}

	// Add conflict count if any
	if session.HasConflicts() {
		conflictCount := session.ConflictCount()
		conflictText := "conflict"
		if conflictCount > 1 {
			conflictText = "conflicts"
		}
		parts = append(parts, fmt.Sprintf(" | %d %s", conflictCount, conflictText))
	}

	return strings.Join(parts, "")
}

// ShowHelpModal displays the help screen.
func (v *View) ShowHelpModal() {
	theme := v.State.ColorScheme
	helpText := fmt.Sprintf(`[%s::b]NAVIGATION[-:-:-]
  ↑/k, ↓/j        Move selection up/down
  h/←, l/→/↵      Fold/unfold project

[%s::b]GLOBAL ACTIONS[-:-:-]
  r               Refresh session list
  m               Toggle display mode
  q, Ctrl-C       Quit application
  ?               Toggle this help screen

[%s::b]PROJECT ACTIONS[-:-:-]
  e               Edit project configuration
  s               Start all specs
  t               Terminate all specs
  f               Flush all specs
  P               Create push sessions
  p/Space         Pause/resume all specs

[%s::b]SPEC ACTIONS[-:-:-]
  s               Start this spec
  t               Terminate this spec
  f               Flush this spec
  P               Create push session
  p/Space         Pause/resume spec
  c               View conflicts

Press ? or Esc to close`,
		colorToTag(theme.HeaderFG),
		colorToTag(theme.HeaderFG),
		colorToTag(theme.HeaderFG),
		colorToTag(theme.HeaderFG))

	modal := tview.NewTextView().
		SetDynamicColors(true).
		SetText(helpText)
	modal.SetBorder(true).SetTitle(" Mutagen TUI - Keyboard Commands ")

	v.Pages.AddPage("help", centerModal(modal, 60, 30), true, true)
	v.App.SetFocus(modal)
}

// HideHelpModal hides the help screen.
func (v *View) HideHelpModal() {
	v.Pages.RemovePage("help")
	v.App.SetFocus(v.List)
}

// ShowConflictModal displays conflicts for the current selection. For projects,
// it aggregates conflicts across all specs with running sessions.
func (v *View) ShowConflictModal(sessionConflicts []SessionConflicts) {
	theme := v.State.ColorScheme

	totalConflicts := 0
	for _, sc := range sessionConflicts {
		totalConflicts += len(sc.Conflicts)
	}

	var text string
	if totalConflicts == 0 {
		text = "No conflicts found"
	} else {
		var sb strings.Builder
		sb.WriteString(fmt.Sprintf("[%s::b]Press 'b' to push all conflicts to beta (alpha → beta copy)[-:-:-]\n",
			colorToTag(theme.HelpKeyFG)))
		sb.WriteString("Press Esc or 'c' to close this view\n\n")

		for _, sc := range sessionConflicts {
			if len(sc.Conflicts) == 0 {
				continue
			}
			if sc.SpecName != "" {
				sb.WriteString(fmt.Sprintf("[%s::b]%s[-:-:-]\n",
					colorToTag(theme.SessionNameFG), sc.SpecName))
			}
			for _, conflict := range sc.Conflicts {
				appendConflictDetails(&sb, theme, conflict, sc.Session)
				sb.WriteString("\n")
			}
			sb.WriteString("\n")
		}
		text = strings.TrimSpace(sb.String())
	}

	modal := tview.NewTextView().
		SetDynamicColors(true).
		SetText(text)
	modal.SetBorder(true).SetTitle(" Conflict Details ")

	v.Pages.AddPage("conflicts", centerModal(modal, 70, 20), true, true)
	v.App.SetFocus(modal)
}

// appendConflictDetails writes a conflict's details into the provided builder.
func appendConflictDetails(sb *strings.Builder, theme ColorScheme, conflict mutagen.Conflict, session *mutagen.SyncSession) {
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

		sb.WriteString(fmt.Sprintf("[%s::b]Alpha (α):[-:-:-] [%s]%s[-]\n",
			colorToTag(theme.SessionAlphaFG),
			colorToTag(theme.SessionAlphaFG),
			alphaPath))
		sb.WriteString(fmt.Sprintf("[%s::b]Beta (β):[-:-:-]  [%s]%s[-]\n",
			colorToTag(theme.SessionBetaFG),
			colorToTag(theme.SessionBetaFG),
			betaPath))
	} else {
		sb.WriteString(fmt.Sprintf("[%s::b]Root:[-:-:-] [%s]%s[-]\n",
			colorToTag(theme.SessionNameFG),
			colorToTag(theme.SessionAlphaFG), conflict.Root))
	}

	sb.WriteString(fmt.Sprintf("  α %d / β %d changes\n",
		len(conflict.AlphaChanges), len(conflict.BetaChanges)))

	if len(conflict.AlphaChanges) > 0 {
		sb.WriteString(fmt.Sprintf("  [%s::b]α[-:-:-] %s\n",
			colorToTag(theme.SessionAlphaFG),
			summarizeChanges(conflict.AlphaChanges)))
	}
	if len(conflict.BetaChanges) > 0 {
		sb.WriteString(fmt.Sprintf("  [%s::b]β[-:-:-] %s\n",
			colorToTag(theme.SessionBetaFG),
			summarizeChanges(conflict.BetaChanges)))
	}
}

// HideConflictModal hides the conflict view.
func (v *View) HideConflictModal() {
	v.Pages.RemovePage("conflicts")
	v.App.SetFocus(v.List)
}

// UpdateConflictModalIfOpen updates the conflict modal if it's currently visible.
// If there are no conflicts, it closes the modal automatically.
// Returns true if the modal was closed due to no conflicts.
func (v *View) UpdateConflictModalIfOpen(sessionConflicts []SessionConflicts) bool {
	if !v.State.ViewingConflicts {
		return false
	}

	totalConflicts := 0
	for _, sc := range sessionConflicts {
		totalConflicts += len(sc.Conflicts)
	}

	// If no conflicts remain, close the modal
	if totalConflicts == 0 {
		v.State.ViewingConflicts = false
		v.HideConflictModal()
		return true
	}

	// Update the modal content by replacing it
	v.Pages.RemovePage("conflicts")
	v.ShowConflictModal(sessionConflicts)
	return false
}

// ShowSyncStatusModal displays detailed sync status for the selected session.
func (v *View) ShowSyncStatusModal(session *mutagen.SyncSession) {
	theme := v.State.ColorScheme

	var text string
	if session == nil {
		text = "No session selected or session not running"
	} else {
		text = fmt.Sprintf("[%s::b]Session:[-:-:-] %s\n", colorToTag(theme.HelpKeyFG), session.Name)
		text += fmt.Sprintf("[%s::b]Status:[-:-:-] %s %s\n", colorToTag(theme.HelpKeyFG), session.StatusIcon(), session.Status)
		if session.Mode != nil {
			text += fmt.Sprintf("[%s::b]Mode:[-:-:-] %s\n", colorToTag(theme.HelpKeyFG), *session.Mode)
		}
		text += fmt.Sprintf("[%s::b]Paused:[-:-:-] %v\n\n", colorToTag(theme.HelpKeyFG), session.Paused)

		// Alpha endpoint
		text += fmt.Sprintf("[%s::b]Alpha (α):[-:-:-]\n", colorToTag(theme.SessionAlphaFG))
		text += formatEndpointDetails(&session.Alpha, &theme)

		// Beta endpoint
		text += fmt.Sprintf("[%s::b]Beta (β):[-:-:-]\n", colorToTag(theme.SessionBetaFG))
		text += formatEndpointDetails(&session.Beta, &theme)

		// Conflicts
		if session.HasConflicts() {
			text += fmt.Sprintf("\n[%s::b]Conflicts:[-:-:-] %d\n", colorToTag(theme.StatusErrorFG), session.ConflictCount())
		}

		// Successful cycles
		if session.SuccessfulCycles != nil {
			text += fmt.Sprintf("\n[%s::b]Successful Cycles:[-:-:-] %d\n", colorToTag(theme.HelpKeyFG), *session.SuccessfulCycles)
		}

		text += "\nPress Esc or 'i' to close"
	}

	modal := tview.NewTextView().
		SetDynamicColors(true).
		SetText(text)
	modal.SetBorder(true).SetTitle(" Sync Status ")

	v.Pages.AddPage("syncstatus", centerModal(modal, 70, 22), true, true)
	v.App.SetFocus(modal)
}

// HideSyncStatusModal hides the sync status view.
func (v *View) HideSyncStatusModal() {
	v.Pages.RemovePage("syncstatus")
	v.App.SetFocus(v.List)
}

// formatEndpointDetails formats endpoint information for display.
func formatEndpointDetails(e *mutagen.Endpoint, theme *ColorScheme) string {
	text := fmt.Sprintf("  %s %s\n", e.StatusIcon(), e.DisplayPath())
	text += fmt.Sprintf("  Connected: %v, Scanned: %v\n", e.Connected, e.Scanned)

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
		text += counts + "\n"
	}

	if e.TotalFileSize != nil {
		text += fmt.Sprintf("  Total Size: %s\n", formatBytes(*e.TotalFileSize))
	}

	if e.StagingProgress != nil {
		text += formatStagingProgress(e.StagingProgress, theme)
	}

	text += "\n"
	return text
}

// formatStagingProgress formats staging progress for display.
func formatStagingProgress(p *mutagen.StagingProgress, theme *ColorScheme) string {
	text := fmt.Sprintf("  [%s]Staging:[-]\n", colorToTag(theme.SessionStatusFG))

	if p.Path != nil {
		text += fmt.Sprintf("    Current: %s\n", *p.Path)
	}

	if p.ReceivedFiles != nil && p.ExpectedFiles != nil {
		percent := float64(0)
		if *p.ExpectedFiles > 0 {
			percent = float64(*p.ReceivedFiles) / float64(*p.ExpectedFiles) * 100
		}
		text += fmt.Sprintf("    Files: %d/%d (%.1f%%)\n", *p.ReceivedFiles, *p.ExpectedFiles, percent)
	}

	if p.ReceivedSize != nil && p.ExpectedSize != nil {
		percent := float64(0)
		if *p.ExpectedSize > 0 {
			percent = float64(*p.ReceivedSize) / float64(*p.ExpectedSize) * 100
		}
		text += fmt.Sprintf("    Size: %s/%s (%.1f%%)\n",
			formatBytes(*p.ReceivedSize), formatBytes(*p.ExpectedSize), percent)
	}

	return text
}

// formatBytes formats a byte count as a human-readable string.
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

// ShowBlockingModal shows a blocking operation modal.
func (v *View) ShowBlockingModal(message string) {
	modal := tview.NewModal().
		SetText("⏳ " + message + "\n\nPlease wait...")

	v.Pages.AddPage("blocking", modal, true, true)
}

// HideBlockingModal hides the blocking operation modal.
func (v *View) HideBlockingModal() {
	v.Pages.RemovePage("blocking")
	v.App.SetFocus(v.List)
}

// Helper functions

func colorToTag(c tcell.Color) string {
	r, g, b := c.RGB()
	return fmt.Sprintf("#%02x%02x%02x", r, g, b)
}

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

	// Handle other users' home directories
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

func centerModal(p tview.Primitive, width, height int) tview.Primitive {
	return tview.NewFlex().
		AddItem(nil, 0, 1, false).
		AddItem(tview.NewFlex().SetDirection(tview.FlexRow).
			AddItem(nil, 0, 1, false).
			AddItem(p, height, 1, true).
			AddItem(nil, 0, 1, false), width, 1, true).
		AddItem(nil, 0, 1, false)
}
