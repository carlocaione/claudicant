use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

use crate::app::App;
use crate::settings::SettingsRow;

pub fn render_settings(frame: &mut Frame, app: &App) {
    let theme = &app.theme;
    let area = frame.area();

    let popup_width = (area.width * 55 / 100).max(50).min(area.width.saturating_sub(4));
    let popup_height = 14u16.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let selected = app.settings_row;
    let settings = &app.settings;
    let label_style = Style::default().fg(theme.status_dim);
    let value_style = Style::default().fg(theme.status_fg).add_modifier(Modifier::BOLD);
    let hint_style = Style::default().fg(theme.status_dim);

    let version = format!("claudicant {}", env!("CARGO_PKG_VERSION"));

    let rows: Vec<(SettingsRow, &str, String, &str)> = vec![
        (SettingsRow::Model, "Model", settings.model.display().to_string(), "</>: cycle"),
        (SettingsRow::Effort, "Effort", settings.effort.display().to_string(), "</>: cycle"),
        (SettingsRow::FastMode, "Fast mode", if settings.fast_mode { "on" } else { "off" }.to_string(), "Enter: toggle"),
        (SettingsRow::ViewPrompt, "System prompt", "view...".to_string(), "Enter: open"),
        (SettingsRow::Version, "Version", version, ""),
    ];

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::raw(""));

    for (row, label, value, hint) in &rows {
        let is_selected = *row == selected;
        let marker = if is_selected { "▶ " } else { "  " };
        let marker_style = if is_selected {
            Style::default().fg(theme.status_accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let val_style = if is_selected {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            value_style
        };

        let mut spans = vec![
            Span::styled(marker, marker_style),
            Span::styled(format!("{:15}", label), label_style),
            Span::styled(value.as_str(), val_style),
        ];

        if is_selected && !hint.is_empty() {
            spans.push(Span::styled(format!("  {hint}"), hint_style));
        }

        lines.push(Line::from(spans));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.status_accent))
        .title(" Settings ")
        .title_style(Style::default().fg(theme.status_accent).add_modifier(Modifier::BOLD))
        .title_bottom(Line::from(vec![
            Span::styled(" j/k", Style::default().fg(theme.status_fg).add_modifier(Modifier::BOLD)),
            Span::styled(": navigate ", Style::default().fg(theme.status_dim)),
            Span::styled("Esc", Style::default().fg(theme.status_fg).add_modifier(Modifier::BOLD)),
            Span::styled(": close ", Style::default().fg(theme.status_dim)),
        ]));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, popup_area);
}
