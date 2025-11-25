use std::{fmt::Debug, path::PathBuf};

use capslock::{Edge, Report};
use llvm_ir_analysis::{ModuleAnalysis, llvm_ir::Module};
use ouroboros::self_referencing;

use crate::bitcode::function::FunctionMap;

mod function;

pub struct Bitcode {
    path: PathBuf,
    functions: FunctionMap,
    edges: Vec<Edge>,
}

impl Bitcode {
    pub fn from_bc_path(path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let path = path.into();
        let module = Module::from_bc_path(&path).map_err(|s| anyhow::anyhow!("{s}"))?;

        let inner = InnerBuilder {
            module,
            analysis_builder: |module| ModuleAnalysis::new(module),
        }
        .build();

        // We need the function map for everything else to make sense.
        let functions = build_function_map(&inner)?;

        // XXX: we can probably parallelise further analysis.
        let edges = build_edges(&inner, &functions);

        // TODO: gather package, module, and build metadata.

        // TODO: match function calls against a known map of function -> capabilities, and then
        // output a summary of what capabilities are in use.

        Ok(Self {
            path,
            functions,
            edges,
        })
    }

    pub fn into_report(self) -> Report {
        Report {
            path: self.path,
            functions: self.functions.into_functions(),
            edges: self.edges,
        }
    }
}

impl Debug for Bitcode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bitcode").field("path", &self.path).finish()
    }
}

#[self_referencing]
struct Inner {
    module: Module,
    #[borrows(module)]
    #[not_covariant]
    analysis: ModuleAnalysis<'this>,
}

fn build_edges(inner: &Inner, functions: &FunctionMap) -> Vec<Edge> {
    inner.with_analysis(|analysis| {
        let mut edges = Vec::new();

        for (caller, callee, ()) in analysis.call_graph().inner().all_edges() {
            // FIXME: if we extend our llvm-ir fork to also include the Call in the digraph, then we
            // can get the call location.
            edges.push(Edge {
                caller: functions.get_index(caller).unwrap(),
                callee: functions.get_index(callee).unwrap(),
                location: None,
            })
        }

        edges
    })
}

fn build_function_map(inner: &Inner) -> anyhow::Result<FunctionMap> {
    // TODO: figure out if we need to do anything with ifuncs.
    let module = inner.borrow_module();
    let mut map = FunctionMap::default();

    for func in module.functions.iter() {
        map.upsert_func(func)?;
    }

    for func in module.func_declarations.iter() {
        map.upsert_func_decl(func)?;
    }

    Ok(map)
}
