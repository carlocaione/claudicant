use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

/// Result of an ActionDialog interaction.
#[derive(Debug, Clone, PartialEq)]
pub enum DialogResult {
    /// Still open, no decision yet
    Pending,
    /// User accepted the text as-is
    Accept(String),
    /// User wants to edit the text in $EDITOR
    Edit(String),
    /// User cancelled/closed the dialog
    Cancel,
}

/// A reusable dialog with a read-only text view.
/// Actions shown in the bottom border: (a)ccept (e)dit Esc:cancel
pub struct ActionDialog {
    text: String,
    scroll: u16,
    title: String,
    border_color: Color,
    markdown: bool,
    extra_hints: Vec<(String, String)>,
}

impl ActionDialog {
    pub fn new(text: &str, title: &str, border_color: Color) -> Self {
        Self {
            text: text.to_string(),
            scroll: 0,
            title: title.to_string(),
            border_color,
            markdown: true,
            extra_hints: Vec::new(),
        }
    }

    pub fn with_hint(mut self, key: &str, desc: &str) -> Self {
        self.extra_hints.push((key.to_string(), desc.to_string()));
        self
    }

    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.scroll = 0;
    }

    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    pub fn set_border_color(&mut self, color: Color) {
        self.border_color = color;
    }

    pub fn on_key(&mut self, key: KeyEvent) -> DialogResult {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.scroll = self.scroll.saturating_add(1);
                DialogResult::Pending
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.scroll = self.scroll.saturating_sub(1);
                DialogResult::Pending
            }
            KeyCode::Char('a') | KeyCode::Enter => DialogResult::Accept(self.text.clone()),
            KeyCode::Char('e') => DialogResult::Edit(self.text.clone()),
            KeyCode::Esc | KeyCode::Char('q') => DialogResult::Cancel,
            _ => DialogResult::Pending,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let popup_width = (area.width * 80 / 100).max(50).min(area.width.saturating_sub(4));
        let popup_height = (area.height * 80 / 100).max(12).min(area.height.saturating_sub(4));
        let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
        let popup_area = Rect::new(x, y, popup_width, popup_height);

        frame.render_widget(Clear, popup_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(self.border_color))
            .title(format!(" {} ", self.title))
            .title_style(Style::default().fg(self.border_color).add_modifier(Modifier::BOLD))
            .title_bottom(Line::from({
                let mut spans = vec![
                    Span::styled(" (a)", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    Span::styled("ccept ", Style::default().fg(Color::DarkGray)),
                    Span::styled("(e)", Style::default().fg(self.border_color).add_modifier(Modifier::BOLD)),
                    Span::styled("dit ", Style::default().fg(Color::DarkGray)),
                ];
                for (key, desc) in &self.extra_hints {
                    spans.push(Span::styled(key.clone(), Style::default().fg(self.border_color).add_modifier(Modifier::BOLD)));
                    spans.push(Span::styled(format!(":{desc} "), Style::default().fg(Color::DarkGray)));
                }
                spans.push(Span::styled("Esc", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)));
                spans.push(Span::styled(":cancel ", Style::default().fg(Color::DarkGray)));
                spans
            }));

        let content = if self.markdown {
            tui_markdown::from_str(&self.text)
        } else {
            ratatui::text::Text::raw(&self.text)
        };

        let paragraph = Paragraph::new(content)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll, 0));

        frame.render_widget(paragraph, popup_area);
    }
}
