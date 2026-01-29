use clap::{Parser, Subcommand};
use tracing_subscriber::{
    EnvFilter,
    fmt::{format::FmtSpan, time::uptime},
};

mod caps;
mod function;
mod graph;
mod location;
mod runtime;
mod r#static;
mod syscall;

#[derive(Parser)]
pub struct Opt {
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

        match self.command {
            Command::Static(cmd) => cmd.main(),
            Command::Runtime(cmd) => Ok(cmd.main()?),
        }
    }
}

#[derive(Subcommand)]
pub enum Command {
    /// Build and statically analyse a Rust project.
    Static(r#static::Static),

    /// Run and analyse a process.
    ///
    /// If the process isn't built in Rust, or lacks debuginfo, this probably won't be as effective
    /// as you might hope.
    Runtime(runtime::Runtime),
}
