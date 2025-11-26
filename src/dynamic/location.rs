use std::{
    collections::HashMap,
    ffi::OsStr,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use capslock::report::Location;
use nix::unistd::Pid;
use ptrace_iterator::Piddable;
use symbolic::debuginfo::Object;

#[derive(Debug)]
pub struct Lookup {
    processes: Option<HashMap<Pid, Option<Process>>>,
}

impl Lookup {
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

    fn process(&mut self, pid: impl Piddable) -> Option<&mut Process> {
        if let Some(processes) = &mut self.processes {
            let pid = pid.into_pid();

            processes
                .entry(pid)
                .or_insert_with(|| match Process::build(pid) {
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

#[derive(Debug)]
struct Process {
    functions: HashMap<String, Location>,
}

impl Process {
    fn build(pid: Pid) -> anyhow::Result<Self> {
        // We're going to read the functions and their locations out of the debuginfo in the PID's
        // executable. It's easier to simply persist them once than to keep a debug session around
        // because of how symbolic's lifetimes work.
        //
        // TODO: the obvious problem here is shared libraries, which we could get through
        // /proc/{pid}/maps, but requires more work.
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
