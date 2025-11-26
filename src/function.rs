use std::collections::{BTreeMap, HashMap};

use capslock::{
    Capability, CapabilityType,
    report::{self, FunctionName, RustFunctionName},
};
use llvm_ir_analysis::llvm_ir::{self, DebugLoc};
use symbolic::{
    common::{Language, Name, NameMangling},
    demangle::{Demangle, DemangleOptions},
};
use thiserror::Error;

use crate::{caps::FunctionCaps, location::IntoOptionLocation};

#[derive(Default, Debug)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct FunctionMap {
    functions: Vec<report::Function>,
    ids: HashMap<String, usize>,
}

impl FunctionMap {
    pub fn get(&self, idx: usize) -> Option<&report::Function> {
        self.functions.get(idx)
    }

    pub fn get_index(&self, mangled: &str) -> Option<usize> {
        self.ids.get(mangled).copied()
    }

    pub fn get_mut(&mut self, idx: usize) -> Option<&mut report::Function> {
        self.functions.get_mut(idx)
    }

    pub fn into_functions(self) -> Vec<report::Function> {
        self.functions
    }

    pub fn upsert_with_caps(
        &mut self,
        function_caps: &FunctionCaps,
        function: impl ToFunction,
    ) -> Result<usize, Error> {
        Ok(self.upsert(
            function.mangled_name(),
            function.to_function_with_fn_caps(function_caps)?,
        ))
    }

    pub fn upsert(&mut self, mangled: &str, function: report::Function) -> usize {
        if let Some(idx) = self.ids.get(mangled) {
            *idx
        } else {
            let idx = self.functions.len();
            self.ids.insert(mangled.to_string(), idx);
            self.functions.push(function);

            idx
        }
    }
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
        if let Some((type_, rem)) = function.split_once(" as ") {
            let (trait_, method) = rem
                .rsplit_once(">::")
                .ok_or_else(|| Error::MalformedTraitMethod(function.to_string()))?;

            Ok(RustFunctionName::TraitMethod {
                trait_: trait_.to_string(),
                type_: type_.to_string(),
                method: method.to_string(),
            })
        } else {
            let (type_, method) = function
                .rsplit_once(">::")
                .ok_or_else(|| Error::MalformedMethod(function.to_string()))?;

            Ok(RustFunctionName::StructMethod {
                type_: type_.to_string(),
                method: method.to_string(),
            })
        }
    } else if !function.ends_with('>')
        && let Some((type_, method)) = function.rsplit_once("::")
    {
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

    #[error("cannot parse a type and method out of {0}")]
    MalformedMethod(String),

    #[error("cannot parse trait and method out of {0}")]
    MalformedTraitMethod(String),
}

pub trait ToFunction {
    fn debugloc(&self) -> Option<&DebugLoc>;
    fn mangled_name(&self) -> &str;

    fn to_function(&self) -> Result<report::Function, Error> {
        Ok(report::Function {
            name: parse_mangled_name(self.mangled_name())?,
            location: self.debugloc().into_option_location(),
            capabilities: BTreeMap::new(),
        })
    }

    fn to_function_with_caps(
        &self,
        caps: impl Iterator<Item = (Capability, CapabilityType)>,
    ) -> Result<report::Function, Error> {
        Ok(report::Function {
            name: parse_mangled_name(self.mangled_name())?,
            location: self.debugloc().into_option_location(),
            capabilities: caps.collect(),
        })
    }

    fn to_function_with_fn_caps(
        &self,
        function_caps: &FunctionCaps,
    ) -> Result<report::Function, Error> {
        let name = parse_mangled_name(self.mangled_name())?;
        let capabilities = direct_fn_caps(function_caps, &name);

        Ok(report::Function {
            name,
            location: self.debugloc().into_option_location(),
            capabilities,
        })
    }
}

impl ToFunction for &llvm_ir::Function {
    fn debugloc(&self) -> Option<&DebugLoc> {
        self.debugloc.as_ref()
    }

    fn mangled_name(&self) -> &str {
        &self.name
    }
}

impl ToFunction for &llvm_ir::function::FunctionDeclaration {
    fn debugloc(&self) -> Option<&DebugLoc> {
        self.debugloc.as_ref()
    }

    fn mangled_name(&self) -> &str {
        &self.name
    }
}

impl<'a> ToFunction for Name<'a> {
    fn debugloc(&self) -> Option<&DebugLoc> {
        None
    }

    fn mangled_name(&self) -> &str {
        self.as_str()
    }
}

fn direct_fn_caps(
    function_caps: &FunctionCaps,
    name: &FunctionName,
) -> BTreeMap<Capability, CapabilityType> {
    if let Some(caps) = function_caps.get(name.display_name()) {
        caps.caps
            .iter()
            .map(|cap| (*cap, CapabilityType::Direct))
            .collect()
    } else {
        BTreeMap::new()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn rust_demangling() -> anyhow::Result<()> {
        use super::parse_rust_function_name as parse;

        use insta::assert_compact_debug_snapshot as snapshot;

        // Success cases.
        snapshot!(parse("no_mangle")?, @r#"Bare { function: "no_mangle" }"#);
        snapshot!(parse("foo::bar")?, @r#"StructMethod { type_: "foo", method: "bar" }"#);
        snapshot!(parse("<axum::extract::path::Path<T> as axum_core::extract::FromRequestParts<S>>::from_request_parts")?, @r#"TraitMethod { trait_: "axum_core::extract::FromRequestParts<S>", type_: "axum::extract::path::Path<T>", method: "from_request_parts" }"#);
        snapshot!(parse("tower::util::map_err::_::<impl tower::util::map_err::MapErrFuture<F,N>>::project")?, @r#"StructMethod { type_: "tower::util::map_err::_::<impl tower::util::map_err::MapErrFuture<F,N>>", method: "project" }"#);

        // Failure cases.
        snapshot!(parse("<foo as bar"), @r#"Err(MalformedTraitMethod("foo as bar"))"#);
        snapshot!(parse("<foo>"), @r#"Err(MalformedMethod("foo>"))"#);

        Ok(())
    }
}
