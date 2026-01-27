use std::{collections::BTreeSet, path::PathBuf, process::Command};

use escargot::{
    CommandMessages,
    format::{Artifact, Message::CompilerArtifact},
};
use indicatif::ProgressBar;

use crate::test::{environment::Environment, error::Error};

#[tracing::instrument]
pub fn enumerate(env: &Environment, bar: &ProgressBar) -> Result<BTreeSet<PathBuf>, Error> {
    let mut cmd = Command::new("cargo");
    cmd.args([
        "test",
        "--message-format",
        "json",
        "--workspace",
        "--all-targets",
        "--no-run",
        "--target-dir",
    ])
    .arg(env.target.path())
    .env("RUSTFLAGS", "-Cdebuginfo=2 -Cstrip=none");

    if let Some(workspace) = env.workspace.as_ref() {
        cmd.current_dir(workspace);
    }

    let messages = CommandMessages::with_command(cmd)?;
    let mut tests = BTreeSet::new();
    for result in messages {
        if let CompilerArtifact(Artifact {
            executable: Some(executable),
            ..
        }) = result?.decode()?
        {
            tests.insert(executable.to_path_buf());
            bar.inc(1);
        }
    }

    Ok(tests)
}
