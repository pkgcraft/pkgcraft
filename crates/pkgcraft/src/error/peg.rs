use std::fmt;

type PegError = ::peg::error::ParseError<::peg::str::LineCol>;

#[derive(Debug, Clone)]
pub struct Error {
    msg: String,
    src: String,
    error: PegError,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.src.is_empty() {
            write!(f, "{}: empty string", self.msg)
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
            let s = s.strip_prefix("error: ").unwrap_or(&s);
            write!(f, "{s}")
        }
    }
}

/// Convert a PEG parsing error to an internal pkgcraft error type.
pub(crate) fn peg_error(msg: &str, src: &str, error: PegError) -> super::Error {
    let msg = if src.is_empty() {
        msg.into()
    } else {
        format!("{msg}: {src}")
    };

    let error = Error {
        msg,
        src: src.to_string(),
        error,
    };
    super::Error::PegParse(error)
}
