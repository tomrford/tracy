use thiserror::Error;

#[derive(Debug, Error)]
pub enum FilterError {
    #[error("failed to walk directory: {0}")]
    Walk(#[from] ignore::Error),
}
