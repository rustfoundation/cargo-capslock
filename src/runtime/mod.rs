use std::{
    collections::{HashMap, VecDeque},
    ffi::OsString,
    fs::File,
    io::Write,
    path::PathBuf,
    process::Command,
};

use capslock::CapabilityType;
use clap::Parser;
use nix::unistd::Pid;
use ptrace_iterator::{
    CommandTrace, Piddable, Tracer,
    event::{Event, SyscallEntry, SyscallExit},
};
use symbolic::common::Name;
use unwind::{Accessors, AddressSpace, Byteorder, Cursor, PTraceState, PTraceStateRef, RegNum};

use crate::{
    function::ToFunction,
    runtime::{error::Error, location::Lookup, signal::SignalForwarder, syscall::Meta},
};

mod error;
mod fd;
mod location;
mod process;
mod signal;
mod syscall;

#[derive(Parser, Debug)]
pub struct Runtime {
    /// If enabled, an additional section will be added to the JSON output providing information for
    /// child processes.
    #[arg(short = 'c', long)]
    include_children: bool,

    /// If enabled, functions before `_start` will also be included in the output.
    #[arg(long)]
    include_before_start: bool,

    /// If enabled, the actual syscalls invoked will be included in the output.
    #[arg(long)]
    include_syscalls: bool,

    /// If enabled, source file locations will be looked up via debuginfo.
    ///
    /// This tends to have a significant performance impact.
    #[arg(short, long)]
    lookup_locations: bool,

    /// If provided, the file to write the JSON output to.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// The command and any arguments to analyse.
    #[arg(num_args=1..)]
    argv: Vec<OsString>,
}

impl Runtime {
    #[tracing::instrument(err)]
    pub fn main(self) -> Result<(), Error> {
        // Wrangle argv and extract the command path.
        let mut argv = self.argv.into_iter().collect::<VecDeque<_>>();
        let path = argv.pop_front().ok_or(Error::Argv0)?;

        // Spawn the command we're going to trace.
        let mut cmd = Command::new(&path);
        cmd.args(&argv).traceme();
        let child = cmd.spawn().map_err(Error::Spawn)?;
        let child_pid = child.id().into_pid();

        // Set up signal handling to pass signals on to the child.
        let signal_forwarder = SignalForwarder::spawn(child.id())?;

        let mut global_state = GlobalState::new(
            child_pid,
            process::Exec::new(path, argv.into_iter(), std::iter::empty::<OsString>()),
            std::env::current_dir().map_err(Error::Cwd)?,
            self.include_before_start,
            self.include_syscalls,
            self.lookup_locations,
        );

        // Actually start tracing the child.
        let mut tracer = Tracer::<Meta>::new(child)?;
        for event_result in tracer.iter() {
            let mut event = match event_result {
                Ok(event) => event,
                Err(e) => {
                    tracing::error!(%e, "tracer error");
                    continue;
                }
            };

            if let Err(e) = global_state.handle_event(&mut event) {
                tracing::debug!(%e, "error handling event");
            }
        }

        // Stop forwarding signals, since there's no longer a child process.
        drop(signal_forwarder);

        // Output the Capslock JSON.
        let mut writer: Box<dyn Write> = if let Some(output) = self.output {
            Box::new(File::create(&output).map_err(|e| Error::OutputCreate {
                e,
                path: output.to_path_buf(),
            })?)
        } else {
            Box::new(std::io::stdout())
        };
        serde_json::to_writer_pretty(
            &mut writer,
            &global_state.processes.into_report(self.include_children)?,
        )?;

        // Do our best to forward on the child's exit status.
        if let Some(status) = tracer.status()
            && let Some(code) = status.code()
        {
            std::process::exit(code);
        } else {
            Ok(())
        }
    }
}

/// Global state while analysing a tree of running processes.
struct GlobalState {
    processes: process::Map,

    /// To take advantage of libunwind caching, we'll only construct one address space per spawned
    /// process. We'll add these lazily, though, so we don't have to track clones explicitly.
    address_spaces: HashMap<Pid, AddressSpace<PTraceStateRef>>,

    include_syscalls: bool,
    location_lookup: Lookup,
}

impl GlobalState {
    fn new(
        pid: Pid,
        exec: process::Exec,
        wd: PathBuf,
        include_before_start: bool,
        include_syscalls: bool,
        lookup_locations: bool,
    ) -> Self {
        Self {
            processes: process::Map::new(pid, exec, wd, include_before_start),
            address_spaces: HashMap::new(),
            include_syscalls,
            location_lookup: if lookup_locations {
                location::Lookup::enabled()
            } else {
                location::Lookup::disabled()
            },
        }
    }

