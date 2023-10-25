use std::sync::OnceLock;

use ::pkgcraft::{dep, utils::hash};
use pyo3::basic::CompareOp;
use pyo3::prelude::*;

use crate::dep::Version;
use crate::Error;

#[pyclass(module = "pkgcraft.dep")]
pub(crate) struct Cpv {
    cpv: dep::Cpv,
    ver: OnceLock<Py<Version>>,
}

impl From<dep::Cpv> for Cpv {
    fn from(cpv: dep::Cpv) -> Self {
        Self { cpv, ver: OnceLock::new() }
    }
}

#[pymethods]
impl Cpv {
    #[new]
    fn new(s: &str) -> PyResult<Self> {
        let cpv = dep::Cpv::new(s).map_err(Error)?;
        Ok(cpv.into())
    }

    #[getter]
    fn category(&self) -> &str {
        self.cpv.category()
    }

    #[getter]
    fn package(&self) -> &str {
        self.cpv.package()
    }

    #[getter]
    fn version(&mut self) -> &Py<Version> {
        self.ver.get_or_init(|| {
            Python::with_gil(|py| Py::new(py, Version(self.cpv.version().clone())).unwrap())
        })
    }

    #[getter]
    fn revision(&self) -> Option<&str> {
        self.cpv.version().revision().map(|x| x.as_str())
    }

    fn __hash__(&self) -> isize {
        hash(&self.cpv) as isize
    }

    fn __str__(&self) -> String {
        format!("{}", self.cpv)
    }

    fn __repr__(&self) -> String {
        format!("<Cpv '{}' at {:p}>", self.cpv, self)
    }

    fn __richcmp__(&self, other: PyRef<Self>, op: CompareOp) -> bool {
        match op {
            CompareOp::Eq => self.cpv == other.cpv,
            CompareOp::Ne => self.cpv != other.cpv,
            CompareOp::Lt => self.cpv < other.cpv,
            CompareOp::Gt => self.cpv > other.cpv,
            CompareOp::Le => self.cpv <= other.cpv,
            CompareOp::Ge => self.cpv >= other.cpv,
        }
    }
}
