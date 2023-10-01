use std::borrow::Borrow;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

use indexmap::IndexSet;
use scallop::variables::{bind, unbind, Attr};
use scallop::ExecStatus;
use strum::{AsRefStr, Display, EnumIter, EnumString};

use super::scope::{Scope, Scopes};

#[derive(AsRefStr, Display, EnumIter, EnumString, Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[strum(serialize_all = "UPPERCASE")]
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
pub(crate) enum Variable {
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
    MERGE_TYPE,
    REPLACING_VERSIONS,
    REPLACED_BY_VERSION,
}

impl Ord for Variable {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_ref().cmp(other.as_ref())
    }
}

impl PartialOrd for Variable {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Variable {
    pub(crate) fn scopes<I, S>(self, scopes: I) -> ScopedVariable
    where
        I: IntoIterator<Item = S>,
        S: Into<Scopes>,
    {
        let mut scopes: IndexSet<_> = scopes.into_iter().flat_map(Into::into).collect();
        scopes.sort();
        ScopedVariable { var: self, scopes }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ScopedVariable {
    var: Variable,
    scopes: IndexSet<Scope>,
}

impl Ord for ScopedVariable {
    fn cmp(&self, other: &Self) -> Ordering {
        self.var.cmp(&other.var)
    }
}

impl PartialOrd for ScopedVariable {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ScopedVariable {
    fn eq(&self, other: &Self) -> bool {
        self.var == other.var
    }
}

impl Eq for ScopedVariable {}

impl Hash for ScopedVariable {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.var.hash(state);
    }
}

impl fmt::Display for ScopedVariable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.var)
    }
}

impl Borrow<Variable> for ScopedVariable {
    fn borrow(&self) -> &Variable {
        &self.var
    }
}

impl AsRef<str> for ScopedVariable {
    fn as_ref(&self) -> &str {
        self.var.as_ref()
    }
}

impl From<&ScopedVariable> for Variable {
    fn from(value: &ScopedVariable) -> Self {
        value.var
    }
}

impl ScopedVariable {
    pub(crate) fn scopes(&self) -> &IndexSet<Scope> {
        &self.scopes
    }

    /// Externally exported to the package build environment.
    pub(crate) fn is_exported(&self) -> bool {
        use Variable::*;
        matches!(self.var, HOME | TMPDIR)
    }