    #[tracing::instrument(level = "TRACE", skip(self), err)]
    fn handle_event(&mut self, event: &mut Event<Meta>) -> Result<(), Error> {
        match event {
            Event::Clone(event) => self.processes.spawn(event.pid(), event.child_pid()),
            Event::Exited(event) => {
                self.processes.exit(event.pid());
                Ok(())
            }
            Event::SyscallEntry(event) => self.handle_syscall_entry(event),
            Event::SyscallExit(event) if !event.is_error() => self.handle_syscall_exit(event),
            _ => Ok(()),
        }
    }

    #[tracing::instrument(
        level = "TRACE",
        skip_all,
        err,
        fields(
            pid = %event.pid(),
            syscall = event.syscall().nr().name(),
        ),
    )]
    fn handle_syscall_entry(&mut self, event: &mut SyscallEntry<Meta>) -> Result<(), Error> {
        let state = self
            .processes
            .get_mut_active(event.pid())
            .ok_or_else(|| Error::ProcessUnknown(event.pid()))?;

        event.set_userdata(Meta::try_from_syscall(state, event.syscall())?);

        Ok(())
    }

    #[tracing::instrument(level = "TRACE", skip_all, err, fields(pid = %event.pid()))]
    fn handle_syscall_exit(&mut self, event: &mut SyscallExit<Meta>) -> Result<(), Error> {
        let pid = event.pid();
        let meta = event.take_userdata().ok_or(Error::SyscallMetaMissing)?;
        let process_state = self
            .processes
            .get_mut_active(pid)
            .ok_or(Error::ProcessUnknown(pid))?;

        // Even if we can't get a stack trace, let's minimally update the overall set of
        // capabilities.
        let syscall_caps = meta.into_capabilities(process_state, event.sval())?;
        process_state.extend_caps(syscall_caps.iter().copied());

        // Configure libunwind to use ptrace to access the child's memory space.
        let state = PTraceState::new(pid.as_raw() as u32)?;
        let address_space = self
            .address_spaces
            .entry(pid)
            .or_insert_with(|| AddressSpace::new(Accessors::ptrace(), Byteorder::DEFAULT).unwrap());
        let mut cursor = Cursor::remote(address_space, &state)?;

        let mut names = Vec::new();
        loop {
            let Ok(ip) = cursor.register(RegNum::IP) else {
                return Ok(());
            };

            if let Ok(name) = cursor.procedure_name()
                && let Ok(info) = cursor.procedure_info()
                && ip == info.start_ip() + name.offset()
            {
                if name.name() == "_start" {
                    process_state.start_seen();
                }

                names.push(name);
            }

            // On to the next stack frame!
            match cursor.step() {
                Ok(true) => continue,
                Ok(false) | Err(_) => break,
            }
        }

        if !process_state.is_waiting_for_start() {
            let mut child_idx = None;

            for name in names.into_iter() {
                // If this is the first named stack frame we've seen, then we'll consider any
                // capabilities here to be direct. Anything higher in the stack will be considered
                // transitive.
                let ty = if child_idx.is_none() {
                    CapabilityType::Direct
                } else {
                    CapabilityType::Transitive
                };

                let name = Name::from(name.name());
                match name.to_function_with_caps(syscall_caps.iter().map(|cap| (*cap, ty))) {
                    Ok(mut func) => {
                        // Add syscall if this is a direct syscall.
                        if self.include_syscalls
                            && ty == CapabilityType::Direct
                            && let Some(syscall) = event.syscall()
                        {
                            func.insert_syscall(syscall.nr());
                        }

                        // Do the location lookup, bearing in mind that it might be a no-op if this
                        // is disabled.
                        func.location = self.location_lookup.lookup(pid, name.as_str()).cloned();

                        // Ensure the function is known and get its index for the call graph.
                        let func_idx = process_state.upsert_function(name.as_str(), func);

                        // Actually update the call graph as long as this isn't the first frame.
                        if let Some(child_idx) = child_idx {
                            process_state.add_edge(func_idx, child_idx);
                        }

                        // Update the last frame we saw.
                        child_idx = Some(func_idx);
                    }
                    Err(e) => {
                        tracing::error!(%e, ?name, "error parsing function name");
                    }
                }
            }
        }

        Ok(())
    }
}
