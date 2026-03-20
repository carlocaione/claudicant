use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;

use crate::claude;
use crate::event::AppEvent;
use crate::github::models::PullRequest;
use crate::prompts::{self, PromptEntry};
use crate::review::{CommentStatus, Review};
use crate::settings::{Settings, SettingsRow};
use crate::theme::Theme;
use crate::ui::action_dialog::{ActionDialog, DialogResult};
use crate::ui::highlight::Highlighter;

/// Action returned by on_key to signal side-effects to the main loop.
pub enum AppAction {
    None,
    /// Open $EDITOR with this text. When editor closes, call on_editor_done().
    OpenEditor(String),
}

#[derive(PartialEq)]
pub enum Panel {
    Commits,
    Diff,
}

#[derive(PartialEq)]
pub enum CommitReviewState {
    NotReviewed,
    InProgress,
    FullyReviewed,
}

#[derive(Clone, Default)]
pub struct DiffLineMetadata {
    pub file_path: String,
    pub lineno: u32,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
    pub is_code_line: bool,
}

/// What each row in the commit list panel represents.
pub enum CommitListEntry {
    Commit(usize),              // commit index
    Comment(usize, usize),      // (commit_idx, comment_idx)
}

/// GitHub review event type — determines how the review affects the PR.
#[derive(Clone, Copy, PartialEq)]
pub enum ReviewEventType {
    Comment,
    Approve,
    RequestChanges,
}

impl ReviewEventType {
    pub fn next(self) -> Self {
        match self {
            Self::Comment => Self::Approve,
            Self::Approve => Self::RequestChanges,
            Self::RequestChanges => Self::Comment,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Comment => "COMMENT",
            Self::Approve => "APPROVE",
            Self::RequestChanges => "REQUEST_CHANGES",
        }
    }

    pub fn border_color(self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            Self::Comment => Color::Cyan,
            Self::Approve => Color::Green,
            Self::RequestChanges => Color::Red,
        }
    }
}

impl std::fmt::Display for ReviewEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Comment => write!(f, "Comment"),
            Self::Approve => write!(f, "Approve"),
            Self::RequestChanges => write!(f, "Request Changes"),
        }
    }
}

/// What the active dialog is for.
pub enum DialogContext {
    /// Editing a prompt before sending to Claude (commit index or usize::MAX for full PR)
    ReviewPrompt(usize),
    /// Reviewing the summary before submitting to GitHub.
    /// Carries (api_comments, event_type) collected at dialog open time.
    SubmitReview(Vec<crate::github::client::ReviewApiComment>, ReviewEventType),
}

pub struct App {
    pub pr: PullRequest,
    pub focus: Panel,
    pub log_file: Option<std::path::PathBuf>,
    pub owner: String,
    pub repo_name: String,
    pub github_token: String,
    pub submit_in_progress: bool,
    pub submit_result: Option<Result<(), String>>,
    pub commit_state: ListState,
    pub diff_state: ListState,
    pub diff_line_count: usize,
    pub diff_needs_reposition: bool,
    pub show_help: bool,
    pub reviews: HashMap<usize, Review>,
    pub review_in_progress: Option<usize>,
    /// PID of the running claude process (shared with background thread for cancellation)
    pub review_pid: Arc<AtomicU32>,
    pub spinner_state: throbber_widgets_tui::ThrobberState,
    /// Currently open review comment: (commit_idx, comment_idx)
    pub viewing_comment: Option<(usize, usize)>,
    /// Cursor position before comment was opened (to restore on close)
    pub pre_comment_cursor: Option<usize>,
    /// Pending new user comment context: (commit_idx, file_path, line, side)
    pub pending_new_comment: Option<(usize, String, u32, crate::review::CommentSide)>,
    pub active_dialog: Option<(DialogContext, ActionDialog)>,
    pub commit_list_entries: Vec<CommitListEntry>,
    pub diff_line_info: Vec<DiffLineMetadata>,
    /// Precomputed: set of new_lineno values per file in current diff (for unmatched comment detection)
    pub diff_new_lines_by_file: HashMap<String, std::collections::HashSet<u32>>,
    pub theme: Theme,
    pub highlighter: Highlighter,
    pub should_quit: bool,
    pub show_settings: bool,
    pub settings_row: SettingsRow,
    pub settings: Settings,
    pub show_system_prompt: bool,
    pub show_pr_description: bool,
    pub commit_panel_width: u16,
    pub repo_path: Option<std::path::PathBuf>,
    #[allow(dead_code)]
    pub default_prompt: Option<String>,
    /// Currently active prompt name (tracks user's selection via picker)
    pub active_prompt: Option<String>,
    /// Prompt picker state (shown over the review prompt dialog)
    pub prompt_picker: Option<PromptPicker>,
}

pub struct PromptPicker {
    pub entries: Vec<PromptEntry>,
    pub selected: usize,
}

