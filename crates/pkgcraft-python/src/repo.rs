use std::mem;

use ::pkgcraft::repo::{self, PkgRepository, Repository};
use ::pkgcraft::utils::hash;
use pyo3::basic::CompareOp;
use pyo3::prelude::*;

use crate::pkg::Pkg;

mod ebuild;
mod fake;

#[pyclass(subclass, module = "pkgcraft.repo")]
pub(crate) struct Repo(pub(crate) repo::Repo);

impl Repo {
    pub(crate) fn to_object(repo: &repo::Repo) -> PyObject {
        use repo::Repo::*;
        Python::with_gil(|py| {
            let base = Repo(repo.clone());
            match repo {
                Ebuild(r) => {
                    let obj = (ebuild::EbuildRepo::new(r), base);
                    Py::new(py, obj).unwrap().into_py(py)
                }
                Fake(r) => {
                    let obj = (fake::FakeRepo::new(r), base);
                    Py::new(py, obj).unwrap().into_py(py)
                }
                _ => Py::new(py, base).unwrap().into_py(py),
            }
        })
    }
}

#[pyclass(unsendable, module = "pkgcraft.repo")]
struct IterPkg(repo::Iter<'static>);

impl IterPkg {
    fn new(iter: repo::Iter<'_>) -> Self {
        Self(unsafe { mem::transmute(iter) })
    }
}

#[pymethods]
impl IterPkg {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>) -> Option<PyObject> {
        slf.0.next().map(Pkg::to_object)
    }
}

#[pymethods]
impl Repo {
    #[getter]
    fn id(&self) -> &str {
        self.0.id()
    }

    fn __str__(&self) -> &str {
        self.id()
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

    fn __iter__(&self) -> IterPkg {
        IterPkg::new(self.0.iter())
    }
}

/// Repo support.
#[pymodule]
#[pyo3(name = "repo")]
pub(super) fn module(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<Repo>()?;
    Ok(())
}
