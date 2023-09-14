use std::borrow::Borrow;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

use indexmap::IndexSet;
use scallop::builtins::ExecStatus;
use scallop::variables::{bind, unbind, Attr};
use strum::{AsRefStr, Display, EnumIter, EnumString};

use super::scope::{Scope, Scopes};

#[derive(AsRefStr, Display, EnumIter, EnumString, Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[strum(serialize_all = "UPPERCASE")]
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
pub(crate) enum VariableKind {
    // package specific
    CATEGORY,
    P,
    PF,
    PN,
    PR,
    PV,
    PVR,

    // environment specific
    A,
    AA,
    FILESDIR,
    DISTDIR,
    WORKDIR,
    S,
    PORTDIR,
    ECLASSDIR,
    ROOT,
    EROOT,
    SYSROOT,
    ESYSROOT,
    BROOT,
    T,
    TMPDIR,
    HOME,
    EPREFIX,
    D,
    ED,
    DESTTREE,
    INSDESTTREE,
    USE,
    EBUILD_PHASE,
    EBUILD_PHASE_FUNC,
    KV,
    MERGE_TYPE,
    REPLACING_VERSIONS,
    REPLACED_BY_VERSION,
}

impl Ord for VariableKind {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_ref().cmp(other.as_ref())
    }
}

impl PartialOrd for VariableKind {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl VariableKind {
    pub(crate) fn scopes<I: IntoIterator<Item = Scopes>>(self, scopes: I) -> Variable {
        let mut scopes: IndexSet<_> = scopes.into_iter().flatten().collect();
        scopes.sort();
        Variable { kind: self, scopes }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Variable {
    kind: VariableKind,
    scopes: IndexSet<Scope>,
}

impl Ord for Variable {
    fn cmp(&self, other: &Self) -> Ordering {
        self.kind.cmp(&other.kind)
    }
}

impl PartialOrd for Variable {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Variable {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}

impl Eq for Variable {}

impl Hash for Variable {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.kind.hash(state);
    }
}

impl fmt::Display for Variable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.kind)
    }
}

impl Borrow<VariableKind> for Variable {
    fn borrow(&self) -> &VariableKind {
        &self.kind
    }
}

impl AsRef<str> for Variable {
    fn as_ref(&self) -> &str {
        self.kind.as_ref()
    }
}

impl From<&Variable> for VariableKind {
    fn from(value: &Variable) -> Self {
        value.kind
    }
}

impl Variable {
    pub(crate) fn scopes(&self) -> &IndexSet<Scope> {
        &self.scopes
    }

    /// Externally exported to the package build environment.
    pub(crate) fn is_exported(&self) -> bool {
        use VariableKind::*;
        matches!(self.kind, HOME | TMPDIR)
    }

    /// Variable value does not vary between phases.
    pub(crate) fn is_static(&self) -> bool {
        use VariableKind::*;
        !matches!(self.kind, EBUILD_PHASE | EBUILD_PHASE_FUNC)
    }

    pub(crate) fn bind(&self, value: &str) -> scallop::Result<ExecStatus> {
        let attrs = if self.is_exported() {
            Some(Attr::EXPORTED)
        } else {
            None
        };

        bind(self, value, None, attrs)
    }

    pub(crate) fn unbind(&self) -> scallop::Result<ExecStatus> {
        unbind(self)
    }
}

#[cfg(test)]
mod tests {
    use scallop::variables;
    use strum::IntoEnumIterator;

    use crate::config::Config;
    use crate::eapi::EAPIS_OFFICIAL;
    use crate::pkg::{BuildablePackage, SourceablePackage};
    use crate::shell::{get_build_mut, BuildData};

    use super::*;

    #[test]
    fn exports() {
        use crate::shell::scope::Scope::*;

        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let all_scopes: IndexSet<_> = Scopes::All.into_iter().collect();

        for eapi in EAPIS_OFFICIAL.iter() {
            for var in VariableKind::iter() {
                for scope in &all_scopes {
                    match scope {
                        Global => {
                            let data = indoc::formatdoc! {r#"
                                EAPI={eapi}
                                DESCRIPTION="testing {var} exporting"
                                SLOT=0
                            "#};
                            let raw_pkg = t.create_raw_pkg_from_str("cat/pkg-1", &data).unwrap();
                            raw_pkg.source().unwrap();
                            if eapi
                                .env()
                                .get(&var)
                                .is_some_and(|v| v.scopes().contains(scope))
                            {
                                assert!(
                                    variables::optional(var).is_some(),
                                    "EAPI {eapi}: ${var} not exported globally"
                                );
                            } else {
                                assert!(
                                    variables::optional(var).is_none(),
                                    "EAPI {eapi}: ${var} shouldn't be exported globally"
                                );
                            }
                        }
                        Phase(phase) if eapi.phases().contains(phase) => {
                            let exported = if eapi.env().get(&var).is_some_and(|v| {
                                v.scopes().contains(scope) || v.scopes().contains(&Global)
                            }) {
                                "yes"
                            } else {
                                ""
                            };

                            let data = indoc::formatdoc! {r#"
                                EAPI={eapi}
                                DESCRIPTION="testing {var} exporting"
                                SLOT=0
                                {phase}() {{
                                    if [[ -n "{exported}" ]]; then
                                        [[ -v {var} ]] || die "EAPI {eapi}: ${var} not exported in {phase}"
                                    else
                                        [[ -v {var} ]] && die "EAPI {eapi}: ${var} shouldn't be exported in {phase}"
                                    fi
                                    :
                                }}
                            "#};
                            let pkg = t.create_pkg_from_str("cat/pkg-1", &data).unwrap();
                            BuildData::from_pkg(&pkg);
                            get_build_mut().source_ebuild(&pkg.abspath()).unwrap();
                            let phase = eapi.phases().get(phase).unwrap();
                            phase.run().unwrap();

                            BuildData::from_pkg(&pkg);
                            pkg.build().unwrap();
                            if !eapi
                                .env()
                                .get(&var)
                                .is_some_and(|v| v.scopes().contains(&Global))
                            {
                                assert!(
                                    variables::optional(var).is_none(),
                                    "EAPI {eapi}: ${var} is leaking into global scope"
                                );
                            }
                        }
                        _ => (),
                    }
                }
            }
        }
    }
}
