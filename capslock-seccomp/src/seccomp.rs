use std::{collections::BTreeSet, fmt::Display};

use clap::Args;
use serde::Serialize;

use crate::error::Error;

#[derive(Args)]
pub struct ActionDef {
    #[arg(long, default_value = "SCMP_ACT_KILL_PROCESS")]
    default_action: String,

    #[arg(long)]
    default_action_errno: Option<i32>,

    #[arg(long)]
    default_action_trace: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Allow,
    Kill,
    KillProcess,
    Trap,
    Errno(i32),
    Trace(#[allow(dead_code)] u32),
}

impl Action {
    fn errno(&self) -> Option<i32> {
        if let Self::Errno(errno) = self {
            Some(*errno)
        } else {
            None
        }
    }

    #[allow(dead_code)]
    fn trace(&self) -> Option<u32> {
        if let Self::Trace(trace) = self {
            Some(*trace)
        } else {
            None
        }
    }
}

impl Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Action::Allow => "SCMP_ACT_ALLOW",
                Action::Kill => "SCMP_ACT_KILL",
                Action::KillProcess => "SCMP_ACT_KILL_PROCESS",
                Action::Trap => "SCMP_ACT_TRAP",
                Action::Errno(_) => "SCMP_ACT_ERRNO",
                Action::Trace(_) => "SCMP_ACT_TRACE",
            }
        )
    }
}

impl TryFrom<ActionDef> for Action {
    type Error = Error;

    fn try_from(value: ActionDef) -> Result<Self, Self::Error> {
        match value.default_action.as_str() {
            "SCMP_ACT_ALLOW" => Ok(Self::Allow),
            "SCMP_ACT_KILL" => Ok(Self::Kill),
            "SCMP_ACT_KILL_PROCESS" => Ok(Self::KillProcess),
            "SCMP_ACT_TRAP" => Ok(Self::Trap),
            "SCMP_ACT_ERRNO" => Ok(Self::Errno(
                value.default_action_errno.ok_or(Error::NoErrno)?,
            )),
            "SCMP_ACT_TRACE" => Ok(Self::Trace(
                value.default_action_trace.ok_or(Error::NoTrace)?,
            )),
            _ => Err(Error::ActionUnknown(value.default_action)),
        }
    }
}

#[derive(Debug)]
pub struct Policy {
    default_action: Action,
    architectures: Vec<String>,
    syscalls: Vec<Syscalls>,
}

impl Policy {
    pub fn new(default_action: Action) -> Self {
        Self {
            default_action,
            architectures: Vec::new(),
            syscalls: Vec::new(),
        }
    }

    pub fn add_architecture(&mut self, arch: impl ToString) {
        self.architectures.push(arch.to_string());
    }

    pub fn add_syscalls<I>(&mut self, action: Action, names: I)
    where
        I: Iterator,
        I::Item: ToString,
    {
        self.syscalls.push(Syscalls {
            names: names.map(|name| name.to_string()).collect(),
            action,
        })
    }
}

impl Serialize for Policy {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Raw<'a> {
            default_action: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            default_errno_ret: Option<i32>,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            architectures: &'a Vec<String>,
            syscalls: &'a [Syscalls],
        }

        Raw {
            default_action: self.default_action.to_string(),
            default_errno_ret: self.default_action.errno(),
            architectures: &self.architectures,
            syscalls: &self.syscalls,
        }
        .serialize(serializer)
    }
}

#[derive(Debug)]
struct Syscalls {
    names: BTreeSet<String>,
    action: Action,
}

impl Serialize for Syscalls {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Raw<'a> {
            names: &'a BTreeSet<String>,
            action: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            errno_ret: Option<i32>,
        }

        Raw {
            names: &self.names,
            action: self.action.to_string(),
            errno_ret: self.action.errno(),
        }
        .serialize(serializer)
    }
}
