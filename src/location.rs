#[cfg(feature = "llvm-ir-analysis")]
use std::path::{Path, PathBuf};

use capslock::report::Location;

#[cfg(feature = "llvm-ir-analysis")]
pub trait IntoLocation {
    #[allow(clippy::wrong_self_convention)]
    fn into_location(&self) -> Location;
}

pub trait IntoOptionLocation {
    fn into_option_location(self) -> Option<Location>;
}

#[cfg(feature = "llvm-ir-analysis")]
impl IntoLocation for llvm_ir_analysis::llvm_ir::DebugLoc {
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

#[cfg(feature = "llvm-ir-analysis")]
impl IntoOptionLocation for Option<&llvm_ir_analysis::llvm_ir::DebugLoc> {
    fn into_option_location(self) -> Option<Location> {
        self.map(|loc| loc.into_location())
    }
}

#[cfg(feature = "llvm-ir-analysis")]
impl IntoOptionLocation for &Option<llvm_ir_analysis::llvm_ir::DebugLoc> {
    fn into_option_location(self) -> Option<Location> {
        self.as_ref().map(|loc| loc.into_location())
    }
}

#[cfg(feature = "inkwell")]
impl<'a> IntoOptionLocation for inkwell::values::FunctionValue<'a> {
    fn into_option_location(self) -> Option<Location> {
        use inkwell::values::AsValueRef;

        unsafe { inkwell_util::from_value_ref(self.as_value_ref()) }
    }
}

#[cfg(feature = "inkwell")]
impl<'a> IntoOptionLocation for inkwell::values::InstructionValue<'a> {
    fn into_option_location(self) -> Option<Location> {
        use inkwell::values::AsValueRef;

        unsafe { inkwell_util::from_value_ref(self.as_value_ref()) }
    }
}

#[cfg(feature = "inkwell")]
mod inkwell_util {
    use std::{
        ffi::{CStr, OsStr, c_char, c_uint},
        os::unix::ffi::OsStrExt,
        path::{Path, PathBuf},
    };

    use capslock::report::Location;
    use inkwell::llvm_sys::{
        core::{
            LLVMGetDebugLocColumn, LLVMGetDebugLocDirectory, LLVMGetDebugLocFilename,
            LLVMGetDebugLocLine,
        },
        prelude::LLVMValueRef,
    };

    pub unsafe fn from_value_ref(value: LLVMValueRef) -> Option<Location> {
        let line = unsafe { LLVMGetDebugLocLine(value) } as u64;
        let column = match unsafe { LLVMGetDebugLocColumn(value) } {
            0 => None,
            col => Some(col as u64),
        };
        let directory = get_null_terminated_path(
            |value, len| unsafe { LLVMGetDebugLocDirectory(value, len) },
            value,
        );
        let filename = get_null_terminated_path(
            |value, len| unsafe { LLVMGetDebugLocFilename(value, len) },
            value,
        );

        filename.map(|filename| Location {
            directory,
            filename,
            line,
            column,
        })
    }

    fn get_null_terminated_path<F>(f: F, value: LLVMValueRef) -> Option<PathBuf>
    where
        F: FnOnce(LLVMValueRef, *mut c_uint) -> *const c_char,
    {
        let mut len = 0;
        let result = f(value, &mut len);
        if result.is_null() {
            None
        } else {
            let cs = unsafe { CStr::from_ptr(result) };
            let os = OsStr::from_bytes(cs.to_bytes());
            Some(Path::new(os).to_path_buf())
        }
    }
}
