use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

use crate::app::App;

pub fn render_prompt_picker(frame: &mut Frame, app: &App) {
    let Some(picker) = &app.prompt_picker else { return };
    let theme = &app.theme;
    let area = frame.area();

    let popup_width = (area.width * 70 / 100).max(45).min(area.width.saturating_sub(4));
    let popup_height = (picker.entries.len() as u16 + 4).min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let mut lines: Vec<Line> = vec![Line::raw("")];

    for (i, entry) in picker.entries.iter().enumerate() {
        let is_selected = i == picker.selected;
        let marker = if is_selected { "▶ " } else { "  " };
        let marker_style = if is_selected {
            Style::default().fg(theme.status_accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let name_style = if is_selected {
            Style::default().fg(theme.status_fg).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.status_fg)
        };

        let mut spans = vec![
            Span::styled(marker, marker_style),
            Span::styled(&entry.name, name_style),
        ];

        if let Some(path) = &entry.path {
            spans.push(Span::styled(
                format!("  {}", path),
                Style::default().fg(theme.status_dim),
            ));
        }

        lines.push(Line::from(spans));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.status_accent))
        .title(" Select Prompt ")
        .title_style(Style::default().fg(theme.status_accent).add_modifier(Modifier::BOLD))
        .title_bottom(Line::from(vec![
            Span::styled(" Enter", Style::default().fg(theme.status_fg).add_modifier(Modifier::BOLD)),
            Span::styled(": select ", Style::default().fg(theme.status_dim)),
            Span::styled("Esc", Style::default().fg(theme.status_fg).add_modifier(Modifier::BOLD)),
            Span::styled(": cancel ", Style::default().fg(theme.status_dim)),
        ]));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, popup_area);
}
