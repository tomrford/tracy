use thiserror::Error;

use crate::filter::FilterError;
use crate::git::GitError;
use crate::scan::ScanError;

#[derive(Debug, Error)]
pub enum TracyError {
    #[error(transparent)]
    Filter(#[from] FilterError),

    #[error(transparent)]
    Scan(#[from] ScanError),

    #[error(transparent)]
    Git(#[from] GitError),

    #[error("failed to serialize output: {0}")]
    Serialize(#[from] serde_json::Error),

    #[error("failed to write output file: {0}")]
    WriteOutput(#[from] std::io::Error),

    #[error("no matches found")]
    NoResults,
}
