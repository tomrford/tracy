use thiserror::Error;

#[derive(Debug, Error)]
pub enum FilterError {
    #[error("failed to walk directory: {0}")]
    Walk(#[from] ignore::Error),

    #[error("invalid glob pattern {pattern}: {source}")]
    InvalidGlob {
        pattern: String,
        source: glob::PatternError,
    },
}
