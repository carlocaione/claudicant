use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph};

use crate::app::{App, DiffLineMetadata, Panel};
use crate::github::models::DiffLineKind;
use crate::review::Severity;
use super::highlight::extension_from_path;

pub fn render_diff(frame: &mut Frame, area: Rect, app: &mut App) {
    let focused = app.focus == Panel::Diff;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(app.theme.border_style(focused));

    let Some(idx) = app.selected_commit_index() else {
        let paragraph = Paragraph::new("No commit selected").block(block.title(" Diff "));
        frame.render_widget(paragraph, area);
        return;
    };

    let commit = &app.pr.commits[idx];
    let title = format!(" {} {} ", commit.short_sha, commit.summary);
    let block = block.title(title);

    let Some(diff) = &commit.diff else {
        let paragraph = Paragraph::new("No diff available").block(block);
        frame.render_widget(paragraph, area);
        return;
    };

    let theme = &app.theme;
    let review = app.reviews.get(&idx);
    let mut items: Vec<ListItem> = Vec::new();
    let mut line_info: Vec<DiffLineMetadata> = Vec::new();
    let mut comment_last_line: Option<usize> = None;
    // Available width for comment text (area - borders - "  " prefix)
    let comment_wrap_width = (area.width as usize).saturating_sub(6);
    let box_width = comment_wrap_width + 4;

    for (file_idx, file) in diff.files.iter().enumerate() {
        let ext = extension_from_path(&file.path);

        if file_idx > 0 {
            items.push(ListItem::new(Line::raw("")));
            line_info.push(DiffLineMetadata {
                file_path: file.path.clone(),
                lineno: 0,
                old_lineno: None,
                new_lineno: None,
                is_code_line: false,
            });
        }

        // Build a set of line numbers present in this file's diff hunks
        let diff_lines_in_file: std::collections::HashSet<u32> = file.hunks.iter()
            .flat_map(|h| h.lines.iter())
            .flat_map(|l| l.old_lineno.into_iter().chain(l.new_lineno.into_iter()))
            .collect();

        // Check for file-level comments (line == 0) OR comments whose line
        // doesn't appear in the diff (unmatched — shown on file header)
        let file_header_comment = review.and_then(|r| {
            r.comments.iter().enumerate().find(|(_, c)| {
                c.file == file.path && (c.line == 0 || !diff_lines_in_file.contains(&c.line))
            })
        });

        let file_name = match &file.old_path {
            Some(old) => format!("─── {} → {} [{}] ───", old, file.path, file.status),
            None => format!("─── {} [{}] ───", file.path, file.status),
        };
        let file_marker = if let Some((_, comment)) = file_header_comment {
            match comment.status {
                crate::review::CommentStatus::Accepted => ("✓ ", theme.comment_accepted_marker),
                crate::review::CommentStatus::Rejected => ("✗ ", theme.comment_rejected_marker),
                crate::review::CommentStatus::Pending => match comment.severity {
                    Severity::Critical => ("● ", theme.deletion_prefix),
                    Severity::Warning => ("● ", theme.file_header),
                    Severity::Suggestion => ("● ", theme.status_accent),
                    Severity::Nitpick => ("● ", theme.gutter),
                },
            }
        } else {
            ("  ", theme.file_header)
        };
        items.push(ListItem::new(Line::from(vec![
            Span::styled(file_marker.0, Style::default().fg(file_marker.1)),
            Span::styled(
                file_name,
                Style::default().fg(theme.file_header).add_modifier(Modifier::BOLD),
            ),
        ])).style(Style::default().bg(theme.file_header_bg)));
        line_info.push(DiffLineMetadata {
            file_path: file.path.clone(),
            lineno: 0,
            old_lineno: None,
            new_lineno: None,
            is_code_line: true,
        });

        // Expand file-level or unmatched comment if viewing
        if let Some((comment_idx, comment)) = file_header_comment {
            if app.viewing_comment == Some((idx, comment_idx)) {
                let severity_color = match comment.severity {
                    Severity::Critical => theme.deletion_prefix,
                    Severity::Warning => theme.file_header,
                    Severity::Suggestion => theme.status_accent,
                    Severity::Nitpick => theme.gutter,
                };
                render_inline_comment(
                    &mut items, &mut line_info, &mut comment_last_line,
                    app, comment, severity_color,
                    &file.path, 0, idx, comment_idx,
                    comment_wrap_width, box_width,
                );
            }
        }

        for hunk in &file.hunks {
            for diff_line in &hunk.lines {
                let lineno = match diff_line.kind {
                    DiffLineKind::Addition => diff_line.new_lineno.unwrap_or(0),
                    DiffLineKind::Deletion | DiffLineKind::Context => {
                        diff_line.old_lineno.unwrap_or(0)
                    }
                };

                let old_no = diff_line
                    .old_lineno
                    .map(|n| format!("{:>4}", n))
                    .unwrap_or_else(|| "    ".to_string());
                let new_no = diff_line
                    .new_lineno
                    .map(|n| format!("{:>4}", n))
                    .unwrap_or_else(|| "    ".to_string());

                let prefix = match diff_line.kind {
                    DiffLineKind::Addition => "+ ",
                    DiffLineKind::Deletion => "- ",
                    DiffLineKind::Context => "  ",
                };

                // Check if there's a review comment on this line
                // Match based on the comment's `side` field
                let comment_match = review.and_then(|r| {
                    r.comments.iter().enumerate().find(|(_, c)| {
                        if c.file != file.path || c.line == 0 {
                            return false;
                        }
                        match c.side {
                            crate::review::CommentSide::New => Some(c.line) == diff_line.new_lineno,
                            crate::review::CommentSide::Old => Some(c.line) == diff_line.old_lineno,
                            crate::review::CommentSide::File => false, // handled at file header
                        }
                    })
                });
                let comment_marker = comment_match.map(|(_, c)| c);

                let mut spans = Vec::new();

                // Comment marker — reflects both severity and review status
                if let Some(comment) = comment_marker {
                    let (marker, color) = match comment.status {
                        crate::review::CommentStatus::Accepted => ("✓ ", theme.comment_accepted_marker),
                        crate::review::CommentStatus::Rejected => ("✗ ", theme.comment_rejected_marker),
                        crate::review::CommentStatus::Pending => match comment.severity {
                            Severity::Critical => ("● ", theme.deletion_prefix),
                            Severity::Warning => ("● ", theme.file_header),
                            Severity::Suggestion => ("● ", theme.status_accent),
                            Severity::Nitpick => ("● ", theme.gutter),
                        },
                    };
                    spans.push(Span::styled(marker, Style::default().fg(color)));
                } else {
                    spans.push(Span::raw("  "));
                }

                spans.push(Span::styled(
                    format!("{old_no} {new_no}"),
                    Style::default().fg(theme.gutter),
                ));
                spans.push(Span::styled(
                    " │ ",
                    Style::default().fg(theme.gutter),
                ));
                spans.push(Span::styled(
                    prefix.to_string(),
                    match diff_line.kind {
                        DiffLineKind::Addition => Style::default().fg(theme.addition_prefix),
                        DiffLineKind::Deletion => Style::default().fg(theme.deletion_prefix),
                        DiffLineKind::Context => Style::default(),
                    },
                ));

                let highlighted = app.highlighter.highlight_line(
                    &diff_line.content,
                    ext,
                    diff_line.kind,
                );
                if !highlighted.is_empty() {
                    spans.extend(highlighted);
                }

                let line_style = match diff_line.kind {
                    DiffLineKind::Addition => Style::default().bg(theme.addition_bg),
                    DiffLineKind::Deletion => Style::default().bg(theme.deletion_bg),
                    DiffLineKind::Context => Style::default(),
                };
                items.push(ListItem::new(Line::from(spans)).style(line_style));
                line_info.push(DiffLineMetadata {
                    file_path: file.path.clone(),
                    lineno,
                    old_lineno: diff_line.old_lineno,
                    new_lineno: diff_line.new_lineno,
                    is_code_line: true,
                    });

                // Insert comment lines if this comment is currently being viewed
                if let Some((comment_idx, comment)) = comment_match {
                    if app.viewing_comment == Some((idx, comment_idx)) {
                        let severity_color = match comment.severity {
                            Severity::Critical => theme.deletion_prefix,
                            Severity::Warning => theme.file_header,
                            Severity::Suggestion => theme.status_accent,
                            Severity::Nitpick => theme.gutter,
                        };
                        render_inline_comment(
                            &mut items, &mut line_info, &mut comment_last_line,
                            app, comment, severity_color,
                            &file.path, lineno, idx, comment_idx,
                            comment_wrap_width, box_width,
                        );
                    }
                }
            }
        }
    }

    app.diff_line_count = items.len();
    app.diff_line_info = line_info;
    app.rebuild_diff_line_index();

    if app.diff_needs_reposition {
        app.diff_state.select(Some(0));
        app.diff_needs_reposition = false;
    }

    // When viewing a comment, select the last comment line so the list
    // auto-scrolls to show the full comment box. The cursor highlight
    // is hidden so this selection is invisible to the user.
    if let Some(last) = comment_last_line {
        app.diff_state.select(Some(last));
    }

    // Center the selected line in the visible area
    if let Some(selected) = app.diff_state.selected() {
        let visible_height = area.height.saturating_sub(2) as usize; // minus borders
        if visible_height > 0 {
            let ideal_offset = selected.saturating_sub(visible_height / 2);
            *app.diff_state.offset_mut() = ideal_offset;
        }
    }

    let show_cursor = focused && app.viewing_comment.is_none();
    let list = List::new(items)
        .block(block)
        .highlight_style(app.theme.cursor_style(show_cursor));

    frame.render_stateful_widget(list, area, &mut app.diff_state);
}

