use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};

use crate::app::App;
use crate::claude::REVIEW_SYSTEM;
use super::commits::render_commit_list;
use super::diff::render_diff;
use super::prompt_picker::render_prompt_picker;
use super::settings::render_settings;

pub fn render(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let top_bar = chunks[0];
    let main_area = chunks[1];
    let status_area = chunks[2];

    let pct = app.commit_panel_width;
    let panels = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(pct),
            Constraint::Percentage(100 - pct),
        ])
        .split(main_area);

    render_top_bar(frame, top_bar, app);

    // Calculate detail pane height based on commit content
    let detail_height = if let Some(idx) = app.selected_commit_index() {
        let commit = &app.pr.commits[idx];
        let body_lines = if commit.body.is_empty() { 0 } else { commit.body.lines().count() + 1 };
        let stat_lines = commit.diff.as_ref().map(|d| d.files.len() + 2).unwrap_or(0);
        // SHA + author + date + blank + summary + body + stats + 2 borders
        let content_height = 5 + body_lines + stat_lines + 2;
        let max_height = (panels[0].height / 2) as usize; // max half the panel
        (content_height.min(max_height).max(7)) as u16
    } else {
        7
    };

    // Split the left panel: commit list (top) + commit detail (bottom)
    let left_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(detail_height),
        ])
        .split(panels[0]);

    render_commit_list(frame, left_split[0], app);
    render_commit_detail(frame, left_split[1], app);
    render_diff(frame, panels[1], app);
    render_status_bar(frame, status_area, app);

    if app.show_help {
        render_help_popup(frame, app);
    }
    if let Some((_, dialog)) = &mut app.active_dialog {
        dialog.render(frame, frame.area());
    }
    if app.prompt_picker.is_some() {
        render_prompt_picker(frame, app);
    }
    if app.review_in_progress.is_some() {
        render_reviewing_overlay(frame, app);
    }
    if app.submit_in_progress {
        render_submit_overlay(frame, app);
    }
    if let Some(result) = &app.submit_result {
        render_submit_result(frame, result);
    }
    if app.show_pr_description {
        render_pr_description(frame, app);
    }
    if app.show_settings {
        render_settings(frame, app);
    }
    if app.show_system_prompt {
        render_system_prompt(frame, app);
    }
}

