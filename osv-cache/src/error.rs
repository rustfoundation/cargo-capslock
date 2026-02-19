use thiserror::Error;
use zip::result::ZipError;

#[derive(Debug, Error)]
pub enum Error {
    #[error("caching advisory {id}: {e}")]
    AdvisoryCreate {
        #[source]
        e: std::io::Error,
        id: String,
    },

    #[error("parsing local advisory at {path}: {e}")]
    AdvisoryLocalParse {
        #[source]
        e: serde_json::Error,
        path: String,
    },

    #[error("opening local advisory at {path}: {e}")]
    AdvisoryOpen {
        #[source]
        e: std::io::Error,
        path: String,
    },

    #[error("parsing remote advisory {id}: {e}")]
    AdvisoryRemoteParse {
        #[source]
        e: reqwest::Error,
        id: String,
    },

    #[error("requesting advisory {id}: {e}")]
    AdvisoryRequest {
        #[source]
        e: reqwest::Error,
        id: String,
    },

    #[error("advisory {id} response: {e}")]
    AdvisoryResponse {
        #[source]
        e: reqwest::Error,
        id: String,
    },

    #[error("writing advisory {id} JSON: {e}")]
    AdvisoryWrite {
        #[source]
        e: serde_json::Error,
        id: String,
    },

    #[error("extracting all advisories: {0}")]
    AllExtract(#[source] ZipError),

    #[error("opening all advisory archive: {0}")]
    AllOpen(#[source] ZipError),

    #[error("requesting all advisories: {0}")]
    AllRequest(#[source] reqwest::Error),

    #[error("all advisories response: {0}")]
    AllResponse(#[source] reqwest::Error),

    #[error("creating temporary file: {0}")]
    AllTemp(#[source] std::io::Error),

    #[error("building reqwest client: {0}")]
    Client(#[source] reqwest::Error),

    #[error("creating OSV cache within {path}: {e}")]
    Create {
        #[source]
        e: std::io::Error,
        path: String,
    },

    #[error("getting OSV cache home (is $HOME set?)")]
    Home,

    #[error("duplicate ID in modification times: {id}")]
    ModifiedDupe { id: String },

    #[error("parsing modified IDs: {0}")]
    ModifiedParse(#[from] csv::Error),

    #[error("requesting ID modification times: {0}")]
    ModifiedRequest(#[source] reqwest::Error),

    #[error("ID modification time response: {0}")]
    ModifiedResponse(#[source] reqwest::Error),

    #[error("reading entries in OSV cache: {0}")]
    ReadDir(#[source] std::io::Error),
}
