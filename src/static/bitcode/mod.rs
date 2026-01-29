use std::{collections::BTreeSet, fmt::Debug, path::PathBuf};

use capslock::{Report, report};

use crate::{caps::FunctionCaps, function::FunctionMap, graph::CallGraph};

#[cfg(feature = "inkwell")]
mod inkwell;

#[cfg(feature = "llvm-ir-analysis")]
mod llvm_ir;

pub struct Bitcode {
    path: PathBuf,
    functions: FunctionMap,
    call_graph: CallGraph,
}

impl Bitcode {
    #[tracing::instrument(skip(function_caps), err)]
    pub fn from_bc_path(
        path: impl Into<PathBuf> + Debug,
        function_caps: &FunctionCaps,
    ) -> anyhow::Result<Self> {
        #[cfg(all(feature = "inkwell", feature = "llvm-ir-analysis"))]
        {
            compile_error!("only one of the inkwell and llvm-ir-analysis features can be enabled");
        }

        #[cfg(not(any(feature = "inkwell", feature = "llvm-ir-analysis")))]
        {
            compile_error!("one of the inkwell and llvm-ir-analysis features must be enabled");
        }

        #[cfg(feature = "inkwell")]
        {
            return Ok(inkwell::from_bc_path(path, function_caps)?);
        }

        #[cfg(feature = "llvm-ir-analysis")]
        {
            return llvm_ir::from_bc_path(path, function_caps);
        }
    }

    #[tracing::instrument(skip(self))]
    pub fn into_report(self) -> Report {
        let functions = self.functions.into_functions();

        let capabilities = functions.iter().fold(BTreeSet::new(), |mut acc, func| {
            acc.extend(func.capabilities.keys().copied());
            acc
        });

        Report {
            process: report::Process {
                path: self.path,
                capabilities,
                functions,
                edges: self.call_graph.into(),
            },
            children: Vec::new(),
        }
    }
}

impl Debug for Bitcode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bitcode").field("path", &self.path).finish()
    }
}
