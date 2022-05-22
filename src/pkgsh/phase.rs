use std::fmt;

use scallop::builtins::ExecStatus;
use scallop::Result;

use super::builtins::emake::run as emake;
use super::utils::makefile_exists;
use super::BUILD_DATA;

pub(crate) mod eapi0;
pub(crate) mod eapi1;
pub(crate) mod eapi2;
pub(crate) mod eapi4;
pub(crate) mod eapi6;

pub(crate) type PhaseFn = fn() -> Result<ExecStatus>;
pub(crate) static PHASE_STUB: PhaseFn = phase_stub;

fn phase_stub() -> Result<ExecStatus> {
    Ok(ExecStatus::Success)
}

fn emake_install() -> Result<ExecStatus> {
    if makefile_exists() {
        BUILD_DATA.with(|d| -> Result<ExecStatus> {
            let env = &d.borrow().env;
            #[allow(non_snake_case)]
            let D = env.get("D").expect("D undefined");
            let destdir = format!("DESTDIR={D}");
            let args = &[destdir.as_str(), "install"];
            emake(args)
        })?;
    }

    Ok(ExecStatus::Success)
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct Phase {
    name: &'static str,
    func: PhaseFn,
}

impl From<&Phase> for &str {
    fn from(phase: &Phase) -> &'static str {
        phase.name
    }
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl AsRef<str> for Phase {
    fn as_ref(&self) -> &str {
        self.name
    }
}

impl Phase {
    pub(crate) fn new(name: &'static str, func: PhaseFn) -> Self {
        Phase { name, func }
    }

    pub(crate) fn run(&self) -> scallop::Result<ExecStatus> {
        (self.func)()
    }

    /// Return the phase function name, e.g. src_compile.
    pub(crate) fn name(&self) -> &str {
        self.name
    }

    /// Return the shortened phase function name, e.g. src_compile -> compile.
    pub(crate) fn short_name(&self) -> &str {
        self.name.split_once('_').map_or(self.name, |x| x.1)
    }
}
