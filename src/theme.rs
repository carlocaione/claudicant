use ratatui::style::{Color, Style};

#[allow(dead_code)]
pub struct Theme {
    // Borders
    pub border_focused: Color,
    pub border_unfocused: Color,

    // Diff lines
    pub addition_prefix: Color,
    pub deletion_prefix: Color,
    pub gutter: Color,
    pub file_header: Color,

    // Commits list
    pub sha: Color,
    pub commit_not_reviewed: Color,
    pub commit_in_progress: Color,
    pub commit_fully_reviewed: Color,

    // Comment severity colors
    pub severity_critical: Color,
    pub severity_warning: Color,
    pub severity_suggestion: Color,
    pub severity_nitpick: Color,

    // Comment markers in diff
    pub comment_accepted_marker: Color,
    pub comment_rejected_marker: Color,

    // File headers in diff
    pub file_header_bg: Color,

    // Status bar
    pub status_fg: Color,
    pub status_dim: Color,
    pub status_accent: Color,
    pub status_bar_bg: Color,

    // Cursor / selection
    pub cursor_bg: Color,

    // Syntect theme name (empty = no syntax highlighting)
    pub syntect_theme: String,
}

impl Theme {
    pub fn border_style(&self, focused: bool) -> Style {
        if focused {
            Style::default().fg(self.border_focused)
        } else {
            Style::default().fg(self.border_unfocused)
        }
    }

    pub fn cursor_style(&self, focused: bool) -> Style {
        if focused {
            Style::default().bg(self.cursor_bg)
        } else {
            Style::default()
        }
    }

    pub fn is_terminal(&self) -> bool {
        self.syntect_theme.is_empty()
    }

    pub fn name(&self) -> &'static str {
        if self.syntect_theme.is_empty() {
            "Terminal"
        } else {
            "Solarized Dark"
        }
    }

    /// Plain ANSI colors — no RGB, no syntax highlighting. Native in any terminal.
    pub fn terminal() -> Self {
        Self {
            border_focused: Color::Reset,
            border_unfocused: Color::DarkGray,

            addition_prefix: Color::Green,
            deletion_prefix: Color::Red,
            gutter: Color::DarkGray,
            file_header: Color::Yellow,

            sha: Color::Yellow,
            commit_not_reviewed: Color::DarkGray,
            commit_in_progress: Color::Yellow,
            commit_fully_reviewed: Color::Green,

            severity_critical: Color::Red,
            severity_warning: Color::Yellow,
            severity_suggestion: Color::Cyan,
            severity_nitpick: Color::DarkGray,

            comment_accepted_marker: Color::Green,
            comment_rejected_marker: Color::DarkGray,

            file_header_bg: Color::Reset,

            status_fg: Color::Reset,
            status_dim: Color::DarkGray,
            status_accent: Color::Cyan,
            status_bar_bg: Color::Reset,

            cursor_bg: Color::DarkGray,

            syntect_theme: String::new(),
        }
    }

    /// Solarized Dark — curated RGB palette with matched syntax highlighting.
    pub fn solarized_dark() -> Self {
        // Solarized palette (Ethan Schoonover)
        let base01  = Color::Rgb(88, 110, 117);   // muted
        let base0   = Color::Rgb(131, 148, 150);  // fg
        let base02  = Color::Rgb(7, 54, 66);      // selection/highlight bg
        let yellow  = Color::Rgb(181, 137, 0);
        let cyan    = Color::Rgb(42, 161, 152);
        let blue    = Color::Rgb(38, 139, 210);
        let green   = Color::Rgb(133, 153, 0);
        let red     = Color::Rgb(220, 50, 47);
        let violet  = Color::Rgb(108, 113, 196);

        Self {
            border_focused: base0,
            border_unfocused: base01,

            addition_prefix: green,
            deletion_prefix: red,
            gutter: base01,
            file_header: violet,

            sha: blue,
            commit_not_reviewed: base01,
            commit_in_progress: yellow,
            commit_fully_reviewed: green,

            severity_critical: red,
            severity_warning: yellow,
            severity_suggestion: cyan,
            severity_nitpick: base01,

            comment_accepted_marker: green,
            comment_rejected_marker: base01,

            file_header_bg: base02,

            status_fg: base0,
            status_dim: base01,
            status_accent: blue,
            status_bar_bg: base02,

            cursor_bg: base02,

            syntect_theme: "Solarized (dark)".to_string(),
        }
    }

    pub fn by_name(name: &str) -> Option<Self> {
        match name {
            "terminal" => Some(Self::terminal()),
            "solarized-dark" => Some(Self::solarized_dark()),
            _ => None,
        }
    }

    pub fn available_themes() -> &'static [&'static str] {
        &["terminal", "solarized-dark"]
    }
}
