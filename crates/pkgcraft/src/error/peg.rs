use std::fmt::Write;

type PegError = ::peg::error::ParseError<::peg::str::LineCol>;

/// Convert a PEG parsing error to an internal pkgcraft error type.
pub(crate) fn peg_error(msg: &str, src: &str, error: PegError) -> super::Error {
    let msg = if src.is_empty() {
        msg.into()
    } else {
        format!("{msg}: {src}")
    };

    let mut err = String::new();
    if src.is_empty() {
        write!(err, "{msg}: empty string").unwrap();
    } else {
        let chic_error = chic::Error::new(format!("parsing failure: {msg}")).error(
            error.location.line,
            error.location.offset,
            error.location.offset + 1,
            src,
            format!("Expected: {}", error.expected),
        );
        let s = chic_error.to_string();
        // don't prefix error messages
        let s = s.strip_prefix("error: ").unwrap_or(&s);
        write!(err, "{s}").unwrap();
    }

    super::Error::PegParse(err)
}
