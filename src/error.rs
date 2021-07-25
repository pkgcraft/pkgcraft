use std::env;
use std::fmt;
use std::io;


/// A `Result` alias where the `Err` case is `pkgcraft::error::Error`.
pub type Result<T> = ::std::result::Result<T, Error>;

#[derive(Clone, Debug)]
pub enum Error {
    ConfigError(String),
    ParseError(String),
    IOError(String),
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

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::ConfigError(ref s) => write!(f, "{}", s),
            Error::ParseError(ref s) => write!(f, "{}", s),
            Error::IOError(ref s) => write!(f, "{}", s),
        }
    }
}
