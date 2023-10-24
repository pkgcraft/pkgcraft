use ::pkgcraft::{dep, utils::hash};
use pyo3::basic::CompareOp;
use pyo3::prelude::*;

use crate::Error;

#[pyclass(module = "pkgcraft.dep")]
pub(crate) struct Dep(dep::Dep);

#[pymethods]
impl Dep {
    #[new]
    fn new(s: &str, eapi: Option<&str>) -> PyResult<Self> {
        Ok(Self(dep::Dep::new(s, eapi).map_err(Error)?))
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
    fn slot(&self) -> Option<&str> {
        self.0.slot()
    }

    #[getter]
    fn subslot(&self) -> Option<&str> {
        self.0.subslot()
    }

    #[getter]
    fn repo(&self) -> Option<&str> {
        self.0.repo()
    }

    #[getter]
    fn version(&self) -> Option<&str> {
        self.0.version().map(|x| x.as_str())
    }

    #[getter]
    fn revision(&self) -> Option<&str> {
        self.0.revision().map(|x| x.as_str())
    }

    #[getter]
    fn cpv(&self) -> String {
        self.0.cpv()
    }

    fn __hash__(&self) -> isize {
        hash(&self.0) as isize
    }

    fn __str__(&self) -> String {
        format!("{}", self.0)
    }

    fn __repr__(&self) -> String {
        format!("<Dep '{}' at {:p}>", self.0, self)
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

#[pyclass]
pub(crate) struct Version(dep::Version);

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
