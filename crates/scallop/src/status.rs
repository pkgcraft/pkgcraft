use serde::{Deserialize, Serialize};

use crate::{Error, bash};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Serialize, Deserialize)]
pub enum ExecStatus {
    Success,
    Failure(i32),
}

impl From<Error> for ExecStatus {
    fn from(e: Error) -> ExecStatus {
        match e {
            Error::Bail(_) => ExecStatus::Failure(bash::EX_LONGJMP as i32),
            Error::Status(n) => ExecStatus::Failure(n),
            _ => ExecStatus::Failure(1),
        }
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

impl From<i32> for ExecStatus {
    fn from(ret: i32) -> ExecStatus {
        match ret {
            0 => ExecStatus::Success,
            n => ExecStatus::Failure(n),
        }
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

impl From<ExecStatus> for bool {
    fn from(exec: ExecStatus) -> bool {
        matches!(exec, ExecStatus::Success)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symmetric_conversion() {
        // i32
        for expected in [0, 1, 123] {
            let status: ExecStatus = expected.into();
            assert_eq!(expected, status.into());
        }

        // bool
        for expected in [true, false] {
            let status: ExecStatus = expected.into();
            assert_eq!(expected, status.into());
        }
    }
}
