use std::{collections::HashMap, fmt::Debug, fs::File, path::Path};

use capslock_rust::Function;
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
pub struct FunctionCaps(HashMap<String, Function>);

impl FunctionCaps {
    #[tracing::instrument(err)]
    pub fn from_path(path: impl AsRef<Path> + Debug) -> anyhow::Result<Self> {
        Ok(serde_json::from_reader(File::open(path.as_ref())?)?)
    }

    pub fn get(&self, name: &str) -> Option<&Function> {
        self.0.get(name)
    }
}
