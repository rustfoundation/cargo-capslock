use std::{fmt::Display, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub path: PathBuf,
    pub functions: Vec<Function>,
    pub edges: Vec<Edge>,
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
}

impl Function {
    pub fn display_name(&self) -> &str {
        match &self.name {
            FunctionName::Rust { display_name, .. } => display_name,
            FunctionName::Other { display_name, .. } => display_name,
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
