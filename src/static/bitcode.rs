use std::{collections::BTreeSet, fmt::Debug, path::PathBuf};

use capslock::Report;
use llvm_ir_analysis::{ModuleAnalysis, llvm_ir::Module};
use ouroboros::self_referencing;
use petgraph::graphmap::DiGraphMap;

use crate::{
    caps::FunctionCaps, function::FunctionMap, graph::CallGraph, location::IntoOptionLocation,
};

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
        let path = path.into();
        let module = Module::from_bc_path(&path).map_err(|s| anyhow::anyhow!("{s}"))?;

        let inner = InnerBuilder {
            module,
            analysis_builder: |module| ModuleAnalysis::new(module),
        }
        .build();

        // We need the function map for everything else to make sense.
        let mut functions = inner.build_function_map(function_caps)?;

        // Get the call graph and adapt it for what we need to report later.
        let call_graph = inner.build_call_graph(&functions);

        // Bubble the direct capabilities up as transitive capabilities via the call graph.
        call_graph.bubble_transitive_capabilities(&mut functions);

        // TODO: gather package, module, and build metadata.

        Ok(Self {
            path,
            functions,
            call_graph,
        })
    }

    #[tracing::instrument(skip(self))]
    pub fn into_report(self) -> Report {
        let functions = self.functions.into_functions();

        let capabilities = functions.iter().fold(BTreeSet::new(), |mut acc, func| {
            acc.extend(func.capabilities.keys().copied());
            acc
        });

        Report {
            path: self.path,
            capabilities,
            functions,
            edges: self.call_graph.into(),
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

impl Inner {
    #[tracing::instrument(skip_all)]
    fn build_call_graph(&self, functions: &FunctionMap) -> CallGraph {
        self.with_analysis(|analysis| {
            let call_graph = analysis.call_graph();
            let inner = call_graph.inner();

            let mut graph = DiGraphMap::with_capacity(inner.node_count(), inner.edge_count());

            for (caller, callee, call) in inner.all_edges() {
                let caller = functions.get_index(caller).unwrap();
                let callee = functions.get_index(callee).unwrap();
                let location = call.debugloc().into_option_location();

                graph.add_edge(caller, callee, location);
            }

            graph.into()
        })
    }

    #[tracing::instrument(skip_all, err)]
    fn build_function_map(&self, function_caps: &FunctionCaps) -> anyhow::Result<FunctionMap> {
        // TODO: figure out if we need to do anything with ifuncs.
        let module = self.borrow_module();
        let mut map = FunctionMap::default();

        for func in module.functions.iter() {
            map.upsert_with_caps(function_caps, func)?;
        }

        for func in module.func_declarations.iter() {
            map.upsert_with_caps(function_caps, func)?;
        }

        Ok(map)
    }
}
