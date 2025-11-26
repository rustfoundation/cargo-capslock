use std::{
    collections::BTreeSet,
    fmt::Debug,
    ops::{Deref, DerefMut},
};

use capslock::{
    CapabilityType,
    report::{Edge, Location},
};
use petgraph::prelude::DiGraphMap;

use crate::function::FunctionMap;

#[derive(Default)]
pub struct CallGraph(DiGraphMap<usize, Option<Location>>);

impl Debug for CallGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CallGraph")
            .field("node_count", &self.0.node_count())
            .field("edge_count", &self.0.edge_count())
            .finish()
    }
}

impl CallGraph {
    #[tracing::instrument(skip_all)]
    pub fn bubble_transitive_capabilities(&self, functions: &mut FunctionMap) {
        // This is about the stupidest possible way to do this, but hey, I have a film degree.
        let mut changed = true;
        while changed {
            changed = false;

            for (caller, callee, _) in self.0.all_edges() {
                let callee_caps = functions
                    .get(callee)
                    .unwrap()
                    .capabilities
                    .keys()
                    .copied()
                    .collect::<BTreeSet<_>>();
                let caller = functions.get_mut(caller).unwrap();

                for cap in callee_caps.iter() {
                    if !caller.capabilities.contains_key(cap) {
                        caller.capabilities.insert(*cap, CapabilityType::Transitive);
                        changed = true;
                    }
                }
            }
        }
    }
}

impl Deref for CallGraph {
    type Target = DiGraphMap<usize, Option<Location>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for CallGraph {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<DiGraphMap<usize, Option<Location>>> for CallGraph {
    fn from(value: DiGraphMap<usize, Option<Location>>) -> Self {
        Self(value)
    }
}

impl From<CallGraph> for Vec<Edge> {
    fn from(call_graph: CallGraph) -> Self {
        call_graph
            .all_edges()
            .map(|(caller, callee, location)| Edge {
                caller,
                callee,
                location: location.clone(),
            })
            .collect()
    }
}
