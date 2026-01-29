use std::{collections::BTreeSet, fmt::Debug, path::PathBuf};

use capslock::{Report, report};
use llvm_ir_analysis::{ModuleAnalysis, llvm_ir::Module};

use crate::{
    caps::FunctionCaps, function::FunctionMap, graph::CallGraph, location::IntoOptionLocation,
};

pub struct Bitcode {
    path: PathBuf,
    functions: FunctionMap,
    call_graph: CallGraph,
}

pub struct Builder<'caps> {
    bitcode: Bitcode,
    function_caps: &'caps FunctionCaps,
}

impl<'caps> Builder<'caps> {
    pub fn new(path: PathBuf, function_caps: &'caps FunctionCaps) -> Self {
        Self {
            bitcode: Bitcode::new(path),
            function_caps,
        }
    }

    #[tracing::instrument(skip(self), err)]
    pub fn add_module(&mut self, path: impl Into<PathBuf> + Debug) -> anyhow::Result<()> {
        let path = path.into();
        let module = Module::from_bc_path(&path).map_err(|s| anyhow::anyhow!("{s}"))?;

        // We need the function map for everything else to make sense.
        self.upsert_function_map(&module)?;

        // Get the call graph and adapt it for what we need to report later.
        self.upsert_call_graph(&module)?;

        Ok(())
    }

    pub fn into_report(self) -> Report {
        // TODO: gather package, module, and build metadata.

        self.bitcode.into_report()
    }

    fn upsert_call_graph(&mut self, module: &Module) -> anyhow::Result<()> {
        let analysis = ModuleAnalysis::new(module);

        let call_graph = analysis.call_graph();
        let inner = call_graph.inner();

        for (caller, callee, call) in inner.all_edges() {
            let caller = self.bitcode.functions.get_index(caller).unwrap();
            let callee = self.bitcode.functions.get_index(callee).unwrap();
            let location = call.debugloc().into_option_location();

            self.bitcode.call_graph.add_edge(caller, callee, location);
        }

        Ok(())
    }

    fn upsert_function_map(&mut self, module: &Module) -> anyhow::Result<()> {
        for func in module.functions.iter() {
            self.bitcode
                .functions
                .upsert_with_caps(self.function_caps, func)?;
        }

        for func in module.func_declarations.iter() {
            self.bitcode
                .functions
                .upsert_with_caps(self.function_caps, func)?;
        }

        Ok(())
    }
}

impl Bitcode {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            functions: FunctionMap::default(),
            call_graph: CallGraph::default(),
        }
    }

    #[tracing::instrument(skip(self))]
    pub fn into_report(self) -> Report {
        let Self {
            path,
            mut functions,
            call_graph,
        } = self;

        // Bubble the direct capabilities up as transitive capabilities via the call graph.
        call_graph.bubble_transitive_capabilities(&mut functions);

        let functions = functions.into_functions();

        let capabilities = functions.iter().fold(BTreeSet::new(), |mut acc, func| {
            acc.extend(func.capabilities.keys().copied());
            acc
        });

        Report {
            process: report::Process {
                path,
                capabilities,
                functions,
                edges: call_graph.into(),
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
