use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("unknown action: {0}")]
    ActionUnknown(String),

    #[error("opening input: {0}")]
    InputOpen(#[source] std::io::Error),

    #[error("parsing input: {0}")]
    InputParse(#[source] serde_json::Error),

    #[error("SCMP_ACT_ERRNO given, but no errno provided")]
    NoErrno,

    #[error("SCMP_ACT_TRACE given, but no trace process provided")]
    NoTrace,

    #[error("writing output: {0}")]
    OutputWrite(#[source] serde_json::Error),
}
