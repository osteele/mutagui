// Package ui provides the terminal user interface for mutagui.
package ui

import (
	"os"
	"strings"

	"github.com/charmbracelet/lipgloss"
)

// Theme defines the styles used in the UI.
type Theme struct {
	// Base styles
	App lipgloss.Style

	// Header
	Header      lipgloss.Style
	HeaderTitle lipgloss.Style

	// List styles
	ListBorder       lipgloss.Style
	ListTitle        lipgloss.Style
	SelectedItem     lipgloss.Style
	UnselectedItem   lipgloss.Style
	ProjectHeader    lipgloss.Style
	SpecRow          lipgloss.Style
	SessionName      lipgloss.Style
	SessionAlpha     lipgloss.Style
	SessionBeta      lipgloss.Style
	SessionStatus    lipgloss.Style
	StatusRunning    lipgloss.Style
	StatusPaused     lipgloss.Style
	StatusNotRunning lipgloss.Style

	// Status bar
	StatusBar     lipgloss.Style
	StatusMessage lipgloss.Style
	StatusWarning lipgloss.Style
	StatusError   lipgloss.Style

	// Help bar
	HelpBar  lipgloss.Style
	HelpKey  lipgloss.Style
	HelpText lipgloss.Style
	HelpSep  lipgloss.Style

	// Modal styles
	ModalBorder  lipgloss.Style
	ModalTitle   lipgloss.Style
	ModalContent lipgloss.Style
	ModalHelp    lipgloss.Style

	// Conflict modal
	ConflictAlpha lipgloss.Style
	ConflictBeta  lipgloss.Style
}

// DarkTheme returns a theme for dark terminals.
func DarkTheme() Theme {
	return Theme{
		App: lipgloss.NewStyle(),

		Header:      lipgloss.NewStyle().Border(lipgloss.RoundedBorder()).BorderForeground(lipgloss.Color("240")).Padding(0, 1),
		HeaderTitle: lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("14")), // Cyan

		ListBorder:       lipgloss.NewStyle().Border(lipgloss.RoundedBorder()).BorderForeground(lipgloss.Color("240")),
		ListTitle:        lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("15")),
		SelectedItem:     lipgloss.NewStyle().Background(lipgloss.Color("17")).Foreground(lipgloss.Color("15")), // Dark blue bg
		UnselectedItem:   lipgloss.NewStyle(),
		ProjectHeader:    lipgloss.NewStyle().Bold(true),
		SpecRow:          lipgloss.NewStyle(),
		SessionName:      lipgloss.NewStyle().Foreground(lipgloss.Color("15")),  // White
		SessionAlpha:     lipgloss.NewStyle().Foreground(lipgloss.Color("12")),  // Blue
		SessionBeta:      lipgloss.NewStyle().Foreground(lipgloss.Color("13")),  // Magenta
		SessionStatus:    lipgloss.NewStyle().Foreground(lipgloss.Color("7")),   // Silver
		StatusRunning:    lipgloss.NewStyle().Foreground(lipgloss.Color("10")),  // Lime
		StatusPaused:     lipgloss.NewStyle().Foreground(lipgloss.Color("11")),  // Yellow
		StatusNotRunning: lipgloss.NewStyle().Foreground(lipgloss.Color("240")), // Gray

		StatusBar:     lipgloss.NewStyle().Border(lipgloss.RoundedBorder()).BorderForeground(lipgloss.Color("240")).Padding(0, 1),
		StatusMessage: lipgloss.NewStyle().Foreground(lipgloss.Color("11")), // Yellow
		StatusWarning: lipgloss.NewStyle().Foreground(lipgloss.Color("11")), // Yellow
		StatusError:   lipgloss.NewStyle().Foreground(lipgloss.Color("9")),  // Red

		HelpBar:  lipgloss.NewStyle().Border(lipgloss.RoundedBorder()).BorderForeground(lipgloss.Color("240")).Padding(0, 1),
		HelpKey:  lipgloss.NewStyle().Foreground(lipgloss.Color("14")), // Cyan
		HelpText: lipgloss.NewStyle().Foreground(lipgloss.Color("15")), // White
		HelpSep:  lipgloss.NewStyle().Foreground(lipgloss.Color("240")),

		ModalBorder:  lipgloss.NewStyle().Border(lipgloss.RoundedBorder()).BorderForeground(lipgloss.Color("14")).Padding(1, 2),
		ModalTitle:   lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("14")),
		ModalContent: lipgloss.NewStyle(),
		ModalHelp:    lipgloss.NewStyle().Foreground(lipgloss.Color("240")).Italic(true),

		ConflictAlpha: lipgloss.NewStyle().Foreground(lipgloss.Color("12")),
		ConflictBeta:  lipgloss.NewStyle().Foreground(lipgloss.Color("13")),
	}
}

