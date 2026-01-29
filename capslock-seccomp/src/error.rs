use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("unknown action: {0}")]
    ActionUnknown(String),

    #[error("SCMP_ACT_ERRNO given, but no errno provided")]
    NoErrno,

    #[error("SCMP_ACT_TRACE given, but no trace process provided")]
    NoTrace,
}
