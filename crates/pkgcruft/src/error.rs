use crate::check::CheckKind;

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    InvalidValue(String),
    #[error("skipping remaining checks due to failure: {0}")]
    SkipRemainingChecks(CheckKind),
}
