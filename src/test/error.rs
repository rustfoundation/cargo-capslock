use escargot::error::CargoError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Cargo(#[from] CargoError),

    #[error(transparent)]
    Dynamic(#[from] crate::dynamic::Error),

    #[error("creating temporary target directory: {0}")]
    TempDir(#[source] std::io::Error),

    #[error("getting current working directory: {0}")]
    WorkingDir(#[source] std::io::Error),
}
