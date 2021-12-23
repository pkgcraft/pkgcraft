use std::fmt;

use crate::error;

pub(crate) type PegError = ::peg::error::ParseError<::peg::str::LineCol>;

#[derive(Debug, Clone)]
pub struct Error {
    msg: String,
    src: String,
    error: PegError,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let err = if self.src.is_empty() {
            format!("{}: empty string", self.msg)
        } else {
            let chic_error = chic::Error::new(format!("parsing failure: {}", self.msg)).error(
                self.error.location.line,
                self.error.location.offset,
                self.error.location.offset + 1,
                &self.src,
                format!("Expected: {}", self.error.expected),
            );
            let s = chic_error.to_string();
            // don't prefix error messages
            s.strip_prefix("error: ").unwrap_or(&s).to_string()
        };
        write!(f, "{}", err)
    }
}

/// Convert a PEG parsing error to an internal pkgcraft error type.
pub(crate) fn peg_error<S1, S2>(msg: S1, src: S2, error: PegError) -> error::Error
where
    S1: Into<String>,
    S2: Into<String>,
{
    let error = Error {
        msg: msg.into(),
        src: src.into(),
        error,
    };
    error::Error::PegParse(error)
}
