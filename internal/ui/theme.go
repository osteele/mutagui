// Package ui provides the terminal user interface for mutagui.
package ui

import (
	"os"
	"strings"

	"github.com/gdamore/tcell/v2"
)

// ColorScheme defines the colors used in the UI.
type ColorScheme struct {
	HeaderFG        tcell.Color
	SessionNameFG   tcell.Color
	SessionAlphaFG  tcell.Color
	SessionBetaFG   tcell.Color
	SessionStatusFG tcell.Color
	StatusRunningFG tcell.Color
	StatusPausedFG  tcell.Color
	SelectionBG     tcell.Color
	StatusMessageFG tcell.Color
	StatusErrorFG   tcell.Color
	HelpKeyFG       tcell.Color
	HelpTextFG      tcell.Color
}

// DarkTheme returns a color scheme for dark terminals.
// Uses standard ANSI colors (0-15) which terminals can remap to their palette.
func DarkTheme() ColorScheme {
	return ColorScheme{
		HeaderFG:        tcell.ColorAqua,    // Bright cyan (ANSI 14)
		SessionNameFG:   tcell.ColorWhite,   // Bright white (ANSI 15)
		SessionAlphaFG:  tcell.ColorBlue,    // Blue (ANSI 4)
		SessionBetaFG:   tcell.ColorFuchsia, // Bright magenta (ANSI 13)
		SessionStatusFG: tcell.ColorSilver,  // Light gray (ANSI 7)
		StatusRunningFG: tcell.ColorLime,    // Bright green (ANSI 10)
		StatusPausedFG:  tcell.ColorYellow,  // Bright yellow (ANSI 11)
		SelectionBG:     tcell.ColorNavy,    // Dark blue (ANSI 4) - visible selection
		StatusMessageFG: tcell.ColorYellow,  // Bright yellow (ANSI 11)
		StatusErrorFG:   tcell.ColorRed,     // Red (ANSI 1)
		HelpKeyFG:       tcell.ColorAqua,    // Bright cyan (ANSI 14)
		HelpTextFG:      tcell.ColorWhite,   // Bright white (ANSI 15)
	}
}

// LightTheme returns a color scheme for light terminals.
func LightTheme() ColorScheme {
	return ColorScheme{
		HeaderFG:        tcell.ColorBlue,
		SessionNameFG:   tcell.ColorBlack,
		SessionAlphaFG:  tcell.ColorDarkGray,
		SessionBetaFG:   tcell.NewRGBColor(128, 0, 128),   // Purple
		SessionStatusFG: tcell.NewRGBColor(64, 64, 64),    // Dark gray
		StatusRunningFG: tcell.NewRGBColor(0, 128, 0),     // Dark green
		StatusPausedFG:  tcell.NewRGBColor(184, 134, 11),  // Dark goldenrod
		SelectionBG:     tcell.NewRGBColor(200, 200, 200), // Light gray
		StatusMessageFG: tcell.NewRGBColor(184, 134, 11),  // Dark goldenrod
		StatusErrorFG:   tcell.ColorRed,
		HelpKeyFG:       tcell.ColorBlue,
		HelpTextFG:      tcell.ColorBlack,
	}
}

// DetectTheme returns the appropriate theme based on MUTAGUI_THEME env var.
// Defaults to light theme since automatic detection is unreliable across terminals.
func DetectTheme() ColorScheme {
	if override := os.Getenv("MUTAGUI_THEME"); override != "" {
		switch strings.ToLower(override) {
		case "dark":
			return DarkTheme()
		}
	}
	return LightTheme()
}