#[allow(clippy::too_many_arguments)]
fn render_inline_comment(
    items: &mut Vec<ListItem<'static>>,
    line_info: &mut Vec<DiffLineMetadata>,
    comment_last_line: &mut Option<usize>,
    _app: &App,
    comment: &crate::review::ReviewComment,
    severity_color: ratatui::style::Color,
    file_path: &str,
    lineno: u32,
    _commit_idx: usize,
    _comment_idx: usize,
    comment_wrap_width: usize,
    box_width: usize,
) {
    let comment_style = Style::default().fg(severity_color);

    let push_meta = |line_info: &mut Vec<DiffLineMetadata>| {
        line_info.push(DiffLineMetadata {
            file_path: file_path.to_string(),
            lineno,
            old_lineno: None,
            new_lineno: None,
            is_code_line: false,
        });
    };

    // Top border
    let title_text = if lineno == 0 {
        format!(" {} [file] ", comment.severity)
    } else {
        format!(" {} [{}:{}] ", comment.severity, comment.file, lineno)
    };
    let remaining = box_width.saturating_sub(2 + title_text.len());
    let top = format!("  ──{}{}", title_text, "─".repeat(remaining));
    items.push(ListItem::new(Line::from(Span::styled(top, comment_style))));
    push_meta(line_info);

    // Top padding
    items.push(ListItem::new(Line::raw("")));
    push_meta(line_info);

    // Content
    let md_text = tui_markdown::from_str(comment.display_comment());
    for md_line in md_text.lines {
        let plain: String = md_line.spans.iter().map(|s| s.content.as_ref()).collect();
        let wrapped_lines = wrap_text(&plain, comment_wrap_width);

        if wrapped_lines.len() <= 1 {
            let mut spans = vec![Span::styled("  ", comment_style)];
            spans.extend(md_line.spans.into_iter().map(|s| {
                Span::styled(s.content.to_string(), s.style)
            }));
            items.push(ListItem::new(Line::from(spans)));
            push_meta(line_info);
        } else {
            for w in wrapped_lines {
                items.push(ListItem::new(Line::from(vec![
                    Span::styled("  ", comment_style),
                    Span::raw(w),
                ])));
                push_meta(line_info);
            }
        }
    }

    // Bottom padding
    items.push(ListItem::new(Line::raw("")));
    push_meta(line_info);

    // Bottom border with actions
    let status_text = match comment.status {
        crate::review::CommentStatus::Pending => "pending",
        crate::review::CommentStatus::Accepted => "✓ accepted",
        crate::review::CommentStatus::Rejected => "✗ rejected",
    };
    let actions_len = " (a)ccept (e)dit (x)reject Esc:close  ".len() + status_text.len() + 2;
    let fill = box_width.saturating_sub(2 + actions_len);
    items.push(ListItem::new(Line::from(vec![
        Span::styled("  ── ", comment_style),
        Span::styled("(a)", Style::default().fg(ratatui::style::Color::Green).add_modifier(Modifier::BOLD)),
        Span::styled("ccept ", Style::default().fg(ratatui::style::Color::DarkGray)),
        Span::styled("(e)", Style::default().fg(severity_color).add_modifier(Modifier::BOLD)),
        Span::styled("dit ", Style::default().fg(ratatui::style::Color::DarkGray)),
        Span::styled("(x)", Style::default().fg(ratatui::style::Color::Red).add_modifier(Modifier::BOLD)),
        Span::styled("reject ", Style::default().fg(ratatui::style::Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(ratatui::style::Color::DarkGray).add_modifier(Modifier::BOLD)),
        Span::styled(":close ", Style::default().fg(ratatui::style::Color::DarkGray)),
        Span::styled(format!("│ {} ", status_text), comment_style),
        Span::styled("─".repeat(fill), comment_style),
    ])));
    push_meta(line_info);

    *comment_last_line = Some(items.len() - 1);
}

fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 || text.is_empty() {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current = word.to_string();
        } else if current.len() + 1 + word.len() <= max_width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current = word.to_string();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

