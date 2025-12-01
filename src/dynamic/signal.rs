use ptrace_iterator::{
    Piddable,
    nix::{self, unistd::Pid},
};
use signal_hook::{
    consts::*,
    iterator::{Handle, Signals},
};

pub struct SignalForwarder {
    handle: Handle,
}

impl SignalForwarder {
    pub fn spawn(pid: impl Piddable) -> anyhow::Result<Self> {
        let signals = Signals::new([SIGINT])?;
        let handle = signals.handle();
        let pid = pid.into_pid();

        std::thread::spawn(move || {
            if let Err(e) = Self::listen(signals, pid) {
                tracing::error!(%e, "error returned by signal forwarder");
            }
        });

        Ok(Self { handle })
    }

    fn listen(mut signals: Signals, pid: Pid) -> anyhow::Result<()> {
        for signal in signals.forever() {
            nix::sys::signal::kill(pid, Some(signal.try_into()?))?;
        }

        Ok(())
    }
}

impl Drop for SignalForwarder {
    fn drop(&mut self) {
        self.handle.close();
    }
}
