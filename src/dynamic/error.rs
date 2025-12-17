use std::{ffi::c_ulong, num::ParseIntError, path::PathBuf};

use nix::{errno::Errno, unistd::Pid};
use ptrace_iterator::{Sysno, core::Fd};
use thiserror::Error;

use crate::dynamic::fd;

#[derive(Debug, Error)]
pub enum Error {
    #[error("cannot get argv[0]")]
    Argv0,

    #[error("child PID {0} missing")]
    ChildMissing(Pid),

    #[error("cannot get current working directory: {0}")]
    Cwd(#[source] std::io::Error),

    #[error("unknown ioctl command {cmd} for FD type {ty:?}")]
    Ioctl { cmd: c_ulong, ty: fd::Type },

    #[error("killing process {pid}: {e}")]
    Kill {
        #[source]
        e: Errno,
        pid: Pid,
    },

    #[error("creating output file {path:?}: {e}")]
    OutputCreate {
        #[source]
        e: std::io::Error,
        path: PathBuf,
    },

    #[error("writing to output file: {0}")]
    OutputWrite(#[from] serde_json::Error),

    #[error("cannot find active process: {0}")]
    ProcessFind(Pid),

    #[error("unknown process in tree: {0}")]
    ProcessUnknown(Pid),

    #[error("cannot find FD {fd} for PID {pid} in procfs: {e}")]
    ProcfsFd {
        #[source]
        e: std::io::Error,
        fd: Fd,
        pid: Pid,
    },

    #[error("cannot find FD info {fd} for PID {pid} in procfs: {e}")]
    ProcfsFdinfo {
        #[source]
        e: std::io::Error,
        fd: Fd,
        pid: Pid,
    },

    #[error("cannot parse flags {flags} for FD {fd} for PID {fd}: {e}")]
    ProcfsFdinfoFlag {
        #[source]
        e: ParseIntError,
        fd: Fd,
        flags: String,
        pid: Pid,
    },

    #[error("flags missing in FD info {fd} for PID {pid}")]
    ProcfsFdinfoMissing { fd: Fd, pid: Pid },

    #[error("cannot resolve path relative to PID {pid} FD {fd}: {path:?}")]
    Resolve { fd: Fd, path: PathBuf, pid: Pid },

    #[error("converting raw signal: {0}")]
    Signal(#[source] Errno),

    #[error("creating signals: {0}")]
    Signals(#[source] std::io::Error),

    #[error("socket type unknown: {0}")]
    SocketTypeUnknown(i32),

    #[error("spawning command: {0}")]
    Spawn(#[source] std::io::Error),

    #[error("syscall metadata missing")]
    SyscallMetaMissing,

    #[error("syscall missing in syscall capability map: {0}")]
    SyscallMissingFromMap(Sysno),

    #[error("no syscall available in exit event")]
    SyscallMissingInExit,

    #[error(transparent)]
    Tracer(#[from] ptrace_iterator::Error),

    #[error(transparent)]
    TracerCore(#[from] ptrace_iterator::core::Error),

    #[error(transparent)]
    Unwind(#[from] unwind::Error),
}
