//! Reusable UI widget abstractions.
//!
//! This module provides builder patterns for common UI elements,
//! reducing code duplication in the main UI rendering code.

use crate::theme::ColorScheme;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

/// Builder for creating styled text lines with semantic color roles.
///
/// This builder provides a fluent API for constructing styled text,
/// using semantic method names that map to the color scheme.
///
/// # Example
///
/// ```ignore
/// let line = StyledText::new(&color_scheme)
///     .status_icon_owned("▶ ".to_string(), true)
///     .session_name_owned("my-session".to_string())
///     .text(" ")
///     .endpoint_alpha("/local/path")
///     .build();
/// ```
pub struct StyledText<'a> {
    theme: &'a ColorScheme,
    spans: Vec<Span<'a>>,
}

impl<'a> StyledText<'a> {
    /// Create a new StyledText builder with the given color scheme.
    pub fn new(theme: &'a ColorScheme) -> Self {
        Self {
            theme,
            spans: Vec::new(),
        }
    }

    /// Add text with custom style.
    pub fn styled(mut self, text: &'a str, style: Style) -> Self {
        self.spans.push(Span::styled(text, style));
        self
    }

    /// Add help text (muted).
    pub fn help_text(mut self, text: &'a str) -> Self {
        self.spans.push(Span::styled(
            text,
            Style::default().fg(self.theme.help_text_fg),
        ));
        self
    }

    /// Add header text (accent color).
    pub fn header(mut self, text: &'a str) -> Self {
        self.spans.push(Span::styled(
            text,
            Style::default().fg(self.theme.header_fg),
        ));
        self
    }

    /// Build the final Line.
    pub fn build(self) -> Line<'a> {
        Line::from(self.spans)
    }
}

/// Builder for creating help bar content.
pub struct HelpBar<'a> {
    theme: &'a ColorScheme,
    items: Vec<(&'a str, &'a str)>, // (key, description)
}

impl<'a> HelpBar<'a> {
    pub fn new(theme: &'a ColorScheme) -> Self {
        Self {
            theme,
            items: Vec::new(),
        }
    }

    /// Add a help item (key and description).
    pub fn item(mut self, key: &'a str, description: &'a str) -> Self {
        self.items.push((key, description));
        self
    }

    /// Build the final Line with all help items.
    pub fn build(self) -> Line<'a> {
        let mut spans = Vec::new();

        for (i, (key, desc)) in self.items.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(
                    " | ",
                    Style::default().fg(self.theme.help_text_fg),
                ));
            }
            spans.push(Span::styled(
                *key,
                Style::default().fg(self.theme.help_key_fg),
            ));
            spans.push(Span::styled(
                format!(" {}", desc),
                Style::default().fg(self.theme.help_text_fg),
            ));
        }

        Line::from(spans)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ColorScheme;

    #[test]
    fn test_help_bar_builder() {
        let theme = ColorScheme::dark();
        let line = HelpBar::new(&theme)
            .item("q", "Quit")
            .item("r", "Refresh")
            .build();

        // 2 items with separator = 5 spans: key1, desc1, separator, key2, desc2
        assert_eq!(line.spans.len(), 5);
    }

    #[test]
    fn test_help_bar_single_item() {
        let theme = ColorScheme::dark();
        let line = HelpBar::new(&theme).item("q", "Quit").build();

        // Single item: key, description
        assert_eq!(line.spans.len(), 2);
    }
}
