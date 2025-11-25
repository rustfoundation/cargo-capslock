use std::{ffi::OsString, os::unix::ffi::OsStrExt, path::PathBuf};

use clap::Parser;
use escargot::{
    CargoBuild, CommandMessages,
    format::{Artifact, Message},
};
use itertools::Itertools;
use tempfile::TempDir;
use walkdir::WalkDir;

use crate::{bitcode::Bitcode, cargo::ExecutableSet};

mod bitcode;
mod cargo;

#[derive(Parser)]
pub struct Opt {
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

impl Opt {
    pub fn main(self) -> anyhow::Result<()> {
        let Opt {
            bin,
            package,
            release,
            rust_toolchain,
            workspace,
            path,
        } = Opt::parse();

        // Set up a temporary target directory so that we don't have to worry about
        // cross-contamination, and we know exactly which `.bc` files are relevant.
        let target = TempDir::new()?;

        let mut cargo = CargoBuild::new()
            // This is the key: we need to emit an LLVM bitcode file.
            .env("RUSTFLAGS", "--emit=llvm-bc")
            // Control the Rust version (which indirectly controls the LLVM version, which is really
            // what we care about).
            .env("RUSTUP_TOOLCHAIN", rust_toolchain)
            .target_dir(target.path());

        if let Some(bin) = bin {
            cargo = cargo.bin(bin);
        }
        if let Some(package) = package {
            cargo = cargo.package(package);
        }
        if release {
            cargo = cargo.release();
        }
        if workspace {
            cargo = cargo.arg("--workspace");
        }

        let path = match path {
            Some(path) => path,
            None => std::env::current_dir()?,
        };

        // We can't set the working directory using escargot, so we'll get the underlying `Command`,
        // mutate that, and then pass it back to `CommandMessages`.
        let mut cmd = cargo.into_command();
        cmd.current_dir(path);

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

        for path_result in WalkDir::new(
            target
                .path()
                .join(if release { "release" } else { "debug" })
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
            let bitcode = Bitcode::from_bc_path(path_result?)?;

            // FIXME: just outputting the JSON blobs one after another isn't particularly useful. We
            // should only do this if there's only one executable, otherwise we should require
            // outputting to a directory.
            serde_json::to_writer_pretty(std::io::stdout(), &bitcode.into_report())?;
            println!();
        }

        Ok(())
    }
}
