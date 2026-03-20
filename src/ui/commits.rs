use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem};

use crate::app::{App, CommitListEntry, CommitReviewState, Panel};
use crate::review::{CommentStatus, Severity};

pub fn render_commit_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let focused = app.focus == Panel::Commits;
    let theme = &app.theme;

    let mut fully_reviewed = 0usize;
    let mut items: Vec<ListItem> = Vec::new();
    let mut entries: Vec<CommitListEntry> = Vec::new();

    for (i, c) in app.pr.commits.iter().enumerate() {
        let state = app.commit_review_state(i);
        let (marker, marker_color) = match state {
            CommitReviewState::NotReviewed => ("[-] ".to_string(), theme.commit_not_reviewed),
            CommitReviewState::InProgress => {
                let review = app.reviews.get(&i).unwrap();
                let total = review.comments.len();
                let addressed = review.comments.iter()
                    .filter(|c| c.status == CommentStatus::Accepted || c.status == CommentStatus::Rejected)
                    .count();
                (format!("[{}/{}] ", addressed, total), theme.commit_in_progress)
            }
            CommitReviewState::FullyReviewed => {
                fully_reviewed += 1;
                let total = app.reviews.get(&i).map(|r| r.comments.len()).unwrap_or(0);
                (format!("[{}/{}] ", total, total), theme.commit_fully_reviewed)
            }
        };

        // Commit row
        items.push(ListItem::new(Line::from(vec![
            Span::styled(marker, Style::default().fg(marker_color)),
            Span::styled(
                format!("{} ", c.short_sha),
                Style::default().fg(theme.sha),
            ),
            Span::styled(&c.summary, Style::default().add_modifier(Modifier::BOLD)),
        ])));
        entries.push(CommitListEntry::Commit(i));

        // Comment rows (if review exists)
        if let Some(review) = app.reviews.get(&i) {
            for (ci, comment) in review.comments.iter().enumerate() {
                let (status_marker, status_color) = match comment.status {
                    CommentStatus::Accepted => ("✓", theme.comment_accepted_marker),
                    CommentStatus::Rejected => ("✗", theme.comment_rejected_marker),
                    CommentStatus::Pending => match comment.severity {
                        Severity::Critical => ("●", theme.severity_critical),
                        Severity::Warning => ("●", theme.severity_warning),
                        Severity::Suggestion => ("●", theme.severity_suggestion),
                        Severity::Nitpick => ("●", theme.severity_nitpick),
                    },
                };

                let location = if comment.line == 0 {
                    comment.file.clone()
                } else {
                    format!("{}:{}", comment.file, comment.line)
                };

                items.push(ListItem::new(Line::from(vec![
                    Span::raw("   "),
                    Span::styled(format!("{} ", status_marker), Style::default().fg(status_color)),
                    Span::styled(location, Style::default().fg(theme.status_dim)),
                    Span::styled(format!(" {}", comment.severity), Style::default().fg(status_color)),
                ])).style(Style::default().add_modifier(Modifier::DIM)));
                entries.push(CommitListEntry::Comment(i, ci));
            }
        }
    }

    app.commit_list_entries = entries;

    let title = if fully_reviewed > 0 {
        format!(" Commits ({}/{} reviewed) ", fully_reviewed, app.pr.commits.len())
    } else {
        format!(" Commits ({}) ", app.pr.commits.len())
    };
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme.border_style(focused))
                .title(title),
        )
        .highlight_style(theme.cursor_style(focused))
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, area, &mut app.commit_state);
}
