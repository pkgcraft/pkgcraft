use std::mem;

use ::pkgcraft::repo;
use pyo3::prelude::*;

use super::Repo;

#[pyclass(extends=Repo)]
pub(crate) struct FakeRepo(pub(crate) &'static repo::fake::Repo);

impl FakeRepo {
    pub(crate) fn new(repo: &repo::fake::Repo) -> Self {
        Self(unsafe { mem::transmute(repo) })
    }
}
