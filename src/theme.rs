use ratatui::style::Color;

#[derive(Debug, Clone)]
pub struct ColorScheme {
    pub header_fg: Color,
    pub session_name_fg: Color,
    pub session_alpha_fg: Color,
    pub session_beta_fg: Color,
    pub session_status_fg: Color,
    pub status_running_fg: Color,
    pub status_paused_fg: Color,
    pub selection_bg: Color,
    pub status_message_fg: Color,
    pub help_key_fg: Color,
    pub help_text_fg: Color,
}

impl ColorScheme {
    pub fn dark() -> Self {
        Self {
            header_fg: Color::Cyan,
            session_name_fg: Color::White,
            session_alpha_fg: Color::Blue,
            session_beta_fg: Color::Magenta,
            session_status_fg: Color::Gray,
            status_running_fg: Color::Green,
            status_paused_fg: Color::Yellow,
            selection_bg: Color::DarkGray,
            status_message_fg: Color::Yellow,
            help_key_fg: Color::Cyan,
            help_text_fg: Color::White,
        }
    }

    pub fn light() -> Self {
        Self {
            header_fg: Color::Blue,
            session_name_fg: Color::Black,
            session_alpha_fg: Color::DarkGray,
            session_beta_fg: Color::Rgb(128, 0, 128), // Purple
            session_status_fg: Color::Rgb(64, 64, 64), // Dark gray
            status_running_fg: Color::Rgb(0, 128, 0), // Dark green
            status_paused_fg: Color::Rgb(184, 134, 11), // Dark goldenrod
            selection_bg: Color::Rgb(200, 200, 200),  // Light gray
            status_message_fg: Color::Rgb(184, 134, 11), // Dark goldenrod
            help_key_fg: Color::Blue,
            help_text_fg: Color::Black,
        }
    }
}

pub fn detect_theme() -> ColorScheme {
    match terminal_light::luma() {
        Ok(luma) if luma > 0.6 => ColorScheme::light(),
        Ok(_) => ColorScheme::dark(),
        Err(_) => ColorScheme::dark(),
    }
}
