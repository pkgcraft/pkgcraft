use pyo3::prelude::*;
use pyo3::wrap_pymodule;

mod config;
mod dep;
mod eapi;
mod error;
mod parse;
mod pkg;
mod repo;

pub(crate) use self::error::Error;

/// Python library for pkgcraft.
#[pymodule]
#[pyo3(name = "pkgcraft")]
fn module(py: Python, m: &PyModule) -> PyResult<()> {
    // register submodules so `from pkgcraft.eapi import Eapi` works as expected
    m.add_wrapped(wrap_pymodule!(config::module))?;
    m.add_wrapped(wrap_pymodule!(dep::module))?;
    m.add_wrapped(wrap_pymodule!(eapi::module))?;
    m.add_wrapped(wrap_pymodule!(parse::module))?;
    m.add_wrapped(wrap_pymodule!(repo::module))?;
    let sys_modules = py.import("sys")?.getattr("modules")?;
    sys_modules.set_item("pkgcraft.config", m.getattr("config")?)?;
    sys_modules.set_item("pkgcraft.dep", m.getattr("dep")?)?;
    sys_modules.set_item("pkgcraft.eapi", m.getattr("eapi")?)?;
    sys_modules.set_item("pkgcraft.parse", m.getattr("parse")?)?;
    sys_modules.set_item("pkgcraft.repo", m.getattr("repo")?)?;

    m.add("PkgcraftError", py.get_type::<error::PkgcraftError>())?;
    Ok(())
}
