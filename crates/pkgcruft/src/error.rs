use crate::check::Check;
use crate::report::ReportKind;

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Pkgcraft(String),
    #[error("{0}")]
    InvalidValue(String),
    #[error("{0}: report {1}")]
    ReportInit(ReportKind, String),
    #[error("{0}: check {1}")]
    CheckInit(Check, String),
}

impl From<pkgcraft::Error> for Error {
    fn from(e: pkgcraft::Error) -> Self {
        Error::Pkgcraft(e.to_string())
    }
}
