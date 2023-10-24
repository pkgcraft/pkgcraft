use pyo3::prelude::*;

pub(crate) mod cpv;
pub(crate) mod pkg;
pub(crate) mod version;

pub(crate) use cpv::Cpv;
pub(crate) use pkg::Dep;
pub(crate) use version::Version;

/// Dependency specification support.
#[pymodule]
#[pyo3(name = "dep")]
pub(super) fn module(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<Cpv>()?;
    m.add_class::<Dep>()?;
    m.add_class::<Version>()?;
    Ok(())
}