fn render_reviewing_overlay(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let w: u16 = 36;
    let h: u16 = 5;
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 2;
    let popup = Rect::new(x, y, w, h);

    frame.render_widget(Clear, popup);

    let yellow = ratatui::style::Color::Yellow;

    let spinner_set = throbber_widgets_tui::BRAILLE_SIX;
    let idx = app.spinner_state.index() as usize % spinner_set.symbols.len();
    let spinner_char = spinner_set.symbols[idx];

    let paragraph = Paragraph::new(vec![
        Line::raw(""),
        Line::from(vec![
            Span::styled(format!("{spinner_char} "), Style::default().fg(yellow)),
            Span::styled("Claude is reviewing...", Style::default().fg(yellow)),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(yellow)),
    )
    .alignment(ratatui::layout::Alignment::Center);

    frame.render_widget(paragraph, popup);
}

fn render_submit_overlay(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let w: u16 = 36;
    let h: u16 = 5;
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 2;
    let popup = Rect::new(x, y, w, h);

    frame.render_widget(Clear, popup);

    let green = ratatui::style::Color::Green;
    let spinner_set = throbber_widgets_tui::BRAILLE_SIX;
    let idx = app.spinner_state.index() as usize % spinner_set.symbols.len();
    let spinner_char = spinner_set.symbols[idx];

    let paragraph = Paragraph::new(vec![
        Line::raw(""),
        Line::from(vec![
            Span::styled(format!("{spinner_char} "), Style::default().fg(green)),
            Span::styled("Submitting to GitHub...", Style::default().fg(green)),
        ]),
    ])
    .block(Block::default().borders(Borders::ALL)
            .border_type(BorderType::Rounded).border_style(Style::default().fg(green)))
    .alignment(ratatui::layout::Alignment::Center);

    frame.render_widget(paragraph, popup);
}

fn render_submit_result(frame: &mut Frame, result: &Result<(), String>) {
    let area = frame.area();

    let (msg, color) = match result {
        Ok(()) => ("Review submitted successfully!".to_string(), ratatui::style::Color::Green),
        Err(e) => (format!("Submit failed: {}", e), ratatui::style::Color::Red),
    };

    let w = (area.width * 70 / 100).max(40).min(area.width.saturating_sub(4));
    // Estimate lines needed: message wrapped at (w - 4) chars + padding + footer
    let inner_w = (w - 4) as usize;
    let msg_lines = if inner_w > 0 { (msg.len() / inner_w) + 1 } else { 1 };
    let h = (msg_lines as u16 + 5).min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 2;
    let popup = Rect::new(x, y, w, h);

    frame.render_widget(Clear, popup);

    let paragraph = Paragraph::new(vec![
        Line::raw(""),
        Line::styled(&msg, Style::default().fg(color)),
        Line::raw(""),
        Line::styled("Press any key to continue", Style::default().fg(ratatui::style::Color::DarkGray)),
    ])
    .block(Block::default().borders(Borders::ALL)
            .border_type(BorderType::Rounded).border_style(Style::default().fg(color)))
    .alignment(ratatui::layout::Alignment::Center)
    .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, popup);
}

fn render_help_popup(frame: &mut Frame, app: &App) {
    let theme = &app.theme;
    let area = frame.area();

    let popup_width = (area.width * 50 / 100).max(45).min(area.width.saturating_sub(4));
    let popup_height = (area.height * 60 / 100).max(20).min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    let help_text = "\
Global
  ?          Show this help
  Tab        Switch panel (commits / diff)
  q / Esc    Quit
  p          Show PR description
  r          Claude review (full PR)
  A          Accept all pending comments
  X          Reject all pending comments
  s          Settings
  S          Submit review to GitHub

Commits panel
  j / k      Next / previous commit
  g / G      First / last commit
  l / Enter  Switch to diff panel

Diff panel
  j / k      Next / previous line
  Ctrl-D/U   Half page down / up
  g / G      First / last line
  n / N      Next / previous comment
  h          Switch to commits panel
  Enter      Open / add comment

Action dialog
  a / Enter  Accept
  e          Edit in $EDITOR
  /          Select prompt (review dialog)
  t          Cycle review type (submit)
  Esc        Cancel";

    let key_style = Style::default().fg(theme.status_fg).add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(theme.status_dim);
    let section_style = Style::default().fg(theme.status_accent).add_modifier(Modifier::BOLD);

    let mut lines: Vec<Line> = Vec::new();
    for raw_line in help_text.lines() {
        if raw_line.trim().is_empty() {
            lines.push(Line::raw(""));
        } else if !raw_line.starts_with(' ') {
            // Section header
            lines.push(Line::from(Span::styled(
                format!(" {}", raw_line.trim()),
                section_style,
            )));
        } else {
            // Key binding: split at column 13
            let split = 13.min(raw_line.len());
            let (key_part, desc_part) = raw_line.split_at(split);
            lines.push(Line::from(vec![
                Span::styled(key_part.to_string(), key_style),
                Span::styled(desc_part.to_string(), desc_style),
            ]));
        }
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.status_accent))
        .title(" Keybindings ")
        .title_style(Style::default().fg(theme.status_accent).add_modifier(Modifier::BOLD));

    frame.render_widget(Clear, popup_area);

    let paragraph = Paragraph::new(lines)
        .block(block);

    frame.render_widget(paragraph, popup_area);
}

fn render_top_bar(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let bar_style = Style::default().bg(theme.status_bar_bg);

    let left = Line::from(vec![
        Span::styled(
            format!(" {}/{}", app.owner, app.repo_name),
            Style::default().fg(theme.status_dim),
        ),
        Span::styled(
            format!("  PR #{}", app.pr.number),
            Style::default().fg(theme.status_accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {}  ", app.pr.title),
            Style::default().fg(theme.status_fg),
        ),
        Span::styled(
            format!("by {}", app.pr.author),
            Style::default().fg(theme.status_dim),
        ),
    ]);

    frame.render_widget(Block::default().style(bar_style), area);
    frame.render_widget(Paragraph::new(left).style(bar_style), area);
}

fn render_commit_detail(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let focused = app.focus == crate::app::Panel::Commits;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border_style(focused))
        .title(" Commit ");

    let Some(idx) = app.selected_commit_index() else {
        frame.render_widget(Paragraph::new("").block(block), area);
        return;
    };

    let commit = &app.pr.commits[idx];
    let dim = Style::default().fg(theme.status_dim);
    let fg = Style::default().fg(theme.status_fg);

    let mut lines = vec![
        Line::from(vec![
            Span::styled(&commit.sha, Style::default().fg(theme.sha)),
        ]),
        Line::from(vec![
            Span::styled("Author: ", dim),
            Span::styled(format!("{} <{}>", commit.author, commit.author_email), fg),
        ]),
        Line::from(vec![
            Span::styled("Date:   ", dim),
            Span::styled(&commit.date, fg),
        ]),
        Line::raw(""),
        Line::styled(format!("  {}", commit.summary), Style::default().fg(theme.status_fg).add_modifier(Modifier::BOLD)),
    ];

    if !commit.body.is_empty() {
        lines.push(Line::raw(""));
        for line in commit.body.lines() {
            lines.push(Line::styled(format!("  {}", line), fg));
        }
    }

    if let Some(diff) = &commit.diff {
        lines.push(Line::raw(""));
        for file in &diff.files {
            let bar_add = "+".repeat((file.additions as usize).min(20));
            let bar_del = "-".repeat((file.deletions as usize).min(20));
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", file.path), dim),
                Span::styled(bar_add, Style::default().fg(theme.addition_prefix)),
                Span::styled(bar_del, Style::default().fg(theme.deletion_prefix)),
            ]));
        }
        lines.push(Line::styled(
            format!("  {} file{}, +{} -{}",
                diff.files.len(),
                if diff.files.len() != 1 { "s" } else { "" },
                commit.stats.additions,
                commit.stats.deletions,
            ),
            dim,
        ));
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

