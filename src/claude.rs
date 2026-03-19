use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{Context, Result};

/// Maximum time to wait for Claude to complete a review.
const REVIEW_TIMEOUT: Duration = Duration::from_secs(5 * 60);

use crate::github::models::PullRequest;
use crate::review::Review;

const REVIEW_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "summary": { "type": "string" },
    "comments": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "commit_sha": { "type": "string" },
          "file": { "type": "string" },
          "line": { "type": "integer" },
          "side": { "type": "string", "enum": ["new", "old", "file"] },
          "severity": { "type": "string", "enum": ["critical", "warning", "suggestion", "nitpick"] },
          "comment": { "type": "string" }
        },
        "required": ["commit_sha", "file", "line", "side", "severity", "comment"]
      }
    }
  },
  "required": ["summary", "comments"]
}"#;

pub const REVIEW_SYSTEM: &str = "\
You are reviewing a GitHub pull request. You have access to git and gh CLI tools.\n\
\n\
HOW TO EXPLORE THE PR:\n\
- Use `gh pr view <number>` to see PR description and metadata.\n\
- Use `gh pr diff <number>` to see the full PR diff.\n\
- Use `git show <sha>` to see individual commit diffs.\n\
- Use `git show <sha>:<filepath>` to read full files at a specific commit.\n\
- Use `git show <sha>:<filepath> | cat -n` to see line numbers.\n\
- Do NOT use the Read tool — the working directory may differ from the PR.\n\
\n\
WHAT TO REVIEW:\n\
- Review EACH commit independently using `git show <sha>` for each one.\n\
- Look for: bugs, security issues, performance problems, logic errors, code quality.\n\
- Pin each comment to the commit that INTRODUCED the issue, not a later commit that touches the same file.\n\
- If a bug was introduced in commit A and not fixed in commit B, comment on commit A.\n\
- Every comment MUST be pinned to a specific commit, file, and line.\n\
- Do NOT return general PR-level comments that cannot be pinned to a location.\n\
\n\
COMMENT FIELDS:\n\
- `commit_sha`: the full SHA of the commit this comment applies to.\n\
- `file`: the file path as shown in the diff.\n\
- `line`: the exact line number. Use `git show <sha>:<file> | cat -n` to verify.\n\
- `side`: which version of the file:\n\
  - `\"new\"` for added or modified lines (+ lines). `line` is the line number in the new file.\n\
  - `\"old\"` for deleted lines (- lines). `line` is the line number in the old file.\n\
  - `\"file\"` for whole-file comments (set `line` to 0).\n\
\n\
COMMENT STYLE:\n\
- Write comments as a human reviewer would — direct and conversational.\n\
- Do NOT start with a bold summary sentence like \"**Problem description.** ...\". Just explain the issue directly.\n\
- Use backticks for code references, but avoid excessive markdown formatting.\n\
\n\
If the PR looks good, say so in the summary and return an empty comments array.";

pub fn build_pr_review_prompt(pr: &PullRequest) -> String {
    let commit_list: String = pr
        .commits
        .iter()
        .map(|c| format!("- `{}` {}", c.short_sha, c.summary))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "Review PR #{number}: **{title}**\n\n\
         **Commits:**\n\n{commits}\n\n\
         Use `gh pr diff {number}` and `git show <sha>` to examine the changes.",
        number = pr.number,
        title = pr.title,
        commits = commit_list,
    )
}

pub fn run_review(
    prompt: &str,
    log_file: Option<&std::path::Path>,
    extra_args: &[String],
) -> Result<Review> {
    let mut cmd = Command::new("claude");
    cmd.args([
        "-p",
        "--output-format", "json",
        "--json-schema", REVIEW_SCHEMA,
        "--append-system-prompt", REVIEW_SYSTEM,
        "--allowedTools", "Bash(git *),Bash(gh *)",
    ]);
    cmd.args(extra_args);
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to run 'claude' CLI. Is Claude Code installed?")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(prompt.as_bytes())?;
    }

    // Wait with timeout — kill the process if it takes too long
    let child_id = child.id();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = child.wait_with_output();
        let _ = tx.send(result);
    });

    let output = match rx.recv_timeout(REVIEW_TIMEOUT) {
        Ok(result) => result.context("Failed to wait for claude process")?,
        Err(_) => {
            // Timeout — kill the process tree
            let _ = Command::new("kill").arg(child_id.to_string()).status();
            anyhow::bail!(
                "Claude review timed out after {} minutes",
                REVIEW_TIMEOUT.as_secs() / 60
            );
        }
    };

    let raw_stdout = String::from_utf8_lossy(&output.stdout);

    if let Some(path) = log_file {
        use std::fs::OpenOptions;
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "=== Claude Response ===");
            let _ = writeln!(f, "Prompt: {}", prompt);
            let _ = writeln!(f, "Stdout: {}", raw_stdout);
            let _ = writeln!(f, "Stderr: {}", String::from_utf8_lossy(&output.stderr));
            let _ = writeln!(f, "========================\n");
        }
    }

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Claude review failed: {}", stderr.trim());
    }

    let response: serde_json::Value = serde_json::from_str(&raw_stdout)
        .context("Failed to parse Claude response as JSON")?;

    let review_data = if let Some(structured) = response.get("structured_output") {
        structured.clone()
    } else if let Some(result) = response.get("result") {
        serde_json::from_str(result.as_str().unwrap_or("{}"))
            .unwrap_or_else(|_| result.clone())
    } else {
        response.clone()
    };

    if let Some(path) = log_file {
        use std::fs::OpenOptions;
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "=== Parsed Review Data ===");
            let _ = writeln!(f, "{}", serde_json::to_string_pretty(&review_data).unwrap_or_default());
            let _ = writeln!(f, "==========================\n");
        }
    }

    let review: Review = serde_json::from_value(review_data)
        .context("Failed to parse review from Claude response")?;

    Ok(review)
}
