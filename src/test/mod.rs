use std::{ffi::OsString, fs::File, io::Write, path::PathBuf, process::Command};

use capslock::Report;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use ptrace_iterator::{CommandTrace, Piddable, Tracer};

use crate::{
    dynamic::{Error as DynamicError, GlobalState, Meta, SignalForwarder, process},
    test::{environment::Environment, error::Error},
};

mod environment;
mod error;
mod unit;

#[derive(Parser, Debug)]
pub struct Test {
    /// If enabled, functions before `_start` will also be included in the output.
    #[arg(long)]
    include_before_start: bool,

    /// If enabled, the actual syscalls invoked will be included in the output.
    #[arg(long)]
    include_syscalls: bool,

    /// If enabled, source file locations will be looked up via debuginfo.
    ///
    /// This tends to have a significant performance impact.
    #[arg(short, long)]
    lookup_locations: bool,

    /// If provided, the file to write the JSON output to.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// The workspace to build and run tests within.
    ///
    /// If omitted, the current directory is used.
    #[arg(long)]
    workspace: Option<PathBuf>,
}

impl Test {
    #[tracing::instrument(err)]
    pub fn main(mut self) -> Result<(), Error> {
        let env = Environment::new(self.workspace.take())?;

        // TODO: also doctests.
        let bar = ProgressBar::new(0)
            .with_style(
                ProgressStyle::with_template("Collecting {msg} tests: found {pos} so far").unwrap(),
            )
            .with_message("unit");
        let binaries = unit::enumerate(&env, &bar)?;
        bar.finish_and_clear();

        let mut processes = Vec::new();
        let bar = ProgressBar::new(binaries.len() as u64).with_style(
            ProgressStyle::with_template("Running tests: {wide_bar} {pos}/{len} ETA: {eta}")
                .unwrap(),
        );
        for binary in binaries.into_iter() {
            let report = self.trace(binary)?;
            processes.extend(report.processes.into_iter());
            bar.inc(1);
        }
        bar.finish_and_clear();

        // FIXME: this isn't really great output, truthfully.
        let report = Report { processes };

        // Output the Capslock JSON.
        let mut writer: Box<dyn Write> = if let Some(output) = self.output {
            Box::new(
                File::create(&output).map_err(|e| DynamicError::OutputCreate {
                    e,
                    path: output.to_path_buf(),
                })?,
            )
        } else {
            Box::new(std::io::stdout())
        };
        serde_json::to_writer_pretty(&mut writer, &report).map_err(DynamicError::from)?;

        Ok(())
    }

    #[tracing::instrument(err)]
    fn trace(&self, binary: PathBuf) -> Result<Report, Error> {
        // TODO: dedupe a bunch of this with dynamic::Dynamic::main().

        // Spawn the command we're going to trace.
        let mut cmd = Command::new(&binary);
        cmd.traceme();
        let child = cmd.spawn().map_err(DynamicError::Spawn)?;
        let child_pid = child.id().into_pid();

        // Set up signal handling to pass signals on to the child.
        let signal_forwarder = SignalForwarder::spawn(child.id())?;

        let mut global_state = GlobalState::new(
            child_pid,
            process::Exec::new(
                binary,
                std::iter::empty::<OsString>(),
                std::iter::empty::<OsString>(),
            ),
            std::env::current_dir().map_err(DynamicError::Cwd)?,
            self.include_before_start,
            self.include_syscalls,
            self.lookup_locations,
        );

        // Actually start tracing the child.
        let mut tracer = Tracer::<Meta>::new(child).map_err(DynamicError::from)?;
        for event_result in tracer.iter() {
            let mut event = match event_result {
                Ok(event) => event,
                Err(e) => {
                    tracing::error!(%e, "tracer error");
                    continue;
                }
            };

            if let Err(e) = global_state.handle_event(&mut event) {
                tracing::debug!(%e, "error handling event");
            }
        }

        // Stop forwarding signals, since there's no longer a child process.
        drop(signal_forwarder);

        Ok(global_state.into_report(true)?)
    }
}
