use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};

use indexmap::IndexSet;
use scallop::{ExecStatus, functions};
use strum::{AsRefStr, Display, EnumIter, EnumString};

use super::commands::functions::emake;
use super::environment::Variable;
use super::hooks::{Hook, HookBuilder, HookKind};
use super::utils::makefile_exists;
use super::{BuildData, BuildFn, get_build_mut};

pub(crate) mod eapi5;
pub(crate) mod eapi6;

fn emake_install(build: &mut BuildData) -> scallop::Result<ExecStatus> {
    if makefile_exists() {
        let destdir = build.env(&Variable::D);
        let args = &[&format!("DESTDIR={destdir}"), "install"];
        emake(args)?;
    }

    Ok(ExecStatus::Success)
}

#[derive(AsRefStr, Display, EnumIter, EnumString, Debug, Copy, Clone)]
#[strum(serialize_all = "snake_case")]
pub enum PhaseKind {
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

impl PhaseKind {
    /// Create a phase function that runs an internal function by default.
    pub(crate) fn func(self, func: BuildFn) -> Phase {
        Phase {
            kind: self,
            func: Some(func),
            hooks: Default::default(),
        }
    }

    /// Create a new pre-phase hook builder.
    pub(crate) fn pre(self, name: &str, func: BuildFn) -> HookBuilder {
        HookBuilder {
            phase: self,
            kind: HookKind::Pre,
            name: name.to_string(),
            func,
            priority: 0,
        }
    }

    /// Create a new post-phase hook builder.
    pub(crate) fn post(self, name: &str, func: BuildFn) -> HookBuilder {
        HookBuilder {
            phase: self,
            kind: HookKind::Post,
            name: name.to_string(),
            func,
            priority: 0,
        }
    }

    /// Return the short phase name, e.g. src_compile -> compile.
    pub fn name(&self) -> &str {
        self.as_ref()
            .split_once('_')
            .unwrap_or_else(|| panic!("invalid phase name: {self}"))
            .1
    }
}

impl PartialEq for PhaseKind {
    fn eq(&self, other: &Self) -> bool {
        self.name() == other.name()
    }
}

impl Eq for PhaseKind {}

impl Hash for PhaseKind {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name().hash(state);
    }
}

impl Borrow<str> for PhaseKind {
    fn borrow(&self) -> &str {
        self.name()
    }
}

impl Ord for PhaseKind {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name().cmp(other.name())
    }
}

impl PartialOrd for PhaseKind {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone)]
pub struct Phase {
    pub kind: PhaseKind,
    func: Option<BuildFn>,
    pub(crate) hooks: HashMap<HookKind, IndexSet<Hook>>,
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

impl Ord for Phase {
    fn cmp(&self, other: &Self) -> Ordering {
        self.kind.cmp(&other.kind)
    }
}

impl PartialOrd for Phase {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

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

impl Borrow<str> for Phase {
    fn borrow(&self) -> &str {
        self.kind.borrow()
    }
}

impl From<PhaseKind> for Phase {
    fn from(value: PhaseKind) -> Self {
        Self {
            kind: value,
            func: None,
            hooks: Default::default(),
        }
    }
}

impl Phase {
    /// Run the phase operation.
    #[allow(dead_code)]
    pub(crate) fn run(&self) -> scallop::Result<ExecStatus> {
        let build = get_build_mut();
        let _scope = build.in_phase(self.kind);

        // initialize phase scope variables
        build.set_vars()?;

        // run internal pre-phase hooks
        if let Some(hooks) = self.hooks.get(&HookKind::Pre) {
            for hook in hooks {
                hook.run(build)?;
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
            self.default()?;
        }

        // run user-defined post-phase hooks
        if let Some(mut func) = functions::find(format!("post_{self}")) {
            func.execute(&[])?;
        }

        // run internal post-phase hooks
        if let Some(hooks) = self.hooks.get(&HookKind::Post) {
            for hook in hooks {
                hook.run(build)?;
            }
        }

        // unset phase scope variables
        for var in build.eapi().env() {
            if var.is_allowed(&build.scope) {
                var.unbind()?;
            }
        }

        Ok(ExecStatus::Success)
    }

    /// Run the default phase function.
    pub(crate) fn default(&self) -> scallop::Result<ExecStatus> {
        match self.func {
            Some(func) => func(get_build_mut()),
            None => Ok(ExecStatus::Success),
        }
    }

    /// Return the phase name, e.g. src_compile -> compile.
    pub fn name(&self) -> &str {
        self.kind.name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phasekind_traits() {
        let src_compile = PhaseKind::SrcCompile;
        assert_eq!(src_compile, src_compile);

        // hash and borrow
        let set = IndexSet::from([src_compile]);
        assert_eq!(set.get("compile").unwrap(), &src_compile);

        // ordered by short name
        let pkg_setup = PhaseKind::PkgSetup;
        assert!(src_compile < pkg_setup);
    }

    #[test]
    fn phase_traits() {
        let src_compile: Phase = PhaseKind::SrcCompile.into();
        assert_eq!(src_compile, src_compile);

        // hash and borrow
        let set = IndexSet::from([src_compile.clone()]);
        assert_eq!(set.get("compile").unwrap(), &src_compile);
        assert_eq!(set.get(&PhaseKind::SrcCompile).unwrap(), &src_compile);

        // ordered by short name
        let pkg_setup: Phase = PhaseKind::PkgSetup.into();
        assert!(src_compile < pkg_setup);
    }
}
