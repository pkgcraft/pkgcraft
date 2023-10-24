use std::mem;

use ::pkgcraft::pkg;
use pyo3::prelude::*;

use super::Pkg;

#[pyclass(extends=Pkg)]
pub(crate) struct EbuildPkg(pub(crate) &'static pkg::ebuild::Pkg<'static>);

impl EbuildPkg {
    pub(crate) fn new(pkg: &pkg::ebuild::Pkg<'_>) -> Self {
        Self(unsafe { mem::transmute(pkg) })
    }
}

#[pymethods]
impl EbuildPkg {
    #[getter]
    fn description(&self) -> &str {
        self.0.description()
    }

    #[getter]
    fn slot(&self) -> &str {
        self.0.slot()
    }

    #[getter]
    fn subslot(&self) -> &str {
        self.0.subslot()
    }
}
