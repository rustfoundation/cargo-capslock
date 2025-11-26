use std::{
    collections::{BTreeSet, HashMap, VecDeque},
    ffi::{OsStr, OsString},
    fs::File,
    io::Write,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    process::Command,
};

use capslock::{
    Capability, CapabilityType,
    report::{self, Location},
};
use clap::Parser;
use ptrace_iterator::{CommandTrace, Piddable, Tracer, event::Event, nix::unistd::Pid};
use symbolic::{common::Name, debuginfo::Object};
use unwind::{Accessors, AddressSpace, Byteorder, Cursor, PTraceState, RegNum};

use crate::{
    caps::FunctionCaps,
    function::{FunctionMap, ToFunction},
    graph::CallGraph,
};

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
        let mut argv = self.argv.into_iter().collect::<VecDeque<_>>();
        let path = argv
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("cannot get argv[0]"))?;

        let mut cmd = Command::new(&path);
        cmd.args(argv).traceme();
        let child = cmd.spawn()?;

        let empty_caps = FunctionCaps::default();
        let mut location_lookup = if self.lookup_locations {
            LocationLookup::enabled()
        } else {
            LocationLookup::disabled()
        };
        let mut process_state = ProcessState::default();

        let mut tracer = Tracer::new(child)?;
        for event_result in tracer.by_ref() {
            let event = match event_result {
                Ok(event) => event,
                Err(e) => {
                    tracing::error!(%e, "tracer error");
                    continue;
                }
            };

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

                let state = PTraceState::new(pid.as_raw() as u32)?;
                let address_space = AddressSpace::new(Accessors::ptrace(), Byteorder::DEFAULT)?;
                let Ok(mut cursor) = Cursor::remote(&address_space, &state) else {
                    continue;
                };

                let mut child_idx = None;
                loop {
                    let Ok(ip) = cursor.register(RegNum::IP) else {
                        break;
                    };

                    if let Ok(name) = cursor.procedure_name()
                        && let Ok(info) = cursor.procedure_info()
                        && ip == info.start_ip() + name.offset()
                    {
                        let ty = if child_idx.is_none() {
                            CapabilityType::Direct
                        } else {
                            CapabilityType::Transitive
                        };

                        let name = Name::from(name.name());
                        let mut func = name.to_function(&empty_caps)?;
                        for cap in syscall_caps.iter() {
                            func.insert_capability(*cap, ty);
                        }

                        func.location = location_lookup.lookup(pid, name.as_str()).cloned();

                        let func_idx = process_state.functions.upsert(name.as_str(), func);

                        if let Some(child_idx) = child_idx {
                            process_state.call_graph.add_edge(func_idx, child_idx, None);
                        }

                        child_idx = Some(func_idx);
                    }

                    match cursor.step() {
                        Ok(true) => continue,
                        Ok(false) | Err(_) => break,
                    }
                }
            }
        }

        let mut writer: Box<dyn Write> = if let Some(output) = self.output {
            eprintln!("Writing capslock JSON to {}", output.display());
            Box::new(File::create(output)?)
        } else {
            Box::new(std::io::stdout())
        };
        serde_json::to_writer_pretty(&mut writer, &process_state.into_report(path))?;

        if let Some(status) = tracer.status()
            && let Some(code) = status.code()
        {
            std::process::exit(code);
        } else {
            Ok(())
        }
    }
}

struct LocationLookup {
    processes: Option<HashMap<Pid, Option<ProcessLookup>>>,
}

impl LocationLookup {
    pub fn disabled() -> Self {
        Self { processes: None }
    }

    pub fn enabled() -> Self {
        Self {
            processes: Some(HashMap::new()),
        }
    }

    pub fn lookup(&mut self, pid: impl Piddable, mangled: &str) -> Option<&Location> {
        if let Some(proc) = self.process(pid) {
            proc.lookup(mangled)
        } else {
            None
        }
    }

    fn process(&mut self, pid: impl Piddable) -> Option<&mut ProcessLookup> {
        if let Some(processes) = &mut self.processes {
            let pid = pid.into_pid();

            processes
                .entry(pid)
                .or_insert_with(|| match ProcessLookup::build(pid) {
                    Ok(proc) => Some(proc),
                    Err(e) => {
                        tracing::warn!(%e, %pid, "error building process lookup struct");
                        None
                    }
                });

            processes.get_mut(&pid).unwrap().as_mut()
        } else {
            None
        }
    }
}

struct ProcessLookup {
    functions: HashMap<String, Location>,
}

impl ProcessLookup {
    fn build(pid: Pid) -> anyhow::Result<Self> {
        let data = std::fs::read(format!("/proc/{pid}/exe"))?;
        let object = Object::parse(&data)?;
        let debug = object.debug_session()?;

        let mut functions = HashMap::new();

        for func in debug.functions() {
            let Ok(func) = func else {
                continue;
            };

            if let Some(info) = func.lines.first() {
                let path =
                    Path::new(OsStr::from_bytes(func.compilation_dir)).join(info.file.path_str());

                functions.insert(
                    func.name.to_string(),
                    Location {
                        directory: path.parent().map(PathBuf::from),
                        filename: path
                            .file_name()
                            .map(PathBuf::from)
                            .unwrap_or(PathBuf::from("..")),
                        line: info.line,
                        column: None,
                    },
                );
            }
        }

        Ok(Self { functions })
    }

    fn lookup(&self, mangled: &str) -> Option<&Location> {
        self.functions.get(mangled)
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
