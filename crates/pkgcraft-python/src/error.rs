use std::{error, fmt};

use pyo3::exceptions::PyException;
use pyo3::{create_exception, PyErr};

#[derive(Debug)]
pub(crate) struct Error(pub(crate) ::pkgcraft::Error);

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

create_exception!(pkgcraft, PkgcraftError, PyException, "Generic pkgcraft error.");

impl From<Error> for PyErr {
    fn from(err: Error) -> PyErr {
        PkgcraftError::new_err(err.0.to_string())
    }
}
