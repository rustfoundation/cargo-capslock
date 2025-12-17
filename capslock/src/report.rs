use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{Capability, caps::CapabilityType};

#[derive(Debug, Clone, Deserialize)]
pub struct Report {
    pub path: PathBuf,
    pub capabilities: BTreeSet<Capability>,
    pub functions: Vec<Function>,
    pub edges: Vec<Edge>,
}

impl Serialize for Report {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct Raw<'a> {
            path: &'a Path,
            capabilities: BTreeSet<Capability>,
            functions: &'a [Function],
            edges: &'a [Edge],
        }

        Raw {
            path: &self.path,
            capabilities: self
                .capabilities
                .iter()
                .copied()
                .filter(|cap| self.capabilities.len() < 2 || *cap != Capability::Safe)
                .collect(),
            functions: &self.functions,
            edges: &self.edges,
        }
        .serialize(serializer)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub caller: usize,
    pub callee: usize,
    pub location: Option<Location>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    #[serde(flatten)]
    pub name: FunctionName,
    pub location: Option<Location>,
    pub capabilities: BTreeMap<Capability, CapabilityType>,
}

impl Function {
    pub fn display_name(&self) -> &str {
        match &self.name {
            FunctionName::Rust { display_name, .. } => display_name,
            FunctionName::Other { display_name, .. } => display_name,
        }
    }

    pub fn insert_capability(&mut self, capability: Capability, ty: CapabilityType) {
        use std::collections::btree_map::Entry::*;

        match self.capabilities.entry(capability) {
            Vacant(entry) => {
                entry.insert(ty);
            }
            Occupied(mut entry) => {
                entry.insert(std::cmp::max(*entry.get(), ty));
            }
        }
    }
}

impl Display for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.display_name().fmt(f)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum FunctionName {
    Rust {
        display_name: String,
        name: RustFunctionName,
    },
    Other {
        display_name: String,
        language: String,
    },
}

impl FunctionName {
    pub fn display_name(&self) -> &str {
        match self {
            FunctionName::Rust { display_name, .. } => display_name,
            FunctionName::Other { display_name, .. } => display_name,
        }
    }
}

impl Serialize for FunctionName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Rust { display_name, name } => {
                #[derive(Serialize)]
                struct Raw<'a> {
                    display_name: &'a str,
                    name: &'a RustFunctionName,
                    language: &'static str,
                }

                Raw {
                    display_name,
                    name,
                    language: "rust",
                }
                .serialize(serializer)
            }
            Self::Other {
                display_name,
                language,
            } => {
                #[derive(Serialize)]
                struct Raw<'a> {
                    display_name: &'a str,
                    language: &'a str,
                }

                Raw {
                    display_name,
                    language,
                }
                .serialize(serializer)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RustFunctionName {
    TraitMethod {
        #[serde(rename = "trait")]
        trait_: String,
        #[serde(rename = "type")]
        type_: String,
        method: String,
    },
    StructMethod {
        #[serde(rename = "type")]
        type_: String,
        method: String,
    },
    Bare {
        function: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub directory: Option<PathBuf>,
    pub filename: PathBuf,
    pub line: u64,
    pub column: Option<u64>,
}
