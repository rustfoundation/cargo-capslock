use ptrace_iterator::{
    Piddable,
    nix::{self, unistd::Pid},
};
use signal_hook::{
    consts::*,
    iterator::{Handle, Signals},
};

use crate::runtime::error::Error;

pub struct SignalForwarder {
    handle: Handle,
}

impl SignalForwarder {
    pub fn spawn(pid: impl Piddable) -> Result<Self, Error> {
        let signals = Signals::new([SIGINT]).map_err(Error::Signals)?;
        let handle = signals.handle();
        let pid = pid.into_pid();

        std::thread::spawn(move || {
            if let Err(e) = Self::listen(signals, pid) {
                tracing::error!(%e, "error returned by signal forwarder");
            }
        });

        Ok(Self { handle })
    }

    #[tracing::instrument(level = "TRACE", skip(signals), err)]
    fn listen(mut signals: Signals, pid: Pid) -> Result<(), Error> {
        for signal in signals.forever() {
            nix::sys::signal::kill(pid, Some(signal.try_into().map_err(Error::Signal)?))
                .map_err(|e| Error::Kill { e, pid })?;
        }

        Ok(())
    }
}

impl Drop for SignalForwarder {
    fn drop(&mut self) {
        self.handle.close();
    }
}
