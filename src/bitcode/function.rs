use std::{collections::HashMap, path::PathBuf};

use capslock::{FunctionName, Location, RustFunctionName};
use llvm_ir_analysis::llvm_ir::{self, DebugLoc};
use serde::Serialize;
use symbolic::{
    common::{Language, Name, NameMangling},
    demangle::{Demangle, DemangleOptions},
};
use thiserror::Error;

#[derive(Default, Debug, Serialize)]
pub struct FunctionMap {
    #[serde(flatten)]
    functions: Vec<capslock::Function>,
    #[serde(skip)]
    ids: HashMap<String, usize>,
}

impl FunctionMap {
    pub fn get_index(&self, mangled: &str) -> Option<usize> {
        self.ids.get(mangled).copied()
    }

    pub fn into_functions(self) -> Vec<capslock::Function> {
        self.functions
    }

    pub fn upsert_func(&mut self, func: &llvm_ir::Function) -> Result<(), Error> {
        self.upsert_function(
            &func.name,
            capslock::Function {
                name: parse_mangled_name(&func.name)?,
                location: convert_debugloc(&func.debugloc),
            },
        );

        Ok(())
    }

    pub fn upsert_func_decl(
        &mut self,
        func: &llvm_ir::function::FunctionDeclaration,
    ) -> Result<(), Error> {
        self.upsert_function(
            &func.name,
            capslock::Function {
                name: parse_mangled_name(&func.name)?,
                location: convert_debugloc(&func.debugloc),
            },
        );

        Ok(())
    }

    fn upsert_function(&mut self, mangled: &str, function: capslock::Function) {
        if !self.ids.contains_key(mangled) {
            self.ids.insert(mangled.to_string(), self.functions.len());
            self.functions.push(function);
        }
    }
}

fn convert_debugloc(loc: &Option<DebugLoc>) -> Option<Location> {
    loc.as_ref().map(|loc| Location {
        directory: loc.directory.as_ref().map(PathBuf::from),
        filename: PathBuf::from(&loc.filename),
        line: loc.line as u64,
        column: loc.col.map(u64::from),
    })
}

fn parse_mangled_name(mangled: &str) -> Result<FunctionName, Error> {
    let name = Name::new(mangled, NameMangling::Mangled, Language::Unknown);

    match name.detect_language() {
        Language::Rust => {
            let demangled = name
                .demangle(DemangleOptions::name_only())
                .ok_or_else(|| Error::Demangle(mangled.to_string()))?;
            let rust = parse_rust_function_name(&demangled)?;

            Ok(FunctionName::Rust {
                display_name: demangled,
                name: rust,
            })
        }
        lang => Ok(FunctionName::Other {
            display_name: name.try_demangle(DemangleOptions::name_only()).to_string(),
            language: lang.to_string(),
        }),
    }
}

fn parse_rust_function_name(function: &str) -> Result<RustFunctionName, Error> {
    if let Some(function) = function.strip_prefix('<') {
        let (type_, rem) = function
            .split_once(" as ")
            .ok_or_else(|| Error::MalformedTrait(function.to_string()))?;

        let (trait_, method) = rem
            .rsplit_once(">::")
            .ok_or_else(|| Error::MalformedTraitMethod(function.to_string()))?;

        Ok(RustFunctionName::TraitMethod {
            trait_: trait_.to_string(),
            type_: type_.to_string(),
            method: method.to_string(),
        })
    } else if let Some((type_, method)) = function.rsplit_once("::") {
        Ok(RustFunctionName::StructMethod {
            type_: type_.to_string(),
            method: method.to_string(),
        })
    } else {
        Ok(RustFunctionName::Bare {
            function: function.to_string(),
        })
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("demangling failed for {0}")]
    Demangle(String),

    #[error("cannot parse {0} as a trait method")]
    MalformedTrait(String),

    #[error("cannot parse trait and method out of {0}")]
    MalformedTraitMethod(String),
}
