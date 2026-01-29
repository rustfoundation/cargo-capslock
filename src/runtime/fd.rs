use std::{
    fmt::Debug,
    fs::File,
    io::{BufRead, BufReader},
    os::unix::{ffi::OsStrExt, fs::FileTypeExt},
    path::{Path, PathBuf},
};

use nix::{
    fcntl::OFlag,
    libc::{c_int, c_ulong},
    sys::socket::{AddressFamily, SockType},
    unistd::Pid,
};
use ptrace_iterator::core::Fd;

use crate::runtime::error::Error;

#[derive(Debug, Clone)]
pub struct Meta {
    flags: OFlag,
    ty: Type,
}

impl Meta {
    pub fn new(flags: OFlag, ty: Type) -> Self {
        Self { flags, ty }
    }

    #[tracing::instrument(level = "DEBUG", err)]
    pub fn try_from_procfs(pid: Pid, fd: Fd) -> Result<Self, Error> {
        let flags = OFlag::from_bits_retain(
            pid_fd_flags(pid, fd)?.ok_or(Error::ProcfsFdinfoMissing { fd, pid })?,
        );

        let link = std::fs::read_link(format!("/proc/{pid}/fd/{fd}"))
            .map_err(|e| Error::ProcfsFd { e, fd, pid })?;
        let ty = Type::try_from_procfs(link);

        Ok(Self { flags, ty })
    }

    pub fn is_cloexec(&self) -> bool {
        self.flags.contains(OFlag::O_CLOEXEC)
    }

    pub fn ty(&self) -> &Type {
        &self.ty
    }
}

#[tracing::instrument(level = "TRACE", err)]
fn pid_fd_flags(pid: Pid, fd: Fd) -> Result<Option<c_int>, Error> {
    for line_result in BufReader::new(
        File::open(format!("/proc/{pid}/fdinfo/{fd}")).map_err(|e| Error::ProcfsFdinfo {
            e,
            fd,
            pid,
        })?,
    )
    .split(b'\n')
    {
        let line = line_result.map_err(|e| Error::ProcfsFdinfo { e, fd, pid })?;

        if let Some(flags_bytes) = line.strip_prefix(b"flags:")
            && let Ok(flags_str) = std::str::from_utf8(flags_bytes)
        {
            return Ok(Some(
                i32::from_str_radix(flags_str.trim_ascii(), 8).map_err(|e| {
                    Error::ProcfsFdinfoFlag {
                        e,
                        fd,
                        flags: flags_str.to_string(),
                        pid,
                    }
                })?,
            ));
        }
    }

    Ok(None)
}

#[derive(Debug, Clone)]
pub enum Type {
    Block { path: PathBuf },
    Char { path: PathBuf },
    Directory { path: PathBuf },
    Fifo,
    File { path: PathBuf },
    Socket { domain: AddressFamily, ty: SockType },
    SocketInode { inode: c_ulong },
    Unknown,
}

impl Type {
    #[tracing::instrument(level = "TRACE")]
    pub fn try_from_procfs(path: impl AsRef<Path> + Debug) -> Self {
        let path = path.as_ref();

        // If this points to a real file on the filesystem, we'll go look at what type of file
        // _that_ is.
        if let Ok(path) = path.canonicalize()
            && let Ok(meta) = path.metadata()
        {
            let ty = meta.file_type();

            return if ty.is_dir() {
                Self::Directory { path }
            } else if ty.is_file() {
                Self::File { path }
            } else if ty.is_block_device() {
                Self::Block { path }
            } else if ty.is_char_device() {
                Self::Char { path }
            } else if ty.is_fifo() {
                Self::Fifo
            } else {
                // We shouldn't see anything else here, but whatever.
                Self::Unknown
            };
        }

        // We'll make a minimal effort to handle a couple of common types based on the link name.
        let path_bytes = path.as_os_str().as_bytes();
        if let Some(rem) = path_bytes.strip_prefix(b"socket:[")
            && let Some(inode_bytes) = rem.strip_suffix(b"]")
            && let Ok(inode_str) = std::str::from_utf8(inode_bytes)
            && let Ok(inode) = inode_str.parse()
        {
            return Self::SocketInode { inode };
        } else if path_bytes.starts_with(b"pipe:[") {
            return Self::Fifo;
        }

        // Otherwise, we'll just punt.
        Self::Unknown
    }
}