fn render_status_bar(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let key_style = Style::default().fg(theme.status_fg).add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(theme.status_dim);
    let sep = Span::styled("  ", desc_style);

    let mut hints: Vec<Span> = vec![Span::styled(" ", desc_style)];

    let hint = |key: &str, desc: &str, hints: &mut Vec<Span>| {
        if hints.len() > 1 { hints.push(sep.clone()); }
        hints.push(Span::styled(key.to_string(), key_style));
        hints.push(Span::styled(format!(" {desc}"), desc_style));
    };

    if app.viewing_comment.is_some() {
        // Viewing an inline comment — only comment actions work
        hint("a", "accept", &mut hints);
        hint("e", "edit", &mut hints);
        hint("x", "reject", &mut hints);
        hint("Esc", "close", &mut hints);
    } else {
        let has_reviews = !app.reviews.is_empty();

        match app.focus {
            crate::app::Panel::Commits => {
                hint("j/k", "navigate", &mut hints);
                hint("Enter", "open", &mut hints);
                hint("Tab", "diff", &mut hints);
                hint("p", "PR desc", &mut hints);
            }
            crate::app::Panel::Diff => {
                hint("j/k", "navigate", &mut hints);
                if has_reviews {
                    hint("n/N", "next/prev", &mut hints);
                }
                hint("Enter", "comment", &mut hints);
                hint("Tab", "commits", &mut hints);
                hint("p", "PR desc", &mut hints);
            }
        }

        hint("r", "review", &mut hints);
        if has_reviews {
            hint("A/X", "all ok/no", &mut hints);
            hint("S", "submit", &mut hints);
        }
    }

    let mut right_hints: Vec<Span> = vec![
        Span::styled("│ ", Style::default().fg(theme.status_dim)),
    ];
    hint("?", "help", &mut right_hints);
    hint("s", "settings", &mut right_hints);
    hint("q", "quit", &mut right_hints);
    right_hints.push(Span::styled(" ", desc_style));

    let left = Line::from(hints);
    let right = Line::from(right_hints);

    let bar_style = Style::default().bg(theme.status_bar_bg);
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Min(1)])
        .split(area);

    frame.render_widget(Block::default().style(bar_style), area);
    frame.render_widget(Paragraph::new(left).style(bar_style), chunks[0]);
    frame.render_widget(
        Paragraph::new(right).style(bar_style).alignment(ratatui::layout::Alignment::Right),
        chunks[1],
    );
}

fn render_pr_description(frame: &mut Frame, app: &App) {
    let theme = &app.theme;
    let area = frame.area();

    let popup_width = (area.width * 75 / 100).max(50).min(area.width.saturating_sub(4));
    let popup_height = (area.height * 70 / 100).max(12).min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let content = if app.pr.description.is_empty() {
        "No description provided.".to_string()
    } else {
        app.pr.description.clone()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.status_accent))
        .title(format!(" PR #{}: {} ", app.pr.number, app.pr.title))
        .title_style(Style::default().fg(theme.status_accent).add_modifier(Modifier::BOLD))
        .title_bottom(Line::from(vec![
            Span::styled(" Esc", Style::default().fg(theme.status_fg).add_modifier(Modifier::BOLD)),
            Span::styled(": close ", Style::default().fg(theme.status_dim)),
        ]));

    let paragraph = Paragraph::new(tui_markdown::from_str(&content))
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, popup_area);
}

fn render_system_prompt(frame: &mut Frame, app: &App) {
    let theme = &app.theme;
    let area = frame.area();

    let popup_width = (area.width * 80 / 100).max(50).min(area.width.saturating_sub(4));
    let popup_height = (area.height * 80 / 100).max(12).min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.status_accent))
        .title(" System Prompt ")
        .title_style(Style::default().fg(theme.status_accent).add_modifier(Modifier::BOLD))
        .title_bottom(Line::from(vec![
            Span::styled(" Esc", Style::default().fg(theme.status_fg).add_modifier(Modifier::BOLD)),
            Span::styled(": close ", Style::default().fg(theme.status_dim)),
        ]));

    let paragraph = Paragraph::new(REVIEW_SYSTEM)
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, popup_area);
}