impl App {
    pub fn new(
        pr: PullRequest,
        theme: Theme,
        log_file: Option<std::path::PathBuf>,
        owner: String,
        repo_name: String,
        github_token: String,
        repo_path: Option<std::path::PathBuf>,
        settings: crate::settings::Settings,
        default_prompt: Option<String>,
        commit_panel_width: u16,
    ) -> Self {
        let mut commit_state = ListState::default();
        if !pr.commits.is_empty() {
            commit_state.select(Some(0));
        }
        let highlighter = Highlighter::new(&theme.syntect_theme);
        Self {
            pr,
            focus: Panel::Commits,
            log_file,
            owner,
            repo_name,
            github_token,
            submit_in_progress: false,
            submit_result: None,
            commit_state,
            diff_state: ListState::default(),
            diff_line_count: 0,
            diff_needs_reposition: false,
            show_help: false,
            reviews: HashMap::new(),
            review_in_progress: None,
            review_pid: Arc::new(AtomicU32::new(0)),
            spinner_state: throbber_widgets_tui::ThrobberState::default(),
            viewing_comment: None,
            pre_comment_cursor: None,
            pending_new_comment: None,
            active_dialog: None,
            commit_list_entries: Vec::new(),
            diff_line_info: Vec::new(),
            diff_new_lines_by_file: HashMap::new(),
            theme,
            highlighter,
            should_quit: false,
            show_settings: false,
            settings_row: SettingsRow::Model,
            settings,
            show_system_prompt: false,
            show_pr_description: false,
            commit_panel_width,
            repo_path,
            active_prompt: default_prompt.clone(),
            default_prompt,
            prompt_picker: None,
        }
    }

    pub fn commit_review_state(&self, idx: usize) -> CommitReviewState {
        if self.review_in_progress == Some(idx) {
            return CommitReviewState::InProgress;
        }
        let Some(review) = self.reviews.get(&idx) else {
            return CommitReviewState::NotReviewed;
        };
        if review.comments.is_empty() {
            return CommitReviewState::FullyReviewed;
        }
        let all_addressed = review.comments.iter().all(|c| {
            c.status == CommentStatus::Accepted || c.status == CommentStatus::Rejected
        });
        if all_addressed {
            CommitReviewState::FullyReviewed
        } else {
            CommitReviewState::InProgress
        }
    }

    pub fn tick(&mut self) {
        if self.review_in_progress.is_some() || self.submit_in_progress {
            self.spinner_state.calc_next();
        }
    }

    pub fn selected_commit_index(&self) -> Option<usize> {
        let selected = self.commit_state.selected()?;
        // Scan backward from the selected position to find the parent commit
        for i in (0..=selected).rev() {
            if let Some(CommitListEntry::Commit(idx)) = self.commit_list_entries.get(i) {
                return Some(*idx);
            }
        }
        // Fallback: if commit_list_entries is empty (before first render), use raw index
        if self.commit_list_entries.is_empty() {
            return self.commit_state.selected();
        }
        None
    }

    /// Returns the selected entry in the commit list (commit or comment).
    pub fn selected_commit_list_entry(&self) -> Option<&CommitListEntry> {
        let selected = self.commit_state.selected()?;
        self.commit_list_entries.get(selected)
    }

    pub fn on_key(&mut self, key: KeyEvent, event_tx: &mpsc::Sender<AppEvent>) -> AppAction {
        // Block all input while submit is in progress
        if self.submit_in_progress {
            return AppAction::None;
        }

        // Allow Esc to cancel a running review
        if self.review_in_progress.is_some() {
            if key.code == KeyCode::Esc || key.code == KeyCode::Char('q') {
                let pid = self.review_pid.load(Ordering::Relaxed);
                if pid > 0 {
                    let _ = std::process::Command::new("kill").arg(pid.to_string()).status();
                }
                self.review_in_progress = None;
                self.review_pid.store(0, Ordering::Relaxed);
            }
            return AppAction::None;
        }

        // Dismiss submit result on any key
        if self.submit_result.is_some() {
            self.submit_result = None;
            return AppAction::None;
        }

        // Active dialog captures all input
        if self.active_dialog.is_some() {
            return self.on_key_dialog(key, event_tx);
        }

        // PR description popup (read-only)
        if self.show_pr_description {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('p') => {
                    self.show_pr_description = false;
                }
                _ => {}
            }
            return AppAction::None;
        }

