use ::pkgcraft::{eapi, utils::hash};
use pyo3::basic::CompareOp;
use pyo3::prelude::*;

#[pyclass(module = "pkgcraft.eapi")]
pub(crate) struct Eapi(&'static eapi::Eapi);

impl From<&'static eapi::Eapi> for Eapi {
    fn from(eapi: &'static eapi::Eapi) -> Self {
        Self(eapi)
    }
}

#[pymethods]
impl Eapi {
    fn has(&self, s: &str) -> bool {
        s.parse().map(|x| self.0.has(x)).unwrap_or_default()
    }

    fn __hash__(&self) -> isize {
        hash(self.0) as isize
    }

    fn __str__(&self) -> &str {
        self.0.as_str()
    }

    fn __repr__(&self) -> String {
        format!("<Eapi '{}' at {:p}>", self.0, self)
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

/// EAPI support.
#[pymodule]
#[pyo3(name = "eapi")]
pub(super) fn module(_py: Python, m: &PyModule) -> PyResult<()> {
    // define global module attributes for EAPIs
    for eapi in &*eapi::EAPIS_OFFICIAL {
        m.add(&format!("EAPI{eapi}"), Eapi(eapi))?;
    }
    m.add("EAPI_LATEST_OFFICIAL", Eapi(*eapi::EAPI_LATEST_OFFICIAL))?;
    m.add("EAPI_LATEST", Eapi(*eapi::EAPI_LATEST))?;
    // TODO: use readonly mappings
    let eapis_official: Vec<Eapi> = eapi::EAPIS_OFFICIAL
        .iter()
        .copied()
        .map(Into::into)
        .collect();
    m.add("EAPIS_OFFICIAL", eapis_official)?;
    let eapis: Vec<Eapi> = eapi::EAPIS.iter().copied().map(Into::into).collect();
    m.add("EAPIS", eapis)?;

    m.add_class::<Eapi>()?;
    Ok(())
}