    /// Variable value does not vary between phases.
    pub(crate) fn is_static(&self) -> bool {
        use Variable::*;
        !matches!(self.var, EBUILD_PHASE | EBUILD_PHASE_FUNC)
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
    use itertools::Itertools;
    use scallop::variables;
    use strum::IntoEnumIterator;

    use crate::config::Config;
    use crate::eapi::EAPIS_OFFICIAL;
    use crate::pkg::{BuildPackage, SourcePackage};
    use crate::shell::BuildData;

    use super::*;

    #[test]
    fn set_and_export() {
        use crate::shell::scope::Scope::*;

        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let all_scopes: IndexSet<_> = Scopes::All.into_iter().collect();

        for eapi in EAPIS_OFFICIAL.iter() {
            for var in Variable::iter() {
                for scope in &all_scopes {
                    match scope {
                        Global => {
                            let data = indoc::formatdoc! {r#"
                                EAPI={eapi}
                                DESCRIPTION="testing {var} global scope"
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
                                    "EAPI {eapi}: ${var} not set globally"
                                );
                            } else {
                                assert!(
                                    variables::optional(var).is_none(),
                                    "EAPI {eapi}: ${var} shouldn't be set globally"
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

                            let external = if eapi.env().get(&var).is_some_and(|v| v.is_exported())
                            {
                                "yes"
                            } else {
                                ""
                            };

                            let data = indoc::formatdoc! {r#"
                                EAPI={eapi}
                                DESCRIPTION="testing {var} exporting"
                                SLOT=0
                                {phase}() {{
                                    # verify internal export
                                    if [[ -n "{exported}" ]]; then
                                        [[ -v {var} ]] || die "EAPI {eapi}: \${var} not exported in {phase}"
                                    else
                                        [[ -v {var} ]] && die "EAPI {eapi}: \${var} shouldn't be exported in {phase}"
                                    fi

                                    # verify external export
                                    var={var}
                                    if [[ -n "{external}" ]]; then
                                        [[ "${{!var@a}}" == *x* ]] || die "EAPI {eapi}: \${var} should be exported externally"
                                    else
                                        [[ "${{!var@a}}" == *x* ]] && die "EAPI {eapi}: \${var} shouldn't be exported externally"
                                    fi

                                    :
                                }}
                            "#};
                            let pkg = t.create_pkg_from_str("cat/pkg-1", &data).unwrap();
                            pkg.source().unwrap();
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

    #[test]
    fn state() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let data = indoc::indoc! {r#"
            EAPI=8
            DESCRIPTION="testing environment state handling"
            SLOT=0

            VARIABLE_GLOBAL="a"

            src_compile() {
                VARIABLE_GLOBAL="b"
                VARIABLE_DEFAULT="c"
                export VARIABLE_EXPORTED="d"
                local VARIABLE_LOCAL="e"
            }

            src_install() {
                [[ ${VARIABLE_GLOBAL} == "b" ]] \
                    || die "broken env saving for globals"

                [[ ${VARIABLE_DEFAULT} == "c" ]] \
                    || die "broken env saving for default"

                [[ ${VARIABLE_EXPORTED} == "d" ]] \
                    || die "broken env saving for exported"

                [[ $(printenv VARIABLE_EXPORTED ) == "d" ]] \
                    || die "broken env saving for exported"

                [[ -z ${VARIABLE_LOCAL} ]] \
                    || die "broken env saving for locals"
            }
        "#};
        let pkg = t.create_pkg_from_str("cat/pkg-1", data).unwrap();
        BuildData::from_pkg(&pkg);
        pkg.build().unwrap();
    }

    #[test]
    fn vars_ebuild_phase() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        for eapi in EAPIS_OFFICIAL.iter() {
            // generate phase tests
            let phases = eapi.phases()
                .iter()
                .map(|phase| {
                    let short = phase.short_name();
                    indoc::formatdoc! {r#"
                    {phase}() {{
                        [[ $EBUILD_PHASE == "{short}" ]] || die "invalid EBUILD_PHASE value: $EBUILD_PHASE"
                        [[ $EBUILD_PHASE_FUNC == "{phase}" ]] || die "invalid EBUILD_PHASE_FUNC value: $EBUILD_PHASE_FUNC"
                    }}
                    "#}
                })
                .join("\n");

            let data = indoc::formatdoc! {r#"
                EAPI={eapi}
                DESCRIPTION="testing EBUILD_PHASE(_FUNC) variables"
                SLOT=0
                {phases}
            "#};
            let pkg = t.create_pkg_from_str("cat/pkg-1", &data).unwrap();
            pkg.source().unwrap();
            for phase in eapi.phases() {
                let r = phase.run();
                assert!(r.is_ok(), "EAPI {eapi}: failed running {phase}: {}", r.unwrap_err());
            }
        }
    }

    #[test]
    fn vars_pkg() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        for eapi in EAPIS_OFFICIAL.iter() {
            // generate phase tests
            let phases = eapi
                .phases()
                .iter()
                .map(|phase| {
                    indoc::formatdoc! {r#"
                    {phase}() {{
                        test_vars phase
                    }}
                    "#}
                })
                .join("\n");

            let data = indoc::formatdoc! {r#"
                EAPI={eapi}
                DESCRIPTION="testing package-related variables"
                SLOT=0

                test_vars() {{
                    [[ $CATEGORY == "cat" ]] || die "$1 scope: invalid CATEGORY value: $CATEGORY"
                    [[ $P == "pkg-1" ]] || die "$1 scope: invalid P value: $P"
                    [[ $PF == "pkg-1-r2" ]] || die "$1 scope: invalid PF value: $PF"
                    [[ $PN == "pkg" ]] || die "$1 scope: invalid PN value: $PN"
                    [[ $PR == "r2" ]] || die "$1 scope: invalid PR value: $PR"
                    [[ $PV == "1" ]] || die "$1 scope: invalid PV value: $PV"
                    [[ $PVR == "1-r2" ]] || die "$1 scope: invalid PVR value: $PVR"
                }}

                test_vars global

                {phases}
            "#};
            let pkg = t.create_pkg_from_str("cat/pkg-1-r2", &data).unwrap();
            pkg.source().unwrap();
            for phase in eapi.phases() {
                let r = phase.run();
                assert!(r.is_ok(), "EAPI {eapi}: failed running {phase}: {}", r.unwrap_err());
            }
        }
    }
}