        // System prompt popup (read-only)
        if self.show_system_prompt {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => {
                    self.show_system_prompt = false;
                }
                _ => {}
            }
            return AppAction::None;
        }

        // Settings popup
        if self.show_settings {
            return self.on_key_settings(key);
        }

        // Help popup
        if self.show_help {
            match key.code {
                KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => {
                    self.show_help = false;
                }
                _ => {}
            }
            return AppAction::None;
        }

        // Global keys
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Esc if self.focus == Panel::Commits => self.should_quit = true,
            KeyCode::Tab => self.toggle_focus(),
            KeyCode::Char('p') => self.show_pr_description = true,
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Char('r') => self.open_pr_review_dialog(),
            KeyCode::Char('s') => self.show_settings = true,
            KeyCode::Char('S') => self.submit_to_github(event_tx),
            KeyCode::Char('A') => self.batch_set_all_comments(CommentStatus::Accepted),
            KeyCode::Char('X') => self.batch_set_all_comments(CommentStatus::Rejected),
            _ => match self.focus {
                Panel::Commits => { self.on_key_commits(key); },
                Panel::Diff => return self.on_key_diff(key),
            },
        }

        AppAction::None
    }

    // -- Dialog handling --

    fn on_key_dialog(&mut self, key: KeyEvent, event_tx: &mpsc::Sender<AppEvent>) -> AppAction  {
        // Prompt picker is shown over the review prompt dialog
        if self.prompt_picker.is_some() {
            return self.on_key_prompt_picker(key);
        }

        // Intercept '/' to open prompt picker in review prompt dialog
        if key.code == KeyCode::Char('/') {
            if let Some((DialogContext::ReviewPrompt(_), _)) = &self.active_dialog {
                let entries = prompts::load_prompts(self.repo_path.as_deref());
                if entries.len() > 1 {
                    let default_idx = self.active_prompt.as_ref()
                        .and_then(|name| entries.iter().position(|e| e.name == *name))
                        .unwrap_or(0);
                    self.prompt_picker = Some(PromptPicker {
                        entries,
                        selected: default_idx,
                    });
                }
                return AppAction::None;
            }
        }

        // Intercept 't' to cycle review event type in submit dialog
        if key.code == KeyCode::Char('t') {
            if let Some((DialogContext::SubmitReview(_, ref mut event_type), dialog)) = &mut self.active_dialog {
                *event_type = event_type.next();
                let color = event_type.border_color();
                dialog.set_title(&format!("Submit Review [{}]", event_type));
                dialog.set_border_color(color);
                return AppAction::None;
            }
        }

        let Some((_, dialog)) = &mut self.active_dialog else {
            return AppAction::None;
        };

        let result = dialog.on_key(key);

        match result {
            DialogResult::Pending => AppAction::None,
            DialogResult::Accept(text) => {
                let (ctx, _) = self.active_dialog.take().unwrap();
                self.on_dialog_accept(ctx, text, event_tx);
                AppAction::None
            }
            DialogResult::Edit(text) => {
                // Dialog stays open — main loop opens $EDITOR, then calls on_editor_done()
                AppAction::OpenEditor(text)
            }
            DialogResult::Cancel => {
                self.active_dialog = None;
                AppAction::None
            }
        }
    }

    /// Called by main loop after $EDITOR closes with the (possibly modified) text.
    pub fn on_editor_done(&mut self, new_text: String) {
        // Creating a new user comment
        if let Some((commit_idx, file_path, line, side)) = self.pending_new_comment.take() {
            // Strip comment header lines (starting with #) and trim
            let cleaned: String = new_text
                .lines()
                .filter(|l| !l.starts_with('#'))
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_string();

            if !cleaned.is_empty() {
                let comment = crate::review::ReviewComment {
                    commit_sha: self.pr.commits[commit_idx].sha.clone(),
                    file: file_path,
                    line,
                    side,
                    severity: crate::review::Severity::Suggestion,
                    comment: cleaned,
                    status: CommentStatus::Accepted,
                    edited_comment: None,
                };
                self.reviews
                    .entry(commit_idx)
                    .or_insert_with(|| crate::review::Review {
                        summary: String::new(),
                        comments: vec![],
                    })
                    .comments
                    .push(comment);
            }
            return;
        }

        // Editing an existing review comment
        if let Some((ci, cmt)) = self.viewing_comment {
            let cleaned: String = new_text
                .lines()
                .filter(|l| !l.starts_with('#'))
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_string();
            if let Some(review) = self.reviews.get_mut(&ci) {
                if let Some(comment) = review.comments.get_mut(cmt) {
                    comment.edited_comment = Some(cleaned);
                }
            }
            return;
        }

        // Otherwise, update the active dialog text
        if let Some((_, dialog)) = &mut self.active_dialog {
            dialog.set_text(new_text);
        }
    }

    /// Called when $EDITOR fails to open or exits with error.
    pub fn on_editor_failed(&mut self, error: String) {
        self.pending_new_comment = None;
        self.submit_result = Some(Err(error));
    }

    fn on_dialog_accept(
        &mut self,
        ctx: DialogContext,
        text: String,
        event_tx: &mpsc::Sender<AppEvent>,
    ) {
        match ctx {
            DialogContext::ReviewPrompt(review_idx) => {
                self.review_in_progress = Some(review_idx);
                self.focus = Panel::Diff;
                if self.diff_state.selected().is_none() && self.diff_line_count > 0 {
                    self.diff_state.select(Some(0));
                }
                let tx = event_tx.clone();
                let log = self.log_file.clone();
                let extra_args = self.settings.claude_args();
                let pid_holder = self.review_pid.clone();
                pid_holder.store(0, Ordering::Relaxed);
                thread::spawn(move || {
                    let result = claude::run_review(&text, log.as_deref(), &extra_args, &pid_holder);
                    pid_holder.store(0, Ordering::Relaxed);
                    let _ = tx.send(AppEvent::ReviewComplete(
                        review_idx,
                        result.map_err(|e| e.to_string()),
                    ));
                });
            }
            DialogContext::SubmitReview(api_comments, event_type) => {
                self.do_submit(text, api_comments, event_type, event_tx);
            }
        }
    }

    fn toggle_focus(&mut self) {
        match self.focus {
            Panel::Commits => {
                self.focus = Panel::Diff;
                if self.diff_state.selected().is_none() && self.diff_line_count > 0 {
                    self.diff_state.select(Some(0));
                }
            }
            Panel::Diff => {
                self.focus = Panel::Commits;
            }
        }
    }

    // -- Commits panel --

    fn on_key_commits(&mut self, key: KeyEvent) {
        let list_len = if self.commit_list_entries.is_empty() {
            self.pr.commits.len()
        } else {
            self.commit_list_entries.len()
        };

        let old_commit = self.selected_commit_index();

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(i) = self.commit_state.selected() {
                    if i + 1 < list_len {
                        self.commit_state.select(Some(i + 1));
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(i) = self.commit_state.selected() {
                    if i > 0 {
                        self.commit_state.select(Some(i - 1));
                    }
                }
            }
            KeyCode::Char('g') => {
                self.commit_state.select(Some(0));
            }
            KeyCode::Char('G') => {
                if list_len > 0 {
                    self.commit_state.select(Some(list_len - 1));
                }
            }
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
                self.focus = Panel::Diff;
                // If on a comment row, jump to that comment in the diff
                if let Some(CommitListEntry::Comment(ci, cmt_idx)) = self.selected_commit_list_entry() {
                    let ci = *ci;
                    let cmt_idx = *cmt_idx;
                    self.jump_to_comment_in_diff(ci, cmt_idx);
                } else if self.diff_state.selected().is_none() && self.diff_line_count > 0 {
                    self.diff_state.select(Some(0));
                }
            }
            _ => {}
        }

        // Reset diff cursor only when the selected commit actually changes
        let new_commit = self.selected_commit_index();
        if old_commit != new_commit {
            self.reset_diff_cursor();
        }
    }

    fn jump_to_comment_in_diff(&mut self, commit_idx: usize, comment_idx: usize) {
        let Some(review) = self.reviews.get(&commit_idx) else { return };
        let Some(comment) = review.comments.get(comment_idx) else { return };

        // Find the diff line that matches this comment
        for (i, info) in self.diff_line_info.iter().enumerate() {
            if info.file_path != comment.file || !info.is_code_line {
                continue;
            }
            let matches = match comment.side {
                crate::review::CommentSide::New => Some(comment.line) == info.new_lineno,
                crate::review::CommentSide::Old => Some(comment.line) == info.old_lineno,
                crate::review::CommentSide::File => info.lineno == 0,
            };
            if matches {
                self.diff_state.select(Some(i));
                return;
            }
        }

        // Fallback: just go to start
        if self.diff_state.selected().is_none() && self.diff_line_count > 0 {
            self.diff_state.select(Some(0));
        }
    }

    /// Rebuild the precomputed new-line-numbers-per-file map from diff_line_info.
    pub fn rebuild_diff_line_index(&mut self) {
        self.diff_new_lines_by_file.clear();
        for info in &self.diff_line_info {
            if let Some(nl) = info.new_lineno {
                self.diff_new_lines_by_file
                    .entry(info.file_path.clone())
                    .or_default()
                    .insert(nl);
            }
        }
    }

    fn reset_diff_cursor(&mut self) {
        self.diff_state = ListState::default();
        self.diff_line_count = 0;
    }

    // -- Settings --

    fn on_key_settings(&mut self, key: KeyEvent) -> AppAction {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                let i = self.settings_row.index();
                if i + 1 < SettingsRow::ALL.len() {
                    self.settings_row = SettingsRow::from_index(i + 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let i = self.settings_row.index();
                if i > 0 {
                    self.settings_row = SettingsRow::from_index(i - 1);
                }
            }
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Tab | KeyCode::Enter => {
                match self.settings_row {
                    SettingsRow::Theme => {
                        self.toggle_theme();
                    }
                    SettingsRow::Model => self.settings.model = self.settings.model.next(),
                    SettingsRow::Effort => self.settings.effort = self.settings.effort.next(),
                    SettingsRow::FastMode => self.settings.fast_mode = !self.settings.fast_mode,
                    SettingsRow::ViewPrompt => {
                        self.show_system_prompt = true;
                    }
                    SettingsRow::Version => {}
                }
            }
            KeyCode::Char('h') | KeyCode::Left => {
                match self.settings_row {
                    SettingsRow::Theme => self.toggle_theme(),
                    SettingsRow::Model => self.settings.model = self.settings.model.prev(),
                    SettingsRow::Effort => self.settings.effort = self.settings.effort.prev(),
                    SettingsRow::FastMode => self.settings.fast_mode = !self.settings.fast_mode,
                    _ => {}
                }
            }
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('s') => {
                self.show_settings = false;
            }
            _ => {}
        }
        AppAction::None
    }

    fn toggle_theme(&mut self) {
        self.theme = if self.theme.is_terminal() {
            Theme::solarized_dark()
        } else {
            Theme::terminal()
        };
        self.highlighter = Highlighter::new(&self.theme.syntect_theme);
    }

    // -- Prompt picker --

    fn on_key_prompt_picker(&mut self, key: KeyEvent) -> AppAction {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(picker) = &mut self.prompt_picker {
                    if picker.selected + 1 < picker.entries.len() {
                        picker.selected += 1;
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(picker) = &mut self.prompt_picker {
                    if picker.selected > 0 {
                        picker.selected -= 1;
                    }
                }
            }
            KeyCode::Enter => {
                if let Some(picker) = self.prompt_picker.take() {
                    let entry = &picker.entries[picker.selected];
                    if let Some((DialogContext::ReviewPrompt(_), dialog)) = &mut self.active_dialog {
                        let base = claude::build_pr_review_prompt(&self.pr);
                        if entry.content.is_empty() {
                            // (none) selected — reset to base prompt only
                            self.active_prompt = None;
                            dialog.set_text(base);
                            dialog.set_title("Review Prompt (Full PR)");
                        } else {
                            self.active_prompt = Some(entry.name.clone());
                            dialog.set_text(format!("{}\n\n---\n\n{}", base, entry.content));
                            dialog.set_title(&format!("Review Prompt [{}]", entry.name));
                        }
                    }
                }
            }
            KeyCode::Esc | KeyCode::Char('/') => {
                self.prompt_picker = None;
            }
            _ => {}
        }
        AppAction::None
    }

    // -- GitHub submission --

    fn submit_to_github(&mut self, _event_tx: &mpsc::Sender<AppEvent>) {
        // Check: if Claude reviews exist, all comments must be addressed
        let has_pending = self.reviews.values().any(|r| {
            r.comments.iter().any(|c| c.status == CommentStatus::Pending)
        });

        if has_pending {
            self.submit_result = Some(Err(
                "Cannot submit: some comments are still pending. Accept or reject all comments first.".to_string()
            ));
            return;
        }

        // Check there's something to submit
        let has_any_accepted = self.reviews.values().any(|r| {
            r.comments.iter().any(|c| c.status == CommentStatus::Accepted)
        });
        let has_summary = self.reviews.values().any(|r| !r.summary.is_empty());

        if !has_any_accepted && !has_summary {
            self.submit_result = Some(Err(
                "Nothing to submit. Add comments with Enter or run a Claude review with 'r'.".to_string()
            ));
            return;
        }

        // Collect accepted comments
        let mut api_comments = Vec::new();

        for review in self.reviews.values() {
            for comment in &review.comments {
                if comment.status != CommentStatus::Accepted {
                    continue;
                }

                let body = comment.display_comment().to_string();

                if comment.line == 0 || comment.side == crate::review::CommentSide::File {
                    api_comments.push(crate::github::client::ReviewApiComment {
                        commit_id: comment.commit_sha.clone(),
                        path: comment.file.clone(),
                        line: 0,
                        side: String::new(),
                        body,
                        subject_type: "file".to_string(),
                    });
                } else {
                    let side = match comment.side {
                        crate::review::CommentSide::New => "RIGHT",
                        crate::review::CommentSide::Old => "LEFT",
                        crate::review::CommentSide::File => "RIGHT",
                    };
                    api_comments.push(crate::github::client::ReviewApiComment {
                        commit_id: comment.commit_sha.clone(),
                        path: comment.file.clone(),
                        line: comment.line,
                        side: side.to_string(),
                        body,
                        subject_type: "line".to_string(),
                    });
                }
            }
        }

        // Build initial summary from Claude's review
        let summary = self.reviews.values()
            .find(|r| !r.summary.is_empty())
            .map(|r| r.summary.clone())
            .unwrap_or_default();

        // Open dialog for user to review/edit the summary before submitting
        let event_type = ReviewEventType::Comment;
        let dialog = ActionDialog::new(
            &summary,
            &format!("Submit Review [{}] (t: cycle type)", event_type),
            self.theme.status_accent,
        );
        self.active_dialog = Some((
            DialogContext::SubmitReview(api_comments, event_type),
            dialog,
        ));
    }

    fn do_submit(&mut self, body: String, api_comments: Vec<crate::github::client::ReviewApiComment>, event_type: ReviewEventType, event_tx: &mpsc::Sender<AppEvent>) {
        self.submit_in_progress = true;
        let tx = event_tx.clone();
        let owner = self.owner.clone();
        let repo_name = self.repo_name.clone();
        let token = self.github_token.clone();
        let pr_number = self.pr.number;
        let event_label = event_type.label().to_string();
        let log_file = self.log_file.clone();

        thread::spawn(move || {
            let result = tokio::runtime::Runtime::new()
                .map_err(|e| e.to_string())
                .and_then(|rt| {
                    rt.block_on(async {
                        let client = crate::github::client::GithubClient::new(&token)?;
                        client.submit_review(&owner, &repo_name, pr_number, &body, &event_label, api_comments, log_file.as_deref()).await
                    })
                    .map_err(|e| e.to_string())
                });
            let _ = tx.send(AppEvent::SubmitComplete(result));
        });
    }

    pub fn on_submit_complete(&mut self, result: Result<(), String>) {
        self.submit_in_progress = false;
        self.submit_result = Some(result);
    }

    fn batch_set_all_comments(&mut self, status: CommentStatus) {
        let Some(commit_idx) = self.selected_commit_index() else { return };
        let Some(review) = self.reviews.get_mut(&commit_idx) else { return };
        for comment in &mut review.comments {
            if comment.status == CommentStatus::Pending {
                comment.status = status.clone();
            }
        }
    }

    // -- Claude review dialogs --

    fn open_pr_review_dialog(&mut self) {
        if self.review_in_progress.is_some() {
            return;
        }

        let base_prompt = claude::build_pr_review_prompt(&self.pr);

        // If an active prompt is set, append it
        let (prompt, title) = if let Some(ref name) = self.active_prompt {
            let entries = prompts::load_prompts(self.repo_path.as_deref());
            if let Some(entry) = entries.iter().find(|e| e.name == *name) {
                let combined = format!("{}\n\n---\n\n{}", base_prompt, entry.content);
                (combined, format!("Review Prompt [{}]", name))
            } else {
                (base_prompt, "Review Prompt (Full PR)".to_string())
            }
        } else {
            (base_prompt, "Review Prompt (Full PR)".to_string())
        };

        let dialog = ActionDialog::new(&prompt, &title, self.theme.status_accent)
            .with_hint("/", "prompts");
        // Use usize::MAX to indicate whole-PR review
        self.active_dialog = Some((DialogContext::ReviewPrompt(usize::MAX), dialog));
    }

    pub fn on_review_complete(&mut self, _review_idx: usize, result: Result<Review, String>) {
        self.review_in_progress = None;
        match result {
            Ok(review) => {
                // Distribute comments to commits by commit_sha
                let mut per_commit: HashMap<usize, Vec<crate::review::ReviewComment>> = HashMap::new();

                for comment in review.comments {
                    // Find the commit index matching the comment's SHA (exact or 7-char prefix)
                    let commit_idx = self.pr.commits.iter().position(|c| {
                        c.sha == comment.commit_sha
                            || c.short_sha == comment.commit_sha[..7.min(comment.commit_sha.len())]
                    });

                    if let Some(idx) = commit_idx {
                        per_commit.entry(idx).or_default().push(comment);
                    }
                }

                // Merge Claude's comments with existing manual comments
                for i in 0..self.pr.commits.len() {
                    let claude_comments = per_commit.remove(&i).unwrap_or_default();
                    if let Some(existing) = self.reviews.get_mut(&i) {
                        // Keep user-added comments (Accepted status), replace Claude ones
                        existing.comments.retain(|c| c.status == CommentStatus::Accepted);
                        existing.comments.extend(claude_comments);
                        existing.summary = review.summary.clone();
                    } else {
                        self.reviews.insert(i, Review {
                            summary: review.summary.clone(),
                            comments: claude_comments,
                        });
                    }
                    // Sort by file path, then line number
                    if let Some(r) = self.reviews.get_mut(&i) {
                        r.comments.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
                    }
                }
            }
            Err(e) => {
                // Mark all commits as reviewed with error
                for i in 0..self.pr.commits.len() {
                    self.reviews.insert(
                        i,
                        Review {
                            summary: format!("Review failed: {e}"),
                            comments: vec![],
                        },
                    );
                }
            }
        }
    }

    fn close_comment(&mut self) {
        self.viewing_comment = None;
        if let Some(pos) = self.pre_comment_cursor.take() {
            self.diff_state.select(Some(pos));
        }
    }

    fn create_user_comment(&mut self) -> AppAction {
        let Some(diff_idx) = self.diff_state.selected() else {
            return AppAction::None;
        };
        let Some(info) = self.diff_line_info.get(diff_idx) else {
            return AppAction::None;
        };
        let Some(commit_idx) = self.selected_commit_index() else {
            return AppAction::None;
        };

        // Skip non-code lines (blank separators between files)
        if !info.is_code_line {
            return AppAction::None;
        }

        // File header — create a file-level comment
        if info.lineno == 0 {
            self.pending_new_comment = Some((commit_idx, info.file_path.clone(), 0, crate::review::CommentSide::File));
            let header = format!(
                "# File comment for {}\n# Write your comment below, save and exit\n\n",
                info.file_path
            );
            return AppAction::OpenEditor(header);
        }

        // Determine side based on which line number is present
        let side = if info.new_lineno.is_some() && info.old_lineno.is_none() {
            crate::review::CommentSide::New
        } else if info.old_lineno.is_some() && info.new_lineno.is_none() {
            crate::review::CommentSide::Old
        } else {
            crate::review::CommentSide::New // context lines default to new side
        };

        let line = match side {
            crate::review::CommentSide::New => info.new_lineno.unwrap_or(0),
            crate::review::CommentSide::Old => info.old_lineno.unwrap_or(0),
            _ => 0,
        };

        self.pending_new_comment = Some((commit_idx, info.file_path.clone(), line, side));

        let context = self.get_code_context(diff_idx);
        let header = format!(
            "# Review comment for {}:{}\n# Write your comment below, save and exit\n#\n{}#\n\n",
            info.file_path, line, context
        );
        AppAction::OpenEditor(header)
    }

    /// Get the 10 lines of code context around a diff line, formatted as # comments.
    fn get_code_context(&self, diff_idx: usize) -> String {
        let start = diff_idx.saturating_sub(5);
        let end = (diff_idx + 6).min(self.diff_line_info.len());

        let mut context = String::new();
        for i in start..end {
            let Some(info) = self.diff_line_info.get(i) else { continue };
            if !info.is_code_line { continue; }

            // Find the content from the commit's diff
            let content = self.get_diff_line_content(info);
            let marker = if i == diff_idx { ">>>" } else { "   " };
            let lineno = info.new_lineno.or(info.old_lineno).unwrap_or(0);
            context.push_str(&format!("# {} {:>4} {}\n", marker, lineno, content));
        }
        context
    }

    /// Extract the text content of a diff line from the commit's diff data.
    fn get_diff_line_content(&self, info: &DiffLineMetadata) -> String {
        let Some(commit_idx) = self.selected_commit_index() else {
            return String::new();
        };
        let Some(commit) = self.pr.commits.get(commit_idx) else {
            return String::new();
        };
        let Some(diff) = &commit.diff else {
            return String::new();
        };

        for file in &diff.files {
            if file.path != info.file_path { continue; }
            for hunk in &file.hunks {
                for line in &hunk.lines {
                    let matches = match info.new_lineno {
                        Some(nl) => line.new_lineno == Some(nl),
                        None => match info.old_lineno {
                            Some(ol) => line.old_lineno == Some(ol),
                            None => false,
                        },
                    };
                    if matches {
                        let prefix = match line.kind {
                            crate::github::models::DiffLineKind::Addition => "+ ",
                            crate::github::models::DiffLineKind::Deletion => "- ",
                            crate::github::models::DiffLineKind::Context => "  ",
                        };
                        return format!("{}{}", prefix, line.content);
                    }
                }
            }
        }
        String::new()
    }

    // -- Diff panel --

    fn on_key_diff(&mut self, key: KeyEvent) -> AppAction {
        // State 1: A comment is open — only a/e/x/Esc work
        if let Some((ci, cmt)) = self.viewing_comment {
            match key.code {
                KeyCode::Char('a') => {
                    if let Some(review) = self.reviews.get_mut(&ci) {
                        if let Some(comment) = review.comments.get_mut(cmt) {
                            comment.status = CommentStatus::Accepted;
                        }
                    }
                    self.close_comment();
                }
                KeyCode::Char('x') => {
                    if let Some(review) = self.reviews.get_mut(&ci) {
                        if let Some(comment) = review.comments.get_mut(cmt) {
                            comment.status = CommentStatus::Rejected;
                        }
                    }
                    self.close_comment();
                }
                KeyCode::Char('e') | KeyCode::Enter => {
                    let text = self.reviews.get(&ci)
                        .and_then(|r| r.comments.get(cmt))
                        .map(|c| c.display_comment().to_string())
                        .unwrap_or_default();
                    let diff_idx = self.pre_comment_cursor.unwrap_or(0);
                    let context = self.get_code_context(diff_idx);
                    let header = format!("# Editing review comment\n#\n{}#\n\n", context);
                    return AppAction::OpenEditor(format!("{}{}", header, text));
                }
                KeyCode::Esc => {
                    self.close_comment();
                }
                _ => {}
            }
            return AppAction::None;
        }

        // State 2: Normal diff navigation
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(i) = self.diff_state.selected() {
                    if i + 1 < self.diff_line_count {
                        self.diff_state.select(Some(i + 1));
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(i) = self.diff_state.selected() {
                    if i > 0 {
                        self.diff_state.select(Some(i - 1));
                    }
                }
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(i) = self.diff_state.selected() {
                    let target = (i + 20).min(self.diff_line_count.saturating_sub(1));
                    self.diff_state.select(Some(target));
                }
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(i) = self.diff_state.selected() {
                    self.diff_state.select(Some(i.saturating_sub(20)));
                }
            }
            KeyCode::Char('g') => {
                self.diff_state.select(Some(0));
            }
            KeyCode::Char('G') => {
                if self.diff_line_count > 0 {
                    self.diff_state.select(Some(self.diff_line_count - 1));
                }
            }
            KeyCode::Char('n') => {
                self.jump_to_next_comment();
            }
            KeyCode::Char('N') => {
                self.jump_to_prev_comment();
            }
            KeyCode::Char('h') | KeyCode::Left | KeyCode::Esc => {
                self.focus = Panel::Commits;
            }
            KeyCode::Enter => {
                if let Some(commit_idx) = self.selected_commit_index() {
                    if let Some(comment_idx) = self.find_comment_for_current_line(commit_idx) {
                        // Existing comment — open it
                        self.pre_comment_cursor = self.diff_state.selected();
                        self.viewing_comment = Some((commit_idx, comment_idx));
                    } else {
                        // Open editor for a new user comment (works with or without Claude review)
                        return self.create_user_comment();
                    }
                }
            }
            _ => {}
        }
        AppAction::None
    }

    fn jump_to_next_comment(&mut self) {
        let Some(commit_idx) = self.selected_commit_index() else { return };
        let Some(current) = self.diff_state.selected() else { return };

        // Scan forward from current+1 for a line with a comment
        for i in (current + 1)..self.diff_line_count {
            if self.has_comment_at_line(commit_idx, i) {
                self.diff_state.select(Some(i));
                return;
            }
        }
        // Wrap around from the start
        for i in 0..current {
            if self.has_comment_at_line(commit_idx, i) {
                self.diff_state.select(Some(i));
                return;
            }
        }
    }

    fn jump_to_prev_comment(&mut self) {
        let Some(commit_idx) = self.selected_commit_index() else { return };
        let Some(current) = self.diff_state.selected() else { return };

        // Scan backward from current-1
        for i in (0..current).rev() {
            if self.has_comment_at_line(commit_idx, i) {
                self.diff_state.select(Some(i));
                return;
            }
        }
        // Wrap around from the end
        for i in (current + 1..self.diff_line_count).rev() {
            if self.has_comment_at_line(commit_idx, i) {
                self.diff_state.select(Some(i));
                return;
            }
        }
    }

    fn has_comment_at_line(&self, commit_idx: usize, line_idx: usize) -> bool {
        let Some(review) = self.reviews.get(&commit_idx) else { return false };
        let Some(info) = self.diff_line_info.get(line_idx) else { return false };

        // Check file header comments
        if info.lineno == 0 && info.is_code_line {
            let empty = std::collections::HashSet::new();
            let file_lines = self.diff_new_lines_by_file.get(&info.file_path).unwrap_or(&empty);
            return review.comments.iter().any(|c| {
                c.file == info.file_path && (c.line == 0 || !file_lines.contains(&c.line))
            });
        }

        // Check code line comments
        review.comments.iter().any(|c| {
            if c.file != info.file_path || c.line == 0 {
                return false;
            }
            match c.side {
                crate::review::CommentSide::New => Some(c.line) == info.new_lineno,
                crate::review::CommentSide::Old => Some(c.line) == info.old_lineno,
                crate::review::CommentSide::File => false,
            }
        })
    }

    pub fn find_comment_for_current_line(&self, commit_idx: usize) -> Option<usize> {
        let review = self.reviews.get(&commit_idx)?;
        let diff_idx = self.diff_state.selected()?;
        let info = self.diff_line_info.get(diff_idx)?;

        // On a file header (lineno == 0): match file-level comments (line 0)
        // or any comment whose line doesn't appear in any diff line for this file
        if info.lineno == 0 {
            let empty = std::collections::HashSet::new();
            let file_lines = self.diff_new_lines_by_file.get(&info.file_path).unwrap_or(&empty);

            return review.comments.iter().position(|c| {
                c.file == info.file_path && (c.line == 0 || !file_lines.contains(&c.line))
            });
        }

        // On a code line: match based on comment side
        review.comments.iter().position(|c| {
            if c.file != info.file_path || c.line == 0 {
                return false;
            }
            match c.side {
                crate::review::CommentSide::New => Some(c.line) == info.new_lineno,
                crate::review::CommentSide::Old => Some(c.line) == info.old_lineno,
                crate::review::CommentSide::File => false,
            }
        })
    }
}
