use std::path::PathBuf;

use clap::Parser;

use crate::{
    error::Error,
    seccomp::{Action, ActionDef},
};

mod error;
mod seccomp;

#[derive(Parser)]
struct Opt {
    #[arg(long)]
    architectures: Vec<String>,

    #[command(flatten)]
    default_action: ActionDef,

    #[arg()]
    input: PathBuf,
}

fn main() -> Result<(), Error> {
    let opt = Opt::parse();
    let default_action = Action::try_from(opt.default_action)?;

    // TODO: parse input JSON, pull out caps, map to syscalls via syscalls.cm, build policy, output policy.

    Ok(())
}
