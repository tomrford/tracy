use thiserror::Error;

use crate::filter::FilterError;

#[derive(Debug, Error)]
pub enum TracyError {
    #[error(transparent)]
    Filter(#[from] FilterError),
}
