mod app;
mod auth;
mod claude;
mod cli;
mod event;
mod git_diff;
mod github;
mod config;
mod prompts;
mod repo;
mod review;
mod settings;
mod theme;
mod ui;

use std::io::Write;
use std::process::Command;
use std::time::Duration;

use clap::Parser;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::KeyEventKind;

use app::{App, AppAction};
use event::{AppEvent, EventHandler};
use github::client::GithubClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    preflight_checks()?;
    let pr_number = cli.parse_pr_number()?;

    let repo_path = cli.repo_path.clone();
    let repo_info = repo::detect_repo(repo_path.as_deref())?;
    let effective_repo_path = repo_path
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    // Load config: defaults → global → local → CLI
    let cfg = config::ConfigFile::load_merged(Some(&effective_repo_path));

    let theme_name = cli.theme.or(cfg.theme).unwrap_or_else(|| "ocean-dark".to_string());
    let theme = theme::Theme::by_name(&theme_name).ok_or_else(|| {
        anyhow::anyhow!(
            "Unknown theme '{}'. Available: {}",
            theme_name,
            theme::Theme::available_themes().join(", ")
        )
    })?;

    let initial_settings = settings::Settings {
        model: cli.model.as_deref()
            .or(cfg.model.as_deref())
            .map(settings::ModelChoice::from_config)
            .unwrap_or(settings::ModelChoice::Default),
        effort: cli.effort.as_deref()
            .or(cfg.effort.as_deref())
            .map(settings::EffortLevel::from_config)
            .unwrap_or(settings::EffortLevel::Default),
        fast_mode: false,
    };

    let default_prompt = cfg.default_prompt;
    let commit_panel_width = cfg.commit_panel_width.unwrap_or(30).clamp(10, 80);
    let token = auth::get_token()?;

    let client = GithubClient::new(&token)?;

    eprintln!("Fetching PR #{} from {}...", pr_number, repo_info);

    let mut pr = client
        .fetch_pr(&repo_info.owner, &repo_info.repo, pr_number)
        .await?;

    // Fetch commits locally so git2 can generate diffs
    eprintln!("Fetching commits locally...");
    let mut fetch_failures = 0usize;
    for commit in &pr.commits {
        let status = Command::new("git")
            .args(["fetch", "origin", &commit.sha])
            .current_dir(&effective_repo_path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        match status {
            Ok(s) if !s.success() => {
                eprintln!("Warning: git fetch failed for {} (exit {})", commit.short_sha, s);
                fetch_failures += 1;
            }
            Err(e) => {
                eprintln!("Warning: failed to run git fetch for {}: {}", commit.short_sha, e);
                fetch_failures += 1;
            }
            _ => {}
        }
    }
    if fetch_failures == pr.commits.len() {
        anyhow::bail!(
            "Failed to fetch any commits. Check your network connection and that \
             'origin' points to the correct remote."
        );
    }

    // Open the git repo for local diff generation
    let git_repo = git2::Repository::open(&effective_repo_path)?;

    eprintln!("Generating diffs for {} commits...", pr.commits.len());
    for commit in &mut pr.commits {
        match git_diff::generate_commit_diff(&git_repo, &commit.sha) {
            Ok((diff, stats)) => {
                commit.diff = Some(diff);
                commit.stats = stats;
            }
            Err(e) => {
                eprintln!("Warning: failed to generate diff for {}: {}", commit.short_sha, e);
            }
        }
    }

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = ratatui::restore();
        original_hook(panic_info);
    }));

    let mut terminal = ratatui::init();
    let result = run_tui(&mut terminal, App::new(
        pr, theme, cli.log_file,
        repo_info.owner.clone(), repo_info.repo.clone(), token,
        Some(effective_repo_path),
        initial_settings,
        default_prompt,
        commit_panel_width,
    ));
    ratatui::restore();

    result
}

