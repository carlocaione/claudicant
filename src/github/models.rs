#[derive(Clone)]
#[allow(dead_code)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub author: String,
    pub description: String,
    pub base_branch: String,
    pub head_branch: String,
    pub commits: Vec<Commit>,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct Commit {
    pub sha: String,
    pub short_sha: String,
    pub author: String,
    pub author_email: String,
    pub date: String,
    pub committer: String,
    pub committer_email: String,
    pub committer_date: String,
    pub summary: String,
    pub body: String,
    pub html_url: String,
    pub stats: CommitStats,
    pub diff: Option<Diff>,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct CommitStats {
    pub additions: u64,
    pub deletions: u64,
    pub total: u64,
}

#[derive(Clone)]
pub struct Diff {
    pub files: Vec<FileDiff>,
}

#[derive(Clone)]
pub struct FileDiff {
    pub path: String,
    pub old_path: Option<String>,
    pub status: FileStatus,
    pub additions: u64,
    pub deletions: u64,
    pub hunks: Vec<Hunk>,
}

#[derive(Clone)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
}

impl std::fmt::Display for FileStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileStatus::Added => write!(f, "A"),
            FileStatus::Modified => write!(f, "M"),
            FileStatus::Deleted => write!(f, "D"),
            FileStatus::Renamed => write!(f, "R"),
        }
    }
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct Hunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Clone)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub content: String,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
}

#[derive(Clone, Copy)]
pub enum DiffLineKind {
    Context,
    Addition,
    Deletion,
}
