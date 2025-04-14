use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{bash, Error};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Serialize, Deserialize)]
pub enum ExecStatus {
    Success,
    Failure(i32),
}

impl fmt::Display for ExecStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let ret: i32 = (*self).into();
        write!(f, "{ret}")
    }
}

impl From<ExecStatus> for i32 {
    fn from(exec: ExecStatus) -> i32 {
        match exec {
            ExecStatus::Success => bash::EXECUTION_SUCCESS as i32,
            ExecStatus::Failure(n) => n,
        }
    }
}

impl From<Error> for ExecStatus {
    fn from(e: Error) -> ExecStatus {
        match e {
            Error::Bail(_) => ExecStatus::Failure(bash::EX_LONGJMP as i32),
            Error::Status(s) => s,
            _ => ExecStatus::Failure(1),
        }
    }
}

impl From<i32> for ExecStatus {
    fn from(ret: i32) -> ExecStatus {
        match ret {
            0 => ExecStatus::Success,
            n => ExecStatus::Failure(n),
        }
    }
}

impl From<&ExecStatus> for bool {
    fn from(exec: &ExecStatus) -> bool {
        matches!(exec, ExecStatus::Success)
    }
}

impl From<ExecStatus> for bool {
    fn from(exec: ExecStatus) -> bool {
        matches!(exec, ExecStatus::Success)
    }
}

impl From<bool> for ExecStatus {
    fn from(value: bool) -> ExecStatus {
        if value {
            ExecStatus::Success
        } else {
            ExecStatus::Failure(1)
        }
    }
}
