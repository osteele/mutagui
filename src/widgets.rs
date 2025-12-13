//! Reusable UI widget abstractions.
//!
//! This module provides builder patterns for common UI elements,
//! reducing code duplication in the main UI rendering code.

use crate::theme::ColorScheme;
use ratatui::style::{Modifier, Style};
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

#[allow(dead_code)]
impl<'a> StyledText<'a> {
    /// Create a new StyledText builder with the given color scheme.
    pub fn new(theme: &'a ColorScheme) -> Self {
        Self {
            theme,
            spans: Vec::new(),
        }
    }

    /// Add a status icon (running/paused indicator).
    #[cfg(test)]
    pub fn status_icon(mut self, icon: &'a str, is_running: bool) -> Self {
        let color = if is_running {
            self.theme.status_running_fg
        } else {
            self.theme.status_paused_fg
        };
        self.spans
            .push(Span::styled(icon, Style::default().fg(color)));
        self
    }

    /// Add a status icon from an owned string.
    pub fn status_icon_owned(mut self, icon: String, is_running: bool) -> Self {
        let color = if is_running {
            self.theme.status_running_fg
        } else {
            self.theme.status_paused_fg
        };
        self.spans
            .push(Span::styled(icon, Style::default().fg(color)));
        self
    }

    /// Add a session name (bold, primary text color).
    #[cfg(test)]
    pub fn session_name(mut self, name: &'a str) -> Self {
        self.spans.push(Span::styled(
            name,
            Style::default()
                .fg(self.theme.session_name_fg)
                .add_modifier(Modifier::BOLD),
        ));
        self
    }

    /// Add a session name from an owned string.
    pub fn session_name_owned(mut self, name: String) -> Self {
        self.spans.push(Span::styled(
            name,
            Style::default()
                .fg(self.theme.session_name_fg)
                .add_modifier(Modifier::BOLD),
        ));
        self
    }

    /// Add an alpha endpoint path/status.
    pub fn endpoint_alpha(mut self, text: &'a str) -> Self {
        self.spans.push(Span::styled(
            text,
            Style::default().fg(self.theme.session_alpha_fg),
        ));
        self
    }

    /// Add session status text (muted color).
    pub fn status_text(mut self, text: &'a str) -> Self {
        self.spans.push(Span::styled(
            text,
            Style::default().fg(self.theme.session_status_fg),
        ));
        self
    }

    /// Add session status text from an owned string.
    pub fn status_text_owned(mut self, text: String) -> Self {
        self.spans.push(Span::styled(
            text,
            Style::default().fg(self.theme.session_status_fg),
        ));
        self
    }

    /// Add plain/unstyled text.
    pub fn text(mut self, text: &'a str) -> Self {
        self.spans.push(Span::raw(text));
        self
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

    /// Build and return the spans vector (for when you need Vec<Span>).
    #[cfg(test)]
    pub fn into_spans(self) -> Vec<Span<'a>> {
        self.spans
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
    fn test_styled_text_builder() {
        let theme = ColorScheme::dark();
        let line = StyledText::new(&theme)
            .status_icon("▶", true)
            .text(" ")
            .session_name("test-session")
            .build();

        assert_eq!(line.spans.len(), 3);
    }

    #[test]
    fn test_styled_text_empty() {
        let theme = ColorScheme::dark();
        let line = StyledText::new(&theme).build();

        assert_eq!(line.spans.len(), 0);
    }

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

    #[test]
    fn test_into_spans() {
        let theme = ColorScheme::dark();
        let spans = StyledText::new(&theme)
            .text("hello")
            .text(" ")
            .text("world")
            .into_spans();

        assert_eq!(spans.len(), 3);
    }
}
