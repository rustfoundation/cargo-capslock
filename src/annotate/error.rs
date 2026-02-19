use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Cache(#[from] osv_cache::Error),

    #[error("ecosystem-specific data in {id} affected #{index} is not RustSec-shaped")]
    EcosystemSpecificNotRust { id: String, index: usize },

    #[error("opening report from {path}: {e}")]
    ReportOpen {
        #[source]
        e: std::io::Error,
        path: String,
    },

    #[error("parsing report: {0}")]
    ReportParse(#[source] serde_json::Error),
}
