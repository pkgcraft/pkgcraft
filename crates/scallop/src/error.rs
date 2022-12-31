use std::cell::RefCell;
use std::io;

use crate::builtins::ExecStatus;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Bail(String),
    #[error("{0}")]
    Base(String),
    #[error("{1}")]
    IO(io::ErrorKind, String),
    #[error("{1}")]
    Status(ExecStatus, String),
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::IO(e.kind(), e.to_string())
    }
}

thread_local! {
    pub(crate) static LAST_ERROR: RefCell<Option<Error>> = RefCell::new(None);
}

/// Retrieve the most recent bash error.
pub fn last_error() -> Option<Error> {
    #[cfg(not(feature = "plugin"))]
    crate::shell::raise_shm_error();
    LAST_ERROR.with(|prev| prev.borrow_mut().take())
}

/// Return the most recent error if one exists, otherwise Ok(ExecStatus::Success).
pub fn ok_or_error() -> Result<ExecStatus> {
    match last_error() {
        None => Ok(ExecStatus::Success),
        Some(e) => Err(e),
    }
}
