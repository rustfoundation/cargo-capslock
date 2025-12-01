use std::collections::{BTreeSet, HashSet};

use capslock::Capability;
use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Function {
    pub caps: HashSet<Capability>,
    pub syscalls: BTreeSet<String>,
}
