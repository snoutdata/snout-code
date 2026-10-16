use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not run git: {0}")]
    Io(#[from] std::io::Error),

    #[error("`git {args}` failed: {message}")]
    Git { args: String, message: String },

    #[error("{} is not inside a Git working tree", .0.display())]
    NotARepo(PathBuf),

    #[error("unexpected git output: {0}")]
    Parse(String),

    #[error("{0}")]
    Invalid(String),
}