// LightTheme returns a theme for light terminals.
func LightTheme() Theme {
	return Theme{
		App: lipgloss.NewStyle(),

		Header:      lipgloss.NewStyle().Border(lipgloss.RoundedBorder()).BorderForeground(lipgloss.Color("240")).Padding(0, 1),
		HeaderTitle: lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("4")), // Blue

		ListBorder:       lipgloss.NewStyle().Border(lipgloss.RoundedBorder()).BorderForeground(lipgloss.Color("240")),
		ListTitle:        lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("0")),
		SelectedItem:     lipgloss.NewStyle().Background(lipgloss.Color("252")).Foreground(lipgloss.Color("0")), // Light gray bg
		UnselectedItem:   lipgloss.NewStyle(),
		ProjectHeader:    lipgloss.NewStyle().Bold(true),
		SpecRow:          lipgloss.NewStyle(),
		SessionName:      lipgloss.NewStyle().Foreground(lipgloss.Color("0")),   // Black
		SessionAlpha:     lipgloss.NewStyle().Foreground(lipgloss.Color("240")), // Dark gray
		SessionBeta:      lipgloss.NewStyle().Foreground(lipgloss.Color("5")),   // Purple
		SessionStatus:    lipgloss.NewStyle().Foreground(lipgloss.Color("240")), // Dark gray
		StatusRunning:    lipgloss.NewStyle().Foreground(lipgloss.Color("2")),   // Dark green
		StatusPaused:     lipgloss.NewStyle().Foreground(lipgloss.Color("3")),   // Dark yellow/brown
		StatusNotRunning: lipgloss.NewStyle().Foreground(lipgloss.Color("245")), // Gray

		StatusBar:     lipgloss.NewStyle().Border(lipgloss.RoundedBorder()).BorderForeground(lipgloss.Color("240")).Padding(0, 1),
		StatusMessage: lipgloss.NewStyle().Foreground(lipgloss.Color("3")), // Dark yellow
		StatusWarning: lipgloss.NewStyle().Foreground(lipgloss.Color("3")), // Dark yellow
		StatusError:   lipgloss.NewStyle().Foreground(lipgloss.Color("1")), // Red

		HelpBar:  lipgloss.NewStyle().Border(lipgloss.RoundedBorder()).BorderForeground(lipgloss.Color("240")).Padding(0, 1),
		HelpKey:  lipgloss.NewStyle().Foreground(lipgloss.Color("4")), // Blue
		HelpText: lipgloss.NewStyle().Foreground(lipgloss.Color("0")), // Black
		HelpSep:  lipgloss.NewStyle().Foreground(lipgloss.Color("245")),

		ModalBorder:  lipgloss.NewStyle().Border(lipgloss.RoundedBorder()).BorderForeground(lipgloss.Color("4")).Padding(1, 2),
		ModalTitle:   lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("4")),
		ModalContent: lipgloss.NewStyle(),
		ModalHelp:    lipgloss.NewStyle().Foreground(lipgloss.Color("245")).Italic(true),

		ConflictAlpha: lipgloss.NewStyle().Foreground(lipgloss.Color("240")),
		ConflictBeta:  lipgloss.NewStyle().Foreground(lipgloss.Color("5")),
	}
}

// DetectTheme returns the appropriate theme based on MUTAGUI_THEME env var.
func DetectTheme() Theme {
	if override := os.Getenv("MUTAGUI_THEME"); override != "" {
		switch strings.ToLower(override) {
		case "dark":
			return DarkTheme()
		}
	}
	return LightTheme()
}

// GetTheme returns the theme based on the theme name.
func GetTheme(name string) Theme {
	switch strings.ToLower(name) {
	case "dark":
		return DarkTheme()
	case "light":
		return LightTheme()
	default:
		return DetectTheme()
	}
}
