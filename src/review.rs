use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Review {
    pub summary: String,
    pub comments: Vec<ReviewComment>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReviewComment {
    pub commit_sha: String,
    pub file: String,
    pub line: u32,
    pub side: CommentSide,
    pub severity: Severity,
    pub comment: String,
    #[serde(skip)]
    pub status: CommentStatus,
    #[serde(skip)]
    pub edited_comment: Option<String>,
}

impl ReviewComment {
    pub fn display_comment(&self) -> &str {
        self.edited_comment.as_deref().unwrap_or(&self.comment)
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CommentSide {
    New,
    Old,
    File,
}

impl Default for CommentSide {
    fn default() -> Self {
        Self::New
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    Warning,
    Suggestion,
    Nitpick,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Critical => write!(f, "critical"),
            Severity::Warning => write!(f, "warning"),
            Severity::Suggestion => write!(f, "suggestion"),
            Severity::Nitpick => write!(f, "nitpick"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum CommentStatus {
    #[default]
    Pending,
    Accepted,
    Rejected,
}

impl std::fmt::Display for CommentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommentStatus::Pending => write!(f, "pending"),
            CommentStatus::Accepted => write!(f, "accepted"),
            CommentStatus::Rejected => write!(f, "rejected"),
        }
    }
}
