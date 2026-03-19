use ratatui::style::{Color, Modifier, Style};

#[allow(dead_code)]
pub struct Theme {
    // Borders
    pub border_focused: Color,
    pub border_unfocused: Color,

    // Diff lines
    pub addition_bg: Color,
    pub deletion_bg: Color,
    pub addition_prefix: Color,
    pub deletion_prefix: Color,
    pub gutter: Color,
    pub hunk_header: Color,
    pub file_header: Color,

    // Commits list
    pub sha: Color,
    pub commit_highlight_fg: Color,
    pub commit_highlight_bg: Color,
    pub commit_not_reviewed: Color,
    pub commit_in_progress: Color,
    pub commit_fully_reviewed: Color,

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

    // Syntect theme name (must match a key in ThemeSet::load_defaults())
    pub syntect_theme: &'static str,
}

impl Theme {
    pub fn border_style(&self, focused: bool) -> Style {
        if focused {
            Style::default().fg(self.border_focused)
        } else {
            Style::default().fg(self.border_unfocused)
        }
    }

    pub fn commit_highlight_style(&self) -> Style {
        Style::default()
            .fg(self.commit_highlight_fg)
            .bg(self.commit_highlight_bg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn cursor_style(&self, focused: bool) -> Style {
        if focused {
            Style::default().bg(self.cursor_bg)
        } else {
            Style::default()
        }
    }

    pub fn ocean_dark() -> Self {
        Self {
            border_focused: Color::White,
            border_unfocused: Color::DarkGray,

            addition_bg: Color::Rgb(0, 40, 0),
            deletion_bg: Color::Rgb(40, 0, 0),
            addition_prefix: Color::Green,
            deletion_prefix: Color::Red,
            gutter: Color::DarkGray,
            hunk_header: Color::Cyan,
            file_header: Color::Yellow,

            sha: Color::Yellow,
            commit_highlight_fg: Color::Black,
            commit_highlight_bg: Color::White,
            commit_not_reviewed: Color::DarkGray,
            commit_in_progress: Color::Yellow,
            commit_fully_reviewed: Color::Green,

            comment_accepted_marker: Color::Green,
            comment_rejected_marker: Color::DarkGray,

            file_header_bg: Color::Rgb(30, 40, 55),

            status_fg: Color::White,
            status_dim: Color::DarkGray,
            status_accent: Color::Cyan,
            status_bar_bg: Color::Rgb(30, 40, 55),

            cursor_bg: Color::DarkGray,

            syntect_theme: "base16-ocean.dark",
        }
    }

    pub fn solarized_dark() -> Self {
        // Solarized palette
        let base03 = Color::Rgb(0, 43, 54);
        let base01 = Color::Rgb(88, 110, 117);
        let base0 = Color::Rgb(131, 148, 150);
        let base1 = Color::Rgb(147, 161, 161);
        let yellow = Color::Rgb(181, 137, 0);
        let cyan = Color::Rgb(42, 161, 152);
        let green = Color::Rgb(133, 153, 0);
        let red = Color::Rgb(220, 50, 47);

        Self {
            border_focused: base0,
            border_unfocused: base01,

            addition_bg: Color::Rgb(0, 35, 0),
            deletion_bg: Color::Rgb(45, 0, 0),
            addition_prefix: green,
            deletion_prefix: red,
            gutter: base01,
            hunk_header: cyan,
            file_header: yellow,

            sha: yellow,
            commit_highlight_fg: base03,
            commit_highlight_bg: base1,
            commit_not_reviewed: base01,
            commit_in_progress: yellow,
            commit_fully_reviewed: green,

            comment_accepted_marker: green,
            comment_rejected_marker: base01,

            file_header_bg: Color::Rgb(7, 54, 66),

            status_fg: base0,
            status_dim: base01,
            status_accent: cyan,
            status_bar_bg: Color::Rgb(7, 54, 66),

            cursor_bg: Color::Rgb(7, 54, 66), // base02

            syntect_theme: "Solarized (dark)",
        }
    }

    pub fn light() -> Self {
        Self {
            border_focused: Color::Black,
            border_unfocused: Color::Gray,

            addition_bg: Color::Rgb(220, 255, 220),
            deletion_bg: Color::Rgb(255, 220, 220),
            addition_prefix: Color::Rgb(0, 120, 0),
            deletion_prefix: Color::Rgb(180, 0, 0),
            gutter: Color::Gray,
            hunk_header: Color::Rgb(0, 120, 150),
            file_header: Color::Rgb(140, 100, 0),

            sha: Color::Rgb(140, 100, 0),
            commit_highlight_fg: Color::White,
            commit_highlight_bg: Color::Rgb(0, 100, 180),
            commit_not_reviewed: Color::Gray,
            commit_in_progress: Color::Rgb(180, 130, 0),
            commit_fully_reviewed: Color::Rgb(0, 140, 0),

            comment_accepted_marker: Color::Rgb(0, 140, 0),
            comment_rejected_marker: Color::Gray,

            file_header_bg: Color::Rgb(240, 235, 210),

            status_fg: Color::Black,
            status_dim: Color::Gray,
            status_accent: Color::Rgb(0, 120, 150),
            status_bar_bg: Color::Rgb(230, 230, 240),

            cursor_bg: Color::Rgb(230, 230, 230),

            syntect_theme: "InspiredGitHub",
        }
    }

    pub fn by_name(name: &str) -> Option<Self> {
        match name {
            "ocean-dark" => Some(Self::ocean_dark()),
            "solarized-dark" => Some(Self::solarized_dark()),
            "light" => Some(Self::light()),
            _ => None,
        }
    }

    pub fn available_themes() -> &'static [&'static str] {
        &["ocean-dark", "solarized-dark", "light"]
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::ocean_dark()
    }
}
