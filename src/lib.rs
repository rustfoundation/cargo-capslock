use std::path::PathBuf;

use clap::{Parser, Subcommand};
use tracing_subscriber::{
    EnvFilter,
    fmt::{format::FmtSpan, time::uptime},
};

use crate::caps::FunctionCaps;

mod caps;
mod dynamic;
mod r#static;

#[derive(Parser)]
pub struct Opt {
    #[arg(long)]
    function_caps: PathBuf,

    #[command(subcommand)]
    command: Command,
}

impl Opt {
    pub fn main(self) -> anyhow::Result<()> {
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_env("CARGO_CAPSLOCK_LOG"))
            .with_span_events(FmtSpan::CLOSE)
            .with_timer(uptime())
            .with_writer(std::io::stderr)
            .init();

        let function_caps = FunctionCaps::from_path(self.function_caps)?;

        match self.command {
            Command::Static(cmd) => cmd.main(function_caps),
        }
    }
}

#[derive(Subcommand)]
pub enum Command {
    /// Build and statically analyse a Rust project.
    Static(r#static::Static),
}
