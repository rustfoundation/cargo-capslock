use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{Capability, caps::CapabilityType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    #[serde(flatten)]
    pub process: Process,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Process>,
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
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub syscalls: BTreeSet<String>,
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

    pub fn insert_syscall(&mut self, syscall: impl ToString) {
        self.syscalls.insert(syscall.to_string());
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

#[derive(Debug, Clone, Deserialize)]
pub struct Process {
    pub path: PathBuf,
    pub capabilities: BTreeSet<Capability>,
    pub functions: Vec<Function>,
    pub edges: Vec<Edge>,
}

impl Serialize for Process {
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
            #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
            syscalls: BTreeSet<&'a str>,
        }

        // Always remove safe from the top level capability list.
        let capabilities = self
            .capabilities
            .iter()
            .copied()
            .filter(|cap| cap != &Capability::Safe)
            .collect::<BTreeSet<_>>();

        Raw {
            path: &self.path,
            capabilities,
            functions: &self.functions,
            edges: &self.edges,
            syscalls: collect_syscalls(&self.functions),
        }
        .serialize(serializer)
    }
}

fn collect_syscalls(functions: &[Function]) -> BTreeSet<&str> {
    functions
        .iter()
        .fold(BTreeSet::new(), |mut syscalls, func| {
            syscalls.extend(func.syscalls.iter().map(|syscall| syscall.as_str()));
            syscalls
        })
}
