use std::hash::{Hash, Hasher};

use scallop::builtins::ExecStatus;
use strum::{AsRefStr, Display};

use super::builtins::emake::run as emake;
use super::utils::makefile_exists;
use super::BUILD_DATA;

pub(crate) mod eapi0;
pub(crate) mod eapi1;
pub(crate) mod eapi2;
pub(crate) mod eapi4;
pub(crate) mod eapi6;

pub(crate) type PhaseFn = fn() -> scallop::Result<ExecStatus>;
pub(crate) static PHASE_STUB: PhaseFn = phase_stub;

fn phase_stub() -> scallop::Result<ExecStatus> {
    Ok(ExecStatus::Success)
}

fn emake_install() -> scallop::Result<ExecStatus> {
    if makefile_exists() {
        BUILD_DATA.with(|d| -> scallop::Result<ExecStatus> {
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

#[derive(AsRefStr, Display, Debug, Copy, Clone)]
#[strum(serialize_all = "snake_case")]
pub(crate) enum Phase {
    PkgSetup(PhaseFn),
    PkgConfig(PhaseFn),
    PkgInfo(PhaseFn),
    PkgNofetch(PhaseFn),
    PkgPrerm(PhaseFn),
    PkgPostrm(PhaseFn),
    PkgPreinst(PhaseFn),
    PkgPostinst(PhaseFn),
    PkgPretend(PhaseFn),
    SrcUnpack(PhaseFn),
    SrcPrepare(PhaseFn),
    SrcConfigure(PhaseFn),
    SrcCompile(PhaseFn),
    SrcTest(PhaseFn),
    SrcInstall(PhaseFn),
}

impl PartialEq for Phase {
    fn eq(&self, other: &Self) -> bool {
        self.name() == other.name()
    }
}

impl Eq for Phase {}

impl Hash for Phase {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name().hash(state);
    }
}

impl Phase {
    /// Run the phase function.
    pub(crate) fn run(&self) -> scallop::Result<ExecStatus> {
        use Phase::*;
        match self {
            PkgSetup(f) => f(),
            PkgConfig(f) => f(),
            PkgInfo(f) => f(),
            PkgNofetch(f) => f(),
            PkgPrerm(f) => f(),
            PkgPostrm(f) => f(),
            PkgPreinst(f) => f(),
            PkgPostinst(f) => f(),
            PkgPretend(f) => f(),
            SrcUnpack(f) => f(),
            SrcPrepare(f) => f(),
            SrcConfigure(f) => f(),
            SrcCompile(f) => f(),
            SrcTest(f) => f(),
            SrcInstall(f) => f(),
        }
    }

    /// Return the phase function name, e.g. src_compile.
    pub(crate) fn name(&self) -> &str {
        self.as_ref()
    }

    /// Return the shortened phase function name, e.g. src_compile -> compile.
    pub(crate) fn short_name(&self) -> &str {
        let s = self.name();
        s.split_once('_').map_or(s, |x| x.1)
    }
}
