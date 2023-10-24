use std::mem;

use ::pkgcraft::repo;
use pyo3::prelude::*;

use crate::eapi::Eapi;

use super::Repo;

#[pyclass(extends=Repo)]
pub(crate) struct EbuildRepo(pub(crate) &'static repo::ebuild::Repo);

impl EbuildRepo {
    pub(crate) fn new(repo: &repo::ebuild::Repo) -> Self {
        Self(unsafe { mem::transmute(repo) })
    }
}

#[pymethods]
impl EbuildRepo {
    #[getter]
    fn eapi(&self) -> Eapi {
        self.0.eapi().into()
    }
}
