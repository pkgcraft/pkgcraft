use std::collections::HashMap;

use ::pkgcraft::config;
use pyo3::prelude::*;

use crate::repo::Repo;
use crate::Error;

#[pyclass(module = "pkgcraft.config")]
pub(super) struct Config(config::Config);

#[pymethods]
impl Config {
    #[new]
    fn new() -> Self {
        Self(config::Config::new("pkgcraft", ""))
    }

    fn load(&mut self) -> PyResult<()> {
        Ok(self.0.load().map_err(Error)?)
    }

    #[getter]
    fn repos(&self) -> HashMap<&str, PyObject> {
        self.0
            .repos
            .iter()
            .map(|(id, r)| (id, Repo::to_object(r)))
            .collect()
    }
}

/// Dependency specification support.
#[pymodule]
#[pyo3(name = "config")]
pub(super) fn module(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<Config>()?;
    Ok(())
}
