use std::{collections::BTreeSet, ffi::c_ulong};

use capslock::Capability;
use nix::sys::socket::AddressFamily;

use crate::runtime::{error::Error, fd};

pub fn caps(cmd: c_ulong, ty: &fd::Type) -> Result<BTreeSet<Capability>, Error> {
    tracing::info!(?cmd, ?ty, "ioctl caps");

    // This is definitely overly simplistic right now, but it's a reasonable starting point.
    Ok(match (cmd, ty) {
        (_, fd::Type::Char { .. }) => [Capability::Safe].into_iter().collect(),
        (_, fd::Type::Directory { .. }) | (_, fd::Type::File { .. }) => {
            [Capability::Files].into_iter().collect()
        }
        (_, fd::Type::Socket { domain, .. }) if domain == &AddressFamily::Unix => {
            [Capability::Files].into_iter().collect()
        }
        (_, fd::Type::Socket { .. }) | (_, fd::Type::SocketInode { .. }) => {
            [Capability::Network].into_iter().collect()
        }
        _ => {
            return Err(Error::Ioctl {
                cmd,
                ty: ty.clone(),
            });
        }
    })
}
