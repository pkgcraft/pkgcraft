use std::env;
use std::fmt;
use std::io;

use crate::atom;

/// A `Result` alias where the `Err` case is `pkgcraft::error::Error`.
pub type Result<T> = ::std::result::Result<T, Error>;

#[derive(Clone, Debug)]
pub enum Error {
    ConfigError(String),
    ParseError(String),
    IOError(String),
    InvalidRepo { path: String, error: String },
}

impl ::std::error::Error for Error {}

impl From<env::VarError> for Error {
    fn from(error: env::VarError) -> Self {
        Error::ConfigError(format!("{}", error))
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::IOError(format!("{}", error))
    }
}

impl From<atom::ParseError> for Error {
    fn from(error: atom::ParseError) -> Self {
        Error::ParseError(format!("{}", error))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::ConfigError(ref s) => write!(f, "{}", s),
            Error::ParseError(ref s) => write!(f, "{}", s),
            Error::IOError(ref s) => write!(f, "{}", s),
            Error::InvalidRepo { path, error } => write!(f, "invalid repo {:?}: {}", path, error),
        }
    }
}
