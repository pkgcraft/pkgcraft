use std::fmt;
use std::hash::{Hash, Hasher};

use once_cell::sync::Lazy;
use scallop::builtins::ExecStatus;
use scallop::functions;
use strum::{AsRefStr, Display};

use super::builtins::{emake::run as emake, Scope};
use super::utils::makefile_exists;
use super::{get_build_mut, BASH};

pub(crate) mod eapi0;
pub(crate) mod eapi1;
pub(crate) mod eapi2;
pub(crate) mod eapi4;
pub(crate) mod eapi6;

pub(crate) type PhaseFn = fn() -> scallop::Result<ExecStatus>;
static PHASE_STUB: PhaseFn = phase_stub;

fn phase_stub() -> scallop::Result<ExecStatus> {
    Ok(ExecStatus::Success)
}

fn emake_install() -> scallop::Result<ExecStatus> {
    if makefile_exists() {
        let destdir = get_build_mut().env.get("D").expect("D undefined");
        let args = &[&format!("DESTDIR={destdir}"), "install"];
        emake(args)?;
    }

    Ok(ExecStatus::Success)
}

#[derive(AsRefStr, Display, Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[strum(serialize_all = "snake_case")]
pub(crate) enum PhaseKind {
    PkgSetup,
    PkgConfig,
    PkgInfo,
    PkgNofetch,
    PkgPrerm,
    PkgPostrm,
    PkgPreinst,
    PkgPostinst,
    PkgPretend,
    SrcUnpack,
    SrcPrepare,
    SrcConfigure,
    SrcCompile,
    SrcTest,
    SrcInstall,
}

impl PhaseKind {
    pub(crate) fn stub(self) -> Phase {
        Phase {
            phase: self,
            pre: None,
            func: PHASE_STUB,
            post: None,
        }
    }

    pub(crate) fn func(self, func: PhaseFn) -> Phase {
        Phase {
            phase: self,
            pre: None,
            func,
            post: None,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct Phase {
    phase: PhaseKind,
    pre: Option<PhaseFn>,
    func: PhaseFn,
    post: Option<PhaseFn>,
}

impl AsRef<str> for Phase {
    fn as_ref(&self) -> &str {
        self.phase.as_ref()
    }
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.phase)
    }
}

impl PartialEq for Phase {
    fn eq(&self, other: &Self) -> bool {
        self.phase == other.phase
    }
}

impl Eq for Phase {}

impl Hash for Phase {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.phase.hash(state);
    }
}

impl Phase {
    pub(crate) fn pre(mut self, func: PhaseFn) -> Self {
        self.pre = Some(func);
        self
    }

    pub(crate) fn post(mut self, func: PhaseFn) -> Self {
        self.post = Some(func);
        self
    }

    /// Run the phase operation.
    pub(crate) fn run(&self) -> scallop::Result<ExecStatus> {
        Lazy::force(&BASH);

        let build = get_build_mut();
        build.scope = Scope::Phase(*self);
        build.set_vars()?;

        // run internal pre-phase hooks
        if let Some(func) = self.pre {
            func()?;
        }

        // run user-defined pre-phase hooks
        if let Some(mut func) = functions::find(format!("pre_{self}")) {
            func.execute(&[])?;
        }

        // run phase function falling back to internal default
        if let Some(mut func) = functions::find(self) {
            func.execute(&[])?;
        } else {
            (self.func)()?;
        }

        // run user-defined post-phase hooks
        if let Some(mut func) = functions::find(format!("post_{self}")) {
            func.execute(&[])?;
        }

        // run internal post-phase hooks
        if let Some(func) = self.post {
            func()?;
        }

        Ok(ExecStatus::Success)
    }

    /// Return the phase function name, e.g. src_compile.
    pub(crate) fn name(&self) -> &str {
        self.phase.as_ref()
    }

    /// Return the shortened phase function name, e.g. src_compile -> compile.
    pub(crate) fn short_name(&self) -> &str {
        let s = self.name();
        s.split_once('_').map_or(s, |x| x.1)
    }
}

pub(crate) fn pre_src_install() -> scallop::Result<ExecStatus> {
    let build = get_build_mut();

    // set docompress include/exclude defaults for supported EAPIs
    if build
        .eapi()
        .builtins(build.scope)
        .contains_key("docompress")
    {
        let docompress_include_defaults = ["/usr/share/doc", "/usr/share/info", "/usr/share/man"]
            .into_iter()
            .map(String::from);
        let docompress_exclude_defaults = [format!("/usr/share/doc/{}/html", build.cpv()?.pf())];
        build.compress_include.extend(docompress_include_defaults);
        build.compress_exclude.extend(docompress_exclude_defaults);
    }

    // TODO: set dostrip include/exclude defaults

    Ok(ExecStatus::Success)
}

pub(crate) fn post_src_install() -> scallop::Result<ExecStatus> {
    // TODO: perform docompress and dostrip operations if supported
    Ok(ExecStatus::Success)
}
