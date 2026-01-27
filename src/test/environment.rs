use std::path::PathBuf;

use tempfile::TempDir;

use crate::test::error::Error;

#[derive(Debug)]
pub struct Environment {
    pub target: TempDir,
    pub workspace: Option<PathBuf>,
}

impl Environment {
    pub fn new(workspace: Option<PathBuf>) -> Result<Self, Error> {
        Ok(Self {
            target: TempDir::new().map_err(Error::TempDir)?,
            workspace,
        })
    }
}
