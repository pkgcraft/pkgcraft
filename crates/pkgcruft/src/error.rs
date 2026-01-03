use std::io;

use crate::check::Check;
use crate::report::ReportKind;

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    InvalidValue(String),
    #[error("{0}: report {1}")]
    ReportInit(ReportKind, String),
    #[error("{0}: check {1}")]
    CheckInit(Check, String),
    #[error("{0}")]
    ChannelClosed(String),
    #[error("{0}")]
    IO(String),
    #[error("{0}")]
    Pkgcraft(#[from] pkgcraft::Error),
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::IO(format!("{e}: {}", e.kind()))
    }
}

impl<T> From<crossbeam_channel::SendError<T>> for Error {
    fn from(e: crossbeam_channel::SendError<T>) -> Self {
        Error::ChannelClosed(format!("{e}"))
    }
}
