use thiserror::Error;

#[derive(Debug, Error)]
pub enum NotesError {
    #[error("missing frontmatter")]
    MissingFrontmatter,
    #[error("invalid frontmatter: {0}")]
    Frontmatter(#[from] serde_yaml::Error),
    #[error("invalid version: {0}")]
    InvalidVersion(String),
    #[error("channel {channel} does not match version {version}")]
    ChannelMismatch { channel: &'static str, version: String },
    #[error("invalid date: {0}")]
    InvalidDate(String),
    #[error("empty breaking change entry")]
    EmptyBreakingEntry,
    #[error("line {line}: {message}")]
    Syntax { line: usize, message: String },
    #[error("release notes contain no entries")]
    Empty,
}

impl NotesError {
    pub fn syntax(line: usize, message: &str) -> Self {
        Self::Syntax {
            line,
            message: message.to_string(),
        }
    }
}
