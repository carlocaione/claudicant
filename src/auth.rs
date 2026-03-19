use anyhow::Result;
use std::process::Command;

pub fn get_token() -> Result<String> {
    // Try gh CLI first
    if let Ok(output) = Command::new("gh").args(["auth", "token"]).output() {
        if output.status.success() {
            let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !token.is_empty() {
                return Ok(token);
            }
        }
    }

    // Fall back to GITHUB_TOKEN env var
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        if !token.is_empty() {
            return Ok(token);
        }
    }

    anyhow::bail!(
        "No GitHub token found. Either:\n  \
         1. Install GitHub CLI and run: gh auth login\n  \
         2. Set the GITHUB_TOKEN environment variable"
    )
}
