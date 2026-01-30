use std::{
    collections::{BTreeMap, HashSet},
    io::{BufReader, Read},
};

use capslock::Capability;

static SYSCALLS_CM: &[u8] = include_bytes!("../../syscalls.cm");

pub struct CapabilityMap(BTreeMap<String, HashSet<Capability>>);

impl CapabilityMap {
    pub fn new() -> Self {
        Self::from_reader(SYSCALLS_CM)
    }

    pub fn get_syscalls(
        &self,
        caps: impl Iterator<Item = Capability>,
    ) -> impl Iterator<Item = &str> + '_ {
        let required = caps.collect::<HashSet<_>>();

        // This is absolutely not the most efficient way to do this, but the
        // set's going to be small enough that the O(n) algorithm is fine in
        // practice.
        self.0.iter().filter_map(move |(syscall, caps)| {
            // The syscall must require a subset of or exactly the caps given.
            if (caps.len() == 1 && caps.contains(&Capability::Safe)) || caps.is_subset(&required) {
                Some(syscall.as_str())
            } else {
                None
            }
        })
    }

    fn from_reader(reader: impl Read) -> Self {
        Self(
            cm::Document::from_reader(BufReader::new(reader))
                .unwrap()
                .into_iter()
                .map(|(syscall, caps)| (syscall, caps.into_iter().collect()))
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    static TESTDATA: &[u8] = br#"
safe CAPABILITY_SAFE
files CAPABILITY_FILES
files_network CAPABILITY_FILES CAPABILITY_NETWORK
network_files CAPABILITY_NETWORK CAPABILITY_FILES
read_system_state CAPABILITY_READ_SYSTEM_STATE
all CAPABILITY_FILES CAPABILITY_NETWORK CAPABILITY_READ_SYSTEM_STATE
"#;

    #[test]
    fn map() {
        use Capability::*;

        let map = CapabilityMap::from_reader(TESTDATA);

        // Things that don't match anything.
        assert_syscalls(&map, &[], &["safe"]);
        assert_syscalls(&map, &[Cgo], &["safe"]);

        // Just one capability.
        assert_syscalls(&map, &[Files], &["safe", "files"]);

        // Two capabilities together.
        assert_syscalls(
            &map,
            &[Files, Network],
            &["safe", "files", "files_network", "network_files"],
        );

        // Two capabilities that match disjoint options.
        assert_syscalls(
            &map,
            &[Files, ReadSystemState],
            &["safe", "files", "read_system_state"],
        );

        // Three capabilities.
        assert_syscalls(
            &map,
            &[Files, Network, ReadSystemState],
            &[
                "safe",
                "files",
                "files_network",
                "network_files",
                "read_system_state",
                "all",
            ],
        );
    }

    #[track_caller]
    fn assert_syscalls(map: &CapabilityMap, caps: &[Capability], matches: &[&'static str]) {
        let syscalls = map
            .get_syscalls(caps.iter().copied())
            .collect::<BTreeSet<_>>();
        let matches = matches.iter().copied().collect::<BTreeSet<_>>();

        assert_eq!(syscalls, matches);
    }
}
