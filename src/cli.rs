use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
#[command(name = "claudicant", about = "A TUI tool for curating GitHub PR reviews with Claude AI")]
pub struct Cli {
    /// PR number (e.g., 123 or #123)
    pub pr_number: String,

    /// Path to the git repository (defaults to current directory)
    #[arg(short = 'r', long = "repo")]
    pub repo_path: Option<PathBuf>,

    /// Color theme [ocean-dark, solarized-dark, light]
    #[arg(long)]
    pub theme: Option<String>,

    /// Claude model [opus, sonnet, haiku]
    #[arg(long)]
    pub model: Option<String>,

    /// Effort level [low, medium, high, max]
    #[arg(long)]
    pub effort: Option<String>,

    /// Log file for debugging Claude responses
    #[arg(long)]
    pub log_file: Option<PathBuf>,
}

impl Cli {
    pub fn parse_pr_number(&self) -> anyhow::Result<u64> {
        let cleaned = self.pr_number.trim_start_matches('#');
        cleaned
            .parse::<u64>()
            .map_err(|_| anyhow::anyhow!("Invalid PR number: '{}'. Expected a number like 123 or #123", self.pr_number))
    }
}