fn preflight_checks() -> anyhow::Result<()> {
    let mut errors: Vec<String> = Vec::new();

    // Check git is available
    if Command::new("git").arg("--version").output().is_err() {
        errors.push("'git' is not installed or not in PATH".to_string());
    }

    // Check gh CLI is available
    match Command::new("gh").arg("--version").output() {
        Err(_) => errors.push("'gh' (GitHub CLI) is not installed or not in PATH".to_string()),
        Ok(output) if !output.status.success() => {
            errors.push("'gh' (GitHub CLI) is not working properly".to_string());
        }
        Ok(_) => {
            // Check gh is authenticated
            let auth = Command::new("gh").args(["auth", "status"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            match auth {
                Ok(s) if !s.success() => {
                    errors.push("'gh' is not authenticated. Run: gh auth login".to_string());
                }
                Err(_) => {
                    errors.push("Failed to check 'gh' auth status".to_string());
                }
                _ => {}
            }
        }
    }

    // Check claude CLI is available
    match Command::new("claude").arg("--version").output() {
        Err(_) => errors.push(
            "'claude' (Claude Code CLI) is not installed or not in PATH.\n  \
             Install: https://docs.anthropic.com/en/docs/claude-code".to_string()
        ),
        Ok(output) if !output.status.success() => {
            errors.push("'claude' CLI is not working properly".to_string());
        }
        Ok(_) => {}
    }

    // Check $EDITOR is set or vi/vim exists
    let editor = std::env::var("EDITOR").unwrap_or_default();
    if editor.is_empty() {
        // Check for vi as fallback
        if Command::new("which").arg("vi")
            .stdout(std::process::Stdio::null())
            .status()
            .map(|s| !s.success())
            .unwrap_or(true)
        {
            errors.push(
                "$EDITOR is not set and 'vi' is not available.\n  \
                 Set your preferred editor: export EDITOR=nano".to_string()
            );
        }
    }

    if errors.is_empty() {
        return Ok(());
    }

    let mut msg = String::from("Preflight checks failed:\n");
    for (i, err) in errors.iter().enumerate() {
        msg.push_str(&format!("\n  {}. {}", i + 1, err));
    }
    anyhow::bail!(msg)
}

fn run_tui(terminal: &mut DefaultTerminal, mut app: App) -> anyhow::Result<()> {
    let mut events = EventHandler::new(Duration::from_millis(250));
    let event_tx = events.sender();

    loop {
        terminal.draw(|frame| ui::render::render(frame, &mut app))?;

        match events.next()? {
            AppEvent::Key(key) => {
                if key.kind == KeyEventKind::Press {
                    let action = app.on_key(key, &event_tx);
                    match action {
                        AppAction::OpenEditor(text) => {
                            events.pause();
                            match open_in_editor(terminal, &text) {
                                Ok(new_text) => app.on_editor_done(new_text),
                                Err(e) => app.on_editor_failed(e.to_string()),
                            }
                            events.resume();
                        }
                        AppAction::None => {}
                    }
                }
            }
            AppEvent::ReviewComplete(commit_idx, result) => {
                app.on_review_complete(commit_idx, result);
            }
            AppEvent::SubmitComplete(result) => {
                app.on_submit_complete(result);
            }
            AppEvent::Tick => app.tick(),
            AppEvent::Resize => {}
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

fn open_in_editor(terminal: &mut DefaultTerminal, text: &str) -> anyhow::Result<String> {
    let mut temp_file = tempfile::Builder::new()
        .prefix("claudicant-")
        .suffix(".md")
        .tempfile()?;
    temp_file.write_all(text.as_bytes())?;
    let temp_path = temp_file.path().to_path_buf();

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

    ratatui::restore();

    let status = Command::new(&editor)
        .arg(&temp_path)
        .status();

    *terminal = ratatui::init();

    match status {
        Ok(s) if s.success() => {
            let new_text = std::fs::read_to_string(&temp_path)
                .unwrap_or_else(|_| text.to_string());
            Ok(new_text)
        }
        Ok(s) => {
            anyhow::bail!("Editor '{}' exited with status {}", editor, s)
        }
        Err(e) => {
            anyhow::bail!("Failed to open editor '{}': {}", editor, e)
        }
    }
}
