use anyhow::{Context, Result};
use git2::{DiffOptions, Oid, Patch, Repository};

use crate::github::models::*;

pub fn generate_commit_diff(repo: &Repository, sha: &str) -> Result<(Diff, CommitStats)> {
    let oid = Oid::from_str(sha)
        .with_context(|| format!("Invalid commit SHA: {sha}"))?;
    let commit = repo.find_commit(oid)
        .with_context(|| format!("Commit not found locally: {sha}. Try running: git fetch origin"))?;

    let commit_tree = commit.tree()
        .context("Failed to get commit tree")?;

    let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

    let mut opts = DiffOptions::new();
    opts.context_lines(u32::MAX);

    let git_diff = repo.diff_tree_to_tree(
        parent_tree.as_ref(),
        Some(&commit_tree),
        Some(&mut opts),
    ).context("Failed to generate diff")?;

    let mut files: Vec<FileDiff> = Vec::new();
    let mut total_additions: u64 = 0;
    let mut total_deletions: u64 = 0;

    let num_deltas = git_diff.deltas().len();

    for delta_idx in 0..num_deltas {
        let delta = git_diff.deltas().nth(delta_idx).unwrap();

        let path = delta.new_file().path()
            .or_else(|| delta.old_file().path())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let old_path = if delta.status() == git2::Delta::Renamed {
            delta.old_file().path().map(|p| p.to_string_lossy().to_string())
        } else {
            None
        };

        let status = match delta.status() {
            git2::Delta::Added => FileStatus::Added,
            git2::Delta::Deleted => FileStatus::Deleted,
            git2::Delta::Renamed => FileStatus::Renamed,
            _ => FileStatus::Modified,
        };

        let mut file_additions: u64 = 0;
        let mut file_deletions: u64 = 0;
        let mut hunks: Vec<Hunk> = Vec::new();

        // Use Patch API to iterate hunks and lines for this delta
        if let Ok(patch) = Patch::from_diff(&git_diff, delta_idx) {
            if let Some(patch) = patch {
                let num_hunks = patch.num_hunks();
                for hunk_idx in 0..num_hunks {
                    if let Ok((hunk, num_lines)) = patch.hunk(hunk_idx) {
                        let header = String::from_utf8_lossy(hunk.header()).trim_end().to_string();
                        let mut lines: Vec<DiffLine> = Vec::new();

                        for line_idx in 0..num_lines {
                            if let Ok(line) = patch.line_in_hunk(hunk_idx, line_idx) {
                                let content = String::from_utf8_lossy(line.content())
                                    .trim_end_matches('\n')
                                    .to_string();

                                let kind = match line.origin() {
                                    '+' => {
                                        file_additions += 1;
                                        total_additions += 1;
                                        DiffLineKind::Addition
                                    }
                                    '-' => {
                                        file_deletions += 1;
                                        total_deletions += 1;
                                        DiffLineKind::Deletion
                                    }
                                    _ => DiffLineKind::Context,
                                };

                                lines.push(DiffLine {
                                    kind,
                                    content,
                                    old_lineno: line.old_lineno(),
                                    new_lineno: line.new_lineno(),
                                });
                            }
                        }

                        hunks.push(Hunk { header, lines });
                    }
                }
            }
        }

        files.push(FileDiff {
            path,
            old_path,
            status,
            additions: file_additions,
            deletions: file_deletions,
            hunks,
        });
    }

    let stats = CommitStats {
        additions: total_additions,
        deletions: total_deletions,
        total: total_additions + total_deletions,
    };

    Ok((Diff { files }, stats))
}
