use ::pkgcraft::{dep, utils::hash};
use pyo3::basic::CompareOp;
use pyo3::prelude::*;

use crate::Error;

#[pyclass(module = "pkgcraft.dep")]
pub(crate) struct Version(pub(crate) dep::Version);

#[pymethods]
impl Version {
    #[new]
    fn new(s: &str) -> PyResult<Self> {
        Ok(Self(dep::Version::new(s).map_err(Error)?))
    }

    fn __hash__(&self) -> isize {
        hash(&self.0) as isize
    }

    fn __str__(&self) -> String {
        format!("{}", self.0)
    }

    fn __repr__(&self) -> String {
        format!("<Version '{}' at {:p}>", self.0, self)
    }

    fn __richcmp__(&self, other: PyRef<Self>, op: CompareOp) -> bool {
        match op {
            CompareOp::Eq => self.0 == other.0,
            CompareOp::Ne => self.0 != other.0,
            CompareOp::Lt => self.0 < other.0,
            CompareOp::Gt => self.0 > other.0,
            CompareOp::Le => self.0 <= other.0,
            CompareOp::Ge => self.0 >= other.0,
        }
    }
}
