use std::path::{Path, PathBuf};

use capslock::report::Location;
use llvm_ir_analysis::llvm_ir::DebugLoc;

pub trait IntoLocation {
    #[allow(clippy::wrong_self_convention)]
    fn into_location(&self) -> Location;
}

pub trait IntoOptionLocation {
    fn into_option_location(self) -> Option<Location>;
}

impl IntoLocation for DebugLoc {
    fn into_location(&self) -> Location {
        // The way LLVM reports directory and filename probably isn't what people would expect to
        // see, since it often ends up being something like `/path/to/project` and then
        // `src/main.rs`, so we'll rebuild them from what we have.
        let path = match &self.directory {
            Some(dir) => Path::new(dir).join(&self.filename),
            None => PathBuf::from(&self.filename),
        };
        let directory = path.parent().map(PathBuf::from);
        let filename = path
            .file_name()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(".."));

        Location {
            directory,
            filename,
            line: self.line as u64,
            column: self.col.map(u64::from),
        }
    }
}

impl IntoOptionLocation for Option<&DebugLoc> {
    fn into_option_location(self) -> Option<Location> {
        self.map(|loc| loc.into_location())
    }
}

impl IntoOptionLocation for &Option<DebugLoc> {
    fn into_option_location(self) -> Option<Location> {
        self.as_ref().map(|loc| loc.into_location())
    }
}
