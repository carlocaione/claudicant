use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use octocrab::Octocrab;

use super::models::*;

pub struct GithubClient {
    octocrab: Octocrab,
}

impl GithubClient {
    pub fn new(token: &str) -> Result<Self> {
        let octocrab = Octocrab::builder()
            .personal_token(token.to_string())
            .build()
            .context("Failed to create GitHub client")?;
        Ok(Self { octocrab })
    }

    pub async fn fetch_pr(&self, owner: &str, repo: &str, number: u64) -> Result<PullRequest> {
        let pr = self
            .octocrab
            .pulls(owner, repo)
            .get(number)
            .await
            .with_context(|| format!("Failed to fetch PR #{number}"))?;

        let commits = self.fetch_commits(owner, repo, number).await?;

        let author = pr
            .user
            .map(|u| u.login)
            .unwrap_or_else(|| "unknown".to_string());

        Ok(PullRequest {
            number: pr.number,
            title: pr.title.unwrap_or_default(),
            author,
            description: pr.body.unwrap_or_default(),
            base_branch: pr.base.label.unwrap_or_default(),
            head_branch: pr.head.label.unwrap_or_default(),
            commits,
        })
    }

    async fn fetch_commits(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<Commit>> {
        let commits = self
            .octocrab
            .pulls(owner, repo)
            .pr_commits(number)
            .send()
            .await
            .with_context(|| format!("Failed to fetch commits for PR #{number}"))?;

        Ok(commits
            .items
            .into_iter()
            .map(|c| {
                let sha = c.sha;
                let short_sha = sha[..7.min(sha.len())].to_string();

                let (author, author_email, date) = c
                    .commit
                    .author
                    .map(|a| (
                        a.name,
                        a.email.unwrap_or_default(),
                        a.date.map(|d| d.to_string()).unwrap_or_default(),
                    ))
                    .unwrap_or_else(|| ("unknown".to_string(), String::new(), String::new()));

                let (committer, committer_email, committer_date) = c
                    .commit
                    .committer
                    .map(|a| (
                        a.name,
                        a.email.unwrap_or_default(),
                        a.date.map(|d| d.to_string()).unwrap_or_default(),
                    ))
                    .unwrap_or_else(|| ("unknown".to_string(), String::new(), String::new()));

                let html_url = c.html_url.to_string();

                let message = &c.commit.message;
                let summary = message.lines().next().unwrap_or("").to_string();
                let body = message
                    .splitn(2, '\n')
                    .nth(1)
                    .unwrap_or("")
                    .trim()
                    .to_string();

                Commit {
                    sha,
                    short_sha,
                    author,
                    author_email,
                    date,
                    committer,
                    committer_email,
                    committer_date,
                    summary,
                    body,
                    html_url,
                    stats: CommitStats { additions: 0, deletions: 0, total: 0 },
                    diff: None,
                }
            })
            .collect())
    }

    /// Submit a review: first post the review body, then each inline comment
    /// individually pinned to its commit SHA.
    pub async fn submit_review(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        body: &str,
        event: &str,
        comments: Vec<ReviewApiComment>,
        log_file: Option<&Path>,
    ) -> Result<()> {
        // Step 1: Create the review with just the body (no inline comments)
        let review_route = format!("/repos/{owner}/{repo}/pulls/{pr_number}/reviews");
        let review_request = serde_json::json!({
            "body": body,
            "event": event,
        });

        log_api(log_file, "POST", &review_route, &review_request);

        let response: Result<serde_json::Value, _> = self
            .octocrab
            .post(&review_route, Some(&review_request))
            .await;

        match &response {
            Ok(resp) => log_api_response(log_file, &review_route, resp),
            Err(e) => log_api_error(log_file, &review_route, e),
        }

        response.map_err(|e| {
            if event != "COMMENT" {
                anyhow::anyhow!(
                    "Failed to submit as {event}. You cannot approve or request changes on your own PR. \
                     Use Comment instead."
                )
            } else {
                anyhow::anyhow!("Failed to submit review: {e}")
            }
        })?;

        // Step 2: Post each inline comment individually with its commit_id
        let comment_route = format!("/repos/{owner}/{repo}/pulls/{pr_number}/comments");
        let total = comments.len();
        let mut failed = 0usize;
        let mut first_error = String::new();

        for comment in &comments {
            let comment_request = if comment.subject_type == "file" {
                serde_json::json!({
                    "commit_id": comment.commit_id,
                    "path": comment.path,
                    "subject_type": "file",
                    "body": comment.body,
                })
            } else {
                serde_json::json!({
                    "commit_id": comment.commit_id,
                    "path": comment.path,
                    "line": comment.line,
                    "side": comment.side,
                    "body": comment.body,
                })
            };

            log_api(log_file, "POST", &comment_route, &comment_request);

            let result: Result<serde_json::Value, _> = self
                .octocrab
                .post(&comment_route, Some(&comment_request))
                .await;

            match &result {
                Ok(resp) => log_api_response(log_file, &comment_route, resp),
                Err(e) => {
                    log_api_error(log_file, &comment_route, e);
                    if first_error.is_empty() {
                        first_error = format!("{}:{} - {e}", comment.path, comment.line);
                    }
                    failed += 1;
                }
            }
        }

        if failed > 0 {
            anyhow::bail!("Review posted but {failed}/{total} inline comments failed. First: {first_error}")
        }

        Ok(())
    }
}

fn log_api(log_file: Option<&Path>, method: &str, route: &str, payload: &serde_json::Value) {
    let Some(path) = log_file else { return };
    let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) else { return };
    let _ = writeln!(f, "=== GitHub API Request ===");
    let _ = writeln!(f, "{method} {route}");
    let _ = writeln!(f, "{}", serde_json::to_string_pretty(payload).unwrap_or_default());
    let _ = writeln!(f, "==========================\n");
}

fn log_api_response(log_file: Option<&Path>, route: &str, response: &serde_json::Value) {
    let Some(path) = log_file else { return };
    let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) else { return };
    let _ = writeln!(f, "=== GitHub API Response (OK) ===");
    let _ = writeln!(f, "Route: {route}");
    let _ = writeln!(f, "{}", serde_json::to_string_pretty(response).unwrap_or_default());
    let _ = writeln!(f, "================================\n");
}

fn log_api_error(log_file: Option<&Path>, route: &str, error: &octocrab::Error) {
    let Some(path) = log_file else { return };
    let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) else { return };
    let _ = writeln!(f, "=== GitHub API Response (ERROR) ===");
    let _ = writeln!(f, "Route: {route}");
    let _ = writeln!(f, "Error: {error}");
    let _ = writeln!(f, "Debug: {error:?}");
    let _ = writeln!(f, "===================================\n");
}

pub struct ReviewApiComment {
    pub commit_id: String,
    pub path: String,
    pub line: u32,
    pub side: String,
    pub body: String,
    /// "line" for normal comments, "file" for file-level comments
    pub subject_type: String,
}
