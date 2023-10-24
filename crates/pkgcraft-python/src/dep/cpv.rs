use ::pkgcraft::{dep, utils::hash};
use pyo3::basic::CompareOp;
use pyo3::prelude::*;

use crate::dep::Version;
use crate::Error;

#[pyclass(module = "pkgcraft.dep")]
pub(crate) struct Cpv(pub(crate) dep::Cpv);

#[pymethods]
impl Cpv {
    #[new]
    fn new(s: &str) -> PyResult<Self> {
        Ok(Self(dep::Cpv::new(s).map_err(Error)?))
    }

    #[getter]
    fn category(&self) -> &str {
        self.0.category()
    }

    #[getter]
    fn package(&self) -> &str {
        self.0.package()
    }

    #[getter]
    fn version(&self) -> Version {
        Version(self.0.version().clone())
    }

    #[getter]
    fn revision(&self) -> Option<&str> {
        self.0.version().revision().map(|x| x.as_str())
    }

    fn __hash__(&self) -> isize {
        hash(&self.0) as isize
    }

    fn __str__(&self) -> String {
        format!("{}", self.0)
    }

    fn __repr__(&self) -> String {
        format!("<Cpv '{}' at {:p}>", self.0, self)
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
