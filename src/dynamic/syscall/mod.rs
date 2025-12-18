use std::{
    collections::BTreeSet,
    ffi::{OsString, c_ulong},
    ops::RangeInclusive,
    path::PathBuf,
};

use capslock::Capability;
use itertools::Itertools;
use nix::{
    fcntl::OFlag,
    sys::socket::{AddressFamily, SockType},
};
use ptrace_iterator::{
    Syscall, Sysno,
    core::{Fd, TryFromArg},
};

use crate::{
    dynamic::{error::Error, fd, process},
    syscall::lookup,
};

mod ioctl;

#[derive(Debug, Clone)]
pub struct Meta {
    nr: Sysno,
    typed: Option<Typed>,
}

#[derive(Debug, Clone)]
enum Typed {
    Chdir {
        path: PathBuf,
    },
    Close {
        fd: Fd,
    },
    CloseRange {
        range: RangeInclusive<Fd>,
    },
    Exec {
        path: PathBuf,
        argv: Vec<OsString>,
        envp: Vec<OsString>,
    },
    FdCreate {
        meta: fd::Meta,
    },
    Ioctl {
        cmd: c_ulong,
        fd: Fd,
    },
}

impl Meta {
    #[tracing::instrument(level="TRACE", skip(state), err, fields(pid = %state.pid()))]
    pub fn try_from_syscall(state: &mut process::State, syscall: &Syscall) -> Result<Self, Error> {
        use nix::libc::{
            SOCK_CLOEXEC, SOCK_DGRAM, SOCK_RAW, SOCK_RDM, SOCK_SEQPACKET, SOCK_STREAM,
        };

        let pid = state.pid();

        Ok(Self {
            nr: syscall.nr(),
            typed: match syscall {
                Syscall::Chdir(args) => Some(Typed::Chdir {
                    path: unsafe { args.filename(pid) }?,
                }),
                Syscall::Close(args) => Some(Typed::Close { fd: args.fd() }),
                Syscall::CloseRange(args) => Some(Typed::CloseRange {
                    range: args.fd()..=args.max_fd(),
                }),
                Syscall::Open(args) => Some(Typed::FdCreate {
                    meta: fd::Meta::new(
                        OFlag::from_bits_retain(args.flags()),
                        fd::Type::File {
                            path: unsafe { args.filename(pid) }?,
                        },
                    ),
                }),
                Syscall::Openat(args) => Some(Typed::FdCreate {
                    meta: fd::Meta::new(
                        OFlag::from_bits_retain(args.flags()),
                        fd::Type::File {
                            path: resolve_at_syscall(state, args.dfd(), unsafe {
                                args.filename(pid)
                            }?)?,
                        },
                    ),
                }),
                Syscall::Openat2(args) => Some(Typed::FdCreate {
                    meta: fd::Meta::new(
                        OFlag::from_bits_retain(unsafe { args.how(pid) }?.flags as i32),
                        fd::Type::File {
                            path: resolve_at_syscall(state, args.dfd(), unsafe {
                                args.filename(pid)
                            }?)?,
                        },
                    ),
                }),
                Syscall::Pipe(_) => Some(Typed::FdCreate {
                    meta: fd::Meta::new(OFlag::empty(), fd::Type::Fifo),
                }),
                Syscall::Pipe2(args) => Some(Typed::FdCreate {
                    meta: fd::Meta::new(OFlag::from_bits_retain(args.flags()), fd::Type::Fifo),
                }),
                Syscall::Socket(args) => Some(Typed::FdCreate {
                    meta: fd::Meta::new(
                        if args.r#type() & SOCK_CLOEXEC == SOCK_CLOEXEC {
                            OFlag::O_CLOEXEC
                        } else {
                            OFlag::empty()
                        },
                        fd::Type::Socket {
                            domain: AddressFamily::from_i32(args.family())
                                .unwrap_or(AddressFamily::Unspec),
                            ty: match args.r#type() {
                                t if t & SOCK_STREAM == SOCK_STREAM => SockType::Stream,
                                t if t & SOCK_DGRAM == SOCK_DGRAM => SockType::Datagram,
                                t if t & SOCK_SEQPACKET == SOCK_SEQPACKET => SockType::SeqPacket,
                                t if t & SOCK_RAW == SOCK_RAW => SockType::Raw,
                                t if t & SOCK_RDM == SOCK_RDM => SockType::Rdm,
                                t => return Err(Error::SocketTypeUnknown(t)),
                            },
                        },
                    ),
                }),
                Syscall::Ioctl(args) => Some(Typed::Ioctl {
                    cmd: args.cmd() as c_ulong,
                    fd: args.fd(),
                }),
                Syscall::Execve(args) => Some(Typed::Exec {
                    path: unsafe { args.filename(pid) }?,
                    argv: unsafe { args.argv(pid) }.try_collect()?,
                    envp: unsafe { args.envp(pid) }.try_collect()?,
                }),
                Syscall::Execveat(args) => Some(Typed::Exec {
                    path: unsafe { args.filename(pid) }?,
                    argv: unsafe { args.argv(pid) }.try_collect()?,
                    envp: unsafe { args.envp(pid) }.try_collect()?,
                }),
                _ => None,
            },
        })
    }

    #[tracing::instrument(level="TRACE", skip(self, state), err, fields(pid = %state.pid()))]
    pub fn into_capabilities(
        self,
        state: &mut process::State,
        sval: i64,
    ) -> Result<BTreeSet<Capability>, Error> {
        let Self { nr, typed } = self;

        if let Some(typed) = typed {
            match typed {
                Typed::Chdir { path } => {
                    state.set_working_directory(path);
                }
                Typed::Close { fd } => {
                    state.close(fd);
                }
                Typed::CloseRange { range } => {
                    state.close_range(range);
                }
                Typed::Exec { path, argv, envp } => {
                    state.add_exec(process::Exec::new(
                        path.into_os_string(),
                        argv.into_iter(),
                        envp.into_iter(),
                    ));
                }
                Typed::FdCreate { meta } => {
                    // Update the process state, since we have the FD and metadata available.
                    state.insert_fd(
                        Fd::try_from_arg(sval as u64)
                            .map_err(|e| Error::FdParse { e, fd: sval as u64 })?,
                        meta,
                    );
                }
                Typed::Ioctl { cmd, fd } => {
                    if let Some(meta) = match state.get_fd(fd) {
                        Some(meta) => Some(meta),
                        None => match state.infer_fd(fd) {
                            Ok(meta) => Some(meta),
                            Err(e) => {
                                tracing::warn!(?e, %fd, pid = %state.pid(), "inferring FD");
                                None
                            }
                        },
                    } {
                        match ioctl::caps(cmd, meta.ty()) {
                            Ok(caps) => return Ok(caps),
                            Err(e) => {
                                tracing::warn!(?e, %fd, pid = %state.pid(), "resolving ioctl to capabilities");
                            }
                        }
                    }
                }
            }
        }

        lookup_sysno(nr)
    }
}

fn lookup_sysno(nr: Sysno) -> Result<BTreeSet<Capability>, Error> {
    lookup(nr.name())
        .ok_or_else(|| {
            tracing::warn!(?nr, "cannot find syscall in syscall capability map");
            Error::SyscallMissingFromMap(nr)
        })
        .map(|caps| caps.collect::<BTreeSet<_>>())
}

#[tracing::instrument(level="TRACE", skip(state), err, fields(pid = %state.pid()))]
fn resolve_at_syscall(
    state: &mut process::State,
    dfd: Fd,
    local: PathBuf,
) -> Result<PathBuf, Error> {
    if local.is_absolute() {
        Ok(local)
    } else if dfd.is_at_working_directory() {
        Ok(state.resolve(local))
    } else {
        let meta = match state.get_fd(dfd) {
            Some(meta) => meta,
            None => state.infer_fd(dfd)?,
        };

        match meta.ty() {
            fd::Type::Directory { path } => Ok(path.join(local)),
            fd::Type::File { path } => Ok(path.join(local)),
            _ => Err(Error::Resolve {
                fd: dfd,
                path: local,
                pid: state.pid(),
            }),
        }
    }
}
