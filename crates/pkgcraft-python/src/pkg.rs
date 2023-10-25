use ::pkgcraft::pkg::{self, Package};
use ::pkgcraft::utils::hash;
use pyo3::basic::CompareOp;
use pyo3::prelude::*;

use crate::dep::{Cpv, Version};
use crate::eapi::Eapi;

pub(crate) mod ebuild;

#[pyclass(subclass, module = "pkgcraft.pkg")]
pub(crate) struct Pkg(pub(super) pkg::Pkg<'static>);

impl Pkg {
    pub(crate) fn to_object(pkg: pkg::Pkg<'static>) -> PyObject {
        use pkg::Pkg::*;
        Python::with_gil(|py| match &pkg {
            Ebuild(p, _) => {
                let obj = (ebuild::EbuildPkg::new(p), Pkg(pkg));
                Py::new(py, obj).unwrap().into_py(py)
            }
            _ => Py::new(py, Pkg(pkg)).unwrap().into_py(py),
        })
    }
}

#[pymethods]
impl Pkg {
    #[getter]
    fn eapi(&self) -> Eapi {
        self.0.eapi().into()
    }

    #[getter]
    fn cpv(&self) -> Cpv {
        self.0.cpv().clone().into()
    }

    #[getter]
    fn version(&self) -> Version {
        Version(self.0.cpv().version().clone())
    }

    fn __str__(&self) -> String {
        self.0.to_string()
    }

    fn __repr__(slf: &PyCell<Self>) -> PyResult<String> {
        let name = slf.get_type().name()?;
        Ok(format!("<{name} '{}' at {:p}>", slf.borrow().0, slf))
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

    fn __hash__(&self) -> Result<isize, PyErr> {
        Ok(hash(&self.0) as isize)
    }
}

/// Package support.
#[pymodule]
#[pyo3(name = "pkg")]
pub(super) fn module(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<Pkg>()?;
    Ok(())
}
