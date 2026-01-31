use std::{collections::BTreeSet, fs::File, io::BufReader, path::PathBuf};

use capslock::Capability;
use clap::Parser;
use serde::Deserialize;

use crate::{
    error::Error,
    seccomp::{Action, ActionDef, Policy},
    syscalls::CapabilityMap,
};

mod error;
mod seccomp;
mod syscalls;

#[derive(Parser)]
struct Opt {
    #[arg(long)]
    architectures: Vec<String>,

    #[command(flatten)]
    default_action: ActionDef,

    #[arg()]
    input: PathBuf,
}

#[derive(Deserialize)]
struct Capslock {
    capabilities: BTreeSet<Capability>,
}

fn main() -> Result<(), Error> {
    let opt = Opt::parse();
    let default_action = Action::try_from(opt.default_action)?;

    // We only need the capabilities, so we'll just parse those out for now.
    let Capslock { capabilities } = serde_json::from_reader(BufReader::new(
        File::open(opt.input).map_err(Error::InputOpen)?,
    ))
    .map_err(Error::InputParse)?;

    // Start building the policy.
    let mut policy = Policy::new(default_action);
    if !opt.architectures.is_empty() {
        for arch in opt.architectures.into_iter() {
            policy.add_architecture(arch);
        }
    }

    // Add syscalls to be allowed based on the capabilities in the input.
    let cap_map = CapabilityMap::new();
    policy.add_syscalls(
        Action::Allow,
        cap_map.get_syscalls(capabilities.into_iter()),
    );

    // There are also a handful of syscalls required by runc itself that we must
    // allow, but we'll log in case they're not previously allowed.
    policy.add_syscalls(
        Action::Log,
        [
            "capget",
            "capset",
            "chdir",
            "close",
            "epoll_pwait",
            "execve",
            "fchown",
            "fstat",
            "futex",
            "getdents64",
            "getppid",
            "newfstatat",
            "openat",
            "prctl",
            "read",
            "setgid",
            "setgroups",
            "setuid",
            "write",
        ]
        .into_iter(),
    );

    // Output the policy.
    serde_json::to_writer_pretty(std::io::stdout(), &policy).map_err(Error::OutputWrite)?;

    Ok(())
}
