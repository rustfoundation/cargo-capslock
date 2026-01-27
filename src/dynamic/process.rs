use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    ffi::OsString,
    ops::RangeInclusive,
    path::{Path, PathBuf},
};

use capslock::{Capability, report};
use nix::unistd::Pid;
use ptrace_iterator::core::Fd;

use crate::{
    dynamic::{error::Error, fd},
    function::FunctionMap,
    graph::CallGraph,
};

#[derive(Debug)]
pub struct Map {
    active: BTreeMap<Pid, State>,
    inactive: Vec<(Pid, State)>,

    include_before_start: bool,
    init_pid: Pid,
}

impl Map {
    pub fn new(
        init_pid: Pid,
        init_exec: Exec,
        init_wd: impl Into<PathBuf>,
        include_before_start: bool,
    ) -> Self {
        Self {
            active: [(
                init_pid,
                State {
                    execs: [init_exec].into_iter().collect(),
                    fds: Default::default(),
                    pid: init_pid,
                    waiting_for_start: !include_before_start,
                    wd: init_wd.into(),
                    call_graph: Default::default(),
                    caps: Default::default(),
                    functions: Default::default(),
                },
            )]
            .into_iter()
            .collect(),
            inactive: Vec::new(),
            include_before_start,
            init_pid,
        }
    }

    #[tracing::instrument(level = "TRACE", skip(self))]
    pub fn exit(&mut self, pid: Pid) {
        if pid != self.init_pid
            && let Some(state) = self.active.remove(&pid)
        {
            self.inactive.push((pid, state));
        }
    }

    pub fn get_active(&self, pid: Pid) -> Option<&State> {
        self.active.get(&pid)
    }

    pub fn get_mut_active(&mut self, pid: Pid) -> Option<&mut State> {
        self.active.get_mut(&pid)
    }

    #[tracing::instrument(level = "TRACE", skip(self), err)]
    pub fn spawn(&mut self, parent: Pid, child: Pid) -> Result<(), Error> {
        let parent = self.get_active(parent).ok_or(Error::ProcessFind(parent))?;

        self.active.insert(
            child,
            State {
                execs: Default::default(),
                fds: parent
                    .fds
                    .iter()
                    .filter(|(_, meta)| !meta.is_cloexec())
                    .map(|(fd, meta)| (*fd, meta.clone()))
                    .collect(),
                pid: child,
                waiting_for_start: !self.include_before_start,
                wd: parent.wd.clone(),
                call_graph: Default::default(),
                caps: Default::default(),
                functions: Default::default(),
            },
        );

        Ok(())
    }

    pub fn into_report(mut self, include_children: bool) -> Result<report::Report, Error> {
        let mut processes = vec![
            self.active
                .remove(&self.init_pid)
                .ok_or(Error::ChildMissing(self.init_pid))?
                .into_process(),
        ];

        if include_children {
            processes.extend(
                self.active
                    .into_values()
                    .chain(self.inactive.into_iter().map(|(_, state)| state))
                    .map(|state| state.into_process()),
            );
        }

        // Build the final report.
        Ok(report::Report { processes })
    }
}

#[derive(Debug)]
pub struct State {
    execs: VecDeque<Exec>,
    fds: BTreeMap<Fd, fd::Meta>,
    pid: Pid,
    waiting_for_start: bool,
    wd: PathBuf,

    call_graph: CallGraph,
    caps: BTreeSet<Capability>,
    functions: FunctionMap,
}

impl State {
    pub fn add_edge(&mut self, from: usize, to: usize) {
        self.call_graph.add_edge(from, to, None);
    }

    pub fn add_exec(&mut self, exec: Exec) {
        self.execs.push_back(exec);
    }

    pub fn close(&mut self, fd: Fd) {
        self.fds.remove(&fd);
    }

    pub fn close_range(&mut self, range: RangeInclusive<Fd>) {
        self.fds.retain(|fd, _| !range.contains(fd));
    }

    pub fn extend_caps(&mut self, caps: impl Iterator<Item = Capability>) {
        self.caps.extend(caps);
    }

    pub fn get_fd(&self, fd: Fd) -> Option<&fd::Meta> {
        self.fds.get(&fd)
    }

    pub fn infer_fd(&mut self, fd: Fd) -> Result<&fd::Meta, Error> {
        self.fds
            .insert(fd, fd::Meta::try_from_procfs(self.pid, fd)?);

        // unwrap() used here because we literally just inserted the entry.
        Ok(self.fds.get(&fd).unwrap())
    }

    pub fn insert_fd(&mut self, fd: Fd, meta: fd::Meta) {
        self.fds.insert(fd, meta);
    }

    pub fn is_waiting_for_start(&self) -> bool {
        self.waiting_for_start
    }

    pub fn pid(&self) -> Pid {
        self.pid
    }

    pub fn resolve(&self, path: impl AsRef<Path>) -> PathBuf {
        self.wd.join(path.as_ref())
    }

    pub fn set_working_directory(&mut self, path: impl Into<PathBuf>) {
        self.wd = path.into();
    }

    pub fn start_seen(&mut self) {
        self.waiting_for_start = false;
    }

    pub fn upsert_function(&mut self, mangled: &str, function: report::Function) -> usize {
        self.functions.upsert(mangled, function)
    }

    pub fn into_process(mut self) -> report::Process {
        report::Process {
            path: if let Some(exec) = self.execs.pop_front() {
                exec.command.into()
            } else {
                PathBuf::new()
            },
            capabilities: self.caps,
            functions: self.functions.into_functions(),
            edges: self.call_graph.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Exec {
    command: OsString,
    #[allow(unused)]
    argv: Vec<OsString>,
    #[allow(unused)]
    envp: Vec<OsString>,
}

impl Exec {
    pub fn new<Command, Argv, Envp>(command: Command, argv: Argv, envp: Envp) -> Self
    where
        Command: Into<OsString>,
        Argv: Iterator,
        Argv::Item: Into<OsString>,
        Envp: Iterator,
        Envp::Item: Into<OsString>,
    {
        Self {
            command: command.into(),
            argv: argv.map(|arg| arg.into()).collect(),
            envp: envp.map(|env| env.into()).collect(),
        }
    }
}
