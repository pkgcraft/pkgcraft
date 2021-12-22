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
            chic_error.to_string()
        };
        write!(f, "{}", err)
    }
}

/// Convert a PEG parsing error to an internal pkgcraft error type.
pub(crate) fn peg_error(msg: &str, src: &str, error: PegError) -> error::Error {
    let error = Error {
        msg: msg.to_string(),
        src: src.to_string(),
        error,
    };
    error::Error::PegParse(error)
}
