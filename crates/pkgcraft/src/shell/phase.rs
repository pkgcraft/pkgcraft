use std::borrow::Borrow;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

use once_cell::sync::Lazy;
use scallop::builtins::ExecStatus;
use scallop::functions;
use strum::{AsRefStr, Display, EnumIter, EnumString};

use super::builtins::emake::run as emake;
use super::environment::VariableKind::D;
use super::hooks::{Hook, HookKind};
use super::scope::Scope;
use super::utils::makefile_exists;
use super::{get_build_mut, BuildData, BuildFn, BASH};

pub(crate) mod eapi0;
pub(crate) mod eapi2;
pub(crate) mod eapi4;
pub(crate) mod eapi6;

fn emake_install(build: &mut BuildData) -> scallop::Result<ExecStatus> {
    if makefile_exists() {
        let destdir = build.env.get(&D).expect("D undefined");
        let args = &[&format!("DESTDIR={destdir}"), "install"];
        emake(args)?;
    }

    Ok(ExecStatus::Success)
}

#[derive(AsRefStr, Display, EnumIter, EnumString, Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[strum(serialize_all = "snake_case")]
pub(crate) enum PhaseKind {
    PkgConfig,
    PkgInfo,
    PkgNofetch,
    PkgPostinst,
    PkgPostrm,
    PkgPreinst,
    PkgPrerm,
    PkgPretend,
    PkgSetup,
    SrcCompile,
    SrcConfigure,
    SrcInstall,
    SrcPrepare,
    SrcTest,
    SrcUnpack,
}

impl Ord for PhaseKind {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_ref().cmp(other.as_ref())
    }
}

impl PartialOrd for PhaseKind {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PhaseKind {
    /// Create a phase function that runs an optional, internal function by default.
    pub(crate) fn func(self, func: Option<BuildFn>) -> Phase {
        Phase { kind: self, func }
    }

    /// Create a new pre-phase hook.
    pub(crate) fn pre(self, name: &str, func: BuildFn, priority: usize, parallel: bool) -> Hook {
        Hook {
            phase: self,
            kind: HookKind::Pre,
            name: name.to_string(),
            func,
            priority,
            parallel,
        }
    }

    /// Create a new post-phase hook.
    pub(crate) fn post(self, name: &str, func: BuildFn, priority: usize, parallel: bool) -> Hook {
        Hook {
            phase: self,
            kind: HookKind::Post,
            name: name.to_string(),
            func,
            priority,
            parallel,
        }
    }
}

#[derive(Copy, Clone, PartialOrd, Ord)]
pub(crate) struct Phase {
    kind: PhaseKind,
    func: Option<BuildFn>,
}

impl fmt::Debug for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Phase {{ {}: {:?} }}", self.kind, self.func)
    }
}

impl<T: Borrow<Phase>> From<T> for PhaseKind {
    fn from(phase: T) -> PhaseKind {
        phase.borrow().kind
    }
}

impl AsRef<str> for Phase {
    fn as_ref(&self) -> &str {
        self.kind.as_ref()
    }
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.kind)
    }
}

impl PartialEq for Phase {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}

impl Eq for Phase {}

impl Hash for Phase {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.kind.hash(state);
    }
}

impl Borrow<PhaseKind> for Phase {
    fn borrow(&self) -> &PhaseKind {
        &self.kind
    }
}

impl Phase {
    /// Run the phase operation.
    pub(crate) fn run(&self) -> scallop::Result<ExecStatus> {
        Lazy::force(&BASH);

        let build = get_build_mut();
        build.scope = Scope::Phase(self.kind);

        // initialize phase scope variables
        build.set_vars()?;

        // run internal pre-phase hooks
        if let Some(hooks) = build.eapi().hooks().get(&self.kind) {
            if let Some(hooks) = hooks.get(&HookKind::Pre) {
                for hook in hooks {
                    hook.run(build)?;
                }
            }
        }

        // run user-defined pre-phase hooks
        if let Some(mut func) = functions::find(format!("pre_{self}")) {
            func.execute(&[])?;
        }

        // run phase function falling back to internal default if it exists
        if let Some(mut func) = functions::find(self) {
            func.execute(&[])?;
        } else {
            self.default(build)?;
        }

        // run user-defined post-phase hooks
        if let Some(mut func) = functions::find(format!("post_{self}")) {
            func.execute(&[])?;
        }

        // run internal post-phase hooks
        if let Some(hooks) = build.eapi().hooks().get(&self.kind) {
            if let Some(hooks) = hooks.get(&HookKind::Post) {
                for hook in hooks {
                    hook.run(build)?;
                }
            }
        }

        // unset phase scope variables
        for var in build.eapi().env() {
            if var.scopes().contains(&build.scope) {
                var.unbind()?;
            }
        }

        Ok(ExecStatus::Success)
    }

    /// Run the default phase function.
    pub(crate) fn default(&self, build: &mut BuildData) -> scallop::Result<ExecStatus> {
        match self.func {
            Some(func) => func(build),
            None => Ok(ExecStatus::Success),
        }
    }

    /// Return the shortened phase function name, e.g. src_compile -> compile.
    pub(crate) fn short_name(&self) -> &str {
        let s = self.as_ref();
        s.split_once('_').map_or(s, |x| x.1)
    }
}
