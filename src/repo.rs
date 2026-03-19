use std::path::Path;

use anyhow::Result;
use regex::Regex;

pub struct RepoInfo {
    pub owner: String,
    pub repo: String,
}

impl std::fmt::Display for RepoInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.owner, self.repo)
    }
}

pub fn detect_repo(repo_path: Option<&Path>) -> Result<RepoInfo> {
    let repo = match repo_path {
        Some(path) => git2::Repository::open(path)
            .map_err(|_| anyhow::anyhow!("Not a git repository: {}", path.display()))?,
        None => git2::Repository::open_from_env()
            .map_err(|_| anyhow::anyhow!("Not inside a git repository. Run claudicant from within a cloned GitHub repo, or use -r <path>."))?,
    };

    // Try origin first, then any github.com remote
    if let Some(info) = try_remote(&repo, "origin") {
        return Ok(info);
    }

    let remotes = repo.remotes()
        .map_err(|_| anyhow::anyhow!("Failed to list remotes"))?;

    for name in remotes.iter().flatten() {
        if let Some(info) = try_remote(&repo, name) {
            return Ok(info);
        }
    }

    anyhow::bail!(
        "No GitHub remote found. Expected a remote URL like:\n  \
         git@github.com:owner/repo.git\n  \
         https://github.com/owner/repo.git"
    )
}

fn try_remote(repo: &git2::Repository, name: &str) -> Option<RepoInfo> {
    let remote = repo.find_remote(name).ok()?;
    let url = remote.url()?;
    extract_owner_repo(url)
}

fn extract_owner_repo(url: &str) -> Option<RepoInfo> {
    if !url.contains("github.com") {
        return None;
    }
    let re = Regex::new(r"github\.com[:/]([^/]+)/([^/\s]+?)(?:\.git)?$").ok()?;
    let caps = re.captures(url)?;
    Some(RepoInfo {
        owner: caps[1].to_string(),
        repo: caps[2].to_string(),
    })
}
