use std::{
    borrow::Cow,
    ffi::OsString,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use escargot::{
    CargoBuild, CommandMessages,
    format::{Artifact, Message},
};
use itertools::Itertools;
use tempfile::TempDir;
use tracing_subscriber::{
    EnvFilter,
    fmt::{format::FmtSpan, time::uptime},
};
use walkdir::WalkDir;

use crate::{bitcode::Bitcode, caps::FunctionCaps, cargo::ExecutableSet};

mod bitcode;
mod caps;
mod cargo;

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
    Static(Static),
}

#[derive(Parser, Debug)]
pub struct Static {
    /// Build only the specified binary.
    #[arg(long)]
    bin: Option<OsString>,

    /// Package to build.
    #[arg(short, long)]
    package: Option<OsString>,

    /// Build artifacts in release mode.
    #[arg(short, long)]
    release: bool,

    /// Rust toolchain to use.
    ///
    /// This is mostly relevant in terms of the LLVM version.
    #[arg(long, default_value = "1.86.0")]
    rust_toolchain: String,

    /// Build all packages in the workspace.
    #[arg(long)]
    workspace: bool,

    /// Path to the workspace, or the current working directory if omitted.
    #[arg()]
    path: Option<PathBuf>,
}

impl Static {
    #[tracing::instrument(skip(function_caps), err)]
    pub fn main(self, function_caps: FunctionCaps) -> anyhow::Result<()> {
        // Set up a temporary target directory so that we don't have to worry about
        // cross-contamination, and we know exactly which `.bc` files are relevant.
        let target = TempDir::new()?;

        // Build the package.
        let exes = self.build(target.path())?;

        // Process the generated bitcode files.
        for path_result in WalkDir::new(
            target
                .path()
                .join(if self.release { "release" } else { "debug" })
                .join("deps"),
        )
        .into_iter()
        .filter_map_ok(|entry| {
            if entry.file_type().is_file()
                && let Some(file_name) = entry.path().file_name()
                && entry
                    .path()
                    .extension()
                    .is_some_and(|ext| ext.as_bytes() == b"bc")
                && exes.contains_prefix_match(file_name)
            {
                Some(entry.into_path())
            } else {
                None
            }
        }) {
            let bitcode = Bitcode::from_bc_path(path_result?, &function_caps)?;

            // FIXME: just outputting the JSON blobs one after another isn't particularly useful. We
            // should only do this if there's only one executable, otherwise we should require
            // outputting to a directory.
            serde_json::to_writer_pretty(std::io::stdout(), &bitcode.into_report())?;
            println!();
        }

        Ok(())
    }

    #[tracing::instrument(skip_all, err)]
    fn build(&self, target: &Path) -> anyhow::Result<ExecutableSet> {
        let mut cargo = CargoBuild::new()
            // This is the key: we need to emit an LLVM bitcode file.
            .env("RUSTFLAGS", "--emit=llvm-bc")
            // Control the Rust version (which indirectly controls the LLVM version, which is really
            // what we care about).
            .env("RUSTUP_TOOLCHAIN", &self.rust_toolchain)
            .target_dir(target);

        if let Some(bin) = &self.bin {
            cargo = cargo.bin(bin);
        }
        if let Some(package) = &self.package {
            cargo = cargo.package(package);
        }
        if self.release {
            cargo = cargo.release();
        }
        if self.workspace {
            cargo = cargo.arg("--workspace");
        }

        let path = match &self.path {
            Some(path) => Cow::Borrowed(path),
            None => Cow::Owned(std::env::current_dir()?),
        };

        // We can't set the working directory using escargot, so we'll get the underlying `Command`,
        // mutate that, and then pass it back to `CommandMessages`.
        let mut cmd = cargo.into_command();
        cmd.current_dir(path.as_path());

        // We have to iterate the messages for Cargo to progress, but we also want the executables
        // so we don't look at bincode files we're not interested in.
        //
        // FIXME: extend to detect and handle shared libraries.
        let mut exes = ExecutableSet::default();
        for msg_result in CommandMessages::with_command(cmd)? {
            if let Message::CompilerArtifact(Artifact {
                executable: Some(exe),
                ..
            }) = msg_result?.decode()?
            {
                exes.insert(exe)?;
            }
        }

        Ok(exes)
    }
}
