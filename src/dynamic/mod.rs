use std::{
    collections::{BTreeSet, HashMap, VecDeque},
    ffi::OsString,
    fs::File,
    io::Write,
    path::PathBuf,
    process::Command,
};

use capslock::{
    Capability, CapabilityType,
    report::{self},
};
use clap::Parser;
use ptrace_iterator::{CommandTrace, Tracer, event::Event};
use symbolic::common::Name;
use unwind::{Accessors, AddressSpace, Byteorder, Cursor, PTraceState, RegNum};

use crate::{
    dynamic::signal::SignalForwarder,
    function::{FunctionMap, ToFunction},
    graph::CallGraph,
};

mod location;
mod signal;

#[derive(Parser, Debug)]
pub struct Dynamic {
    #[arg(short, long)]
    lookup_locations: bool,

    #[arg(short, long)]
    output: Option<PathBuf>,

    #[arg(num_args=1..)]
    argv: Vec<OsString>,
}

impl Dynamic {
    #[tracing::instrument(err)]
    pub fn main(self) -> anyhow::Result<()> {
        // Wrangle argv and extract the command path.
        let mut argv = self.argv.into_iter().collect::<VecDeque<_>>();
        let path = argv
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("cannot get argv[0]"))?;

        // Spawn the command we're going to trace.
        let mut cmd = Command::new(&path);
        cmd.args(argv).traceme();
        let child = cmd.spawn()?;

        // Set up signal handling to pass signals on to the child.
        let signal_forwarder = SignalForwarder::spawn(child.id())?;

        // Set up our location lookup service based on the command line flags.
        let mut location_lookup = if self.lookup_locations {
            location::Lookup::enabled()
        } else {
            location::Lookup::disabled()
        };

        // Initialise the process state. For now we'll lump all the descendant processes into one
        // state structure, but if we ever wanted to split them out for more fine-grained reporting,
        // that wouldn't be difficult.
        let mut process_state = ProcessState::default();

        // To take advantage of libunwind caching, we'll only construct one address space per
        // spawned process. We'll add these lazily, though, so we don't have to track clones
        // explicitly.
        let mut address_spaces = HashMap::new();

        // Actually start tracing the child.
        let mut tracer = Tracer::<()>::new(child)?;
        for event_result in tracer.iter() {
            let event = match event_result {
                Ok(event) => event,
                Err(e) => {
                    tracing::error!(%e, "tracer error");
                    continue;
                }
            };

            // We're only interested in syscall exits right now, since we can check if there's an
            // error.
            //
            // If and when there's more fine-grained introspection into syscalls (for example, to
            // ascertain what an `ioctl` syscall is actually doing), then we'll likely also need to
            // track entries so we can examine arguments. But this is sufficient for now.
            if let Event::SyscallExit(event) = &event
                && !event.is_error()
            {
                let pid = event.pid();
                let Some(syscall) = event.syscall() else {
                    continue;
                };

                // Even if we can't get a stack trace, let's minimally update the overall set of
                // capabilities.
                let syscall_caps = match crate::syscall::lookup(syscall.nr().name()) {
                    Some(iter) => iter.collect::<BTreeSet<_>>(),
                    None => {
                        tracing::warn!(?syscall, "cannot find syscall in syscall capability map");
                        continue;
                    }
                };
                process_state.caps.extend(syscall_caps.iter().copied());

                // Configure libunwind to use ptrace to access the child's memory space.
                let state = PTraceState::new(pid.as_raw() as u32)?;
                let address_space = address_spaces.entry(pid).or_insert_with(|| {
                    AddressSpace::new(Accessors::ptrace(), Byteorder::DEFAULT).unwrap()
                });
                let Ok(mut cursor) = Cursor::remote(address_space, &state) else {
                    continue;
                };

                // Now we iterate over the call stack. Note that we have to track the previous child
                // function as well to build the call graph.
                let mut child_idx = None;
                loop {
                    let Ok(ip) = cursor.register(RegNum::IP) else {
                        break;
                    };

                    // We're only interested in stack frames that have symbol names.
                    if let Ok(name) = cursor.procedure_name()
                        && let Ok(info) = cursor.procedure_info()
                        && ip == info.start_ip() + name.offset()
                    {
                        // If this is the first named stack frame we've seen, then we'll consider
                        // any capabilities here to be direct. Anything higher in the stack will be
                        // considered transitive.
                        let ty = if child_idx.is_none() {
                            CapabilityType::Direct
                        } else {
                            CapabilityType::Transitive
                        };

                        let name = Name::from(name.name());
                        match name.to_function_with_caps(syscall_caps.iter().map(|cap| (*cap, ty)))
                        {
                            Ok(mut func) => {
                                // Do the location lookup, bearing in mind that it might be a no-op
                                // if this is disabled.
                                func.location = location_lookup.lookup(pid, name.as_str()).cloned();

                                // Ensure the function is known and get its index for the call
                                // graph.
                                let func_idx = process_state.functions.upsert(name.as_str(), func);

                                // Actually update the call graph as long as this isn't the first
                                // frame.
                                if let Some(child_idx) = child_idx {
                                    process_state.call_graph.add_edge(func_idx, child_idx, None);
                                }

                                // Update the last frame we saw.
                                child_idx = Some(func_idx);
                            }
                            Err(e) => {
                                tracing::error!(%e, ?name, "error parsing function name");
                            }
                        }
                    }

                    // On to the next stack frame!
                    match cursor.step() {
                        Ok(true) => continue,
                        Ok(false) | Err(_) => break,
                    }
                }
            }
        }

        // Stop forwarding signals, since there's no longer a child process.
        drop(signal_forwarder);

        // Output the Capslock JSON.
        let mut writer: Box<dyn Write> = if let Some(output) = self.output {
            eprintln!("Writing capslock JSON to {}", output.display());
            Box::new(File::create(output)?)
        } else {
            Box::new(std::io::stdout())
        };
        serde_json::to_writer_pretty(&mut writer, &process_state.into_report(path))?;

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

#[derive(Debug, Default)]
struct ProcessState {
    call_graph: CallGraph,
    caps: BTreeSet<Capability>,
    functions: FunctionMap,
}

impl ProcessState {
    fn into_report(self, path: impl Into<PathBuf>) -> report::Report {
        report::Report {
            path: path.into(),
            capabilities: self.caps,
            functions: self.functions.into_functions(),
            edges: self.call_graph.into(),
        }
    }
}
