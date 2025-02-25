use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt;
use std::hash::{Hash, Hasher};

use scallop::variables::{bind, unbind, Attr};
use scallop::ExecStatus;
use strum::{AsRefStr, Display, EnumIter, EnumString};

use super::scope::{EbuildScope, Scope};

#[derive(AsRefStr, Display, EnumIter, EnumString, Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[strum(serialize_all = "UPPERCASE")]
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
pub enum Variable {
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

    // internal only
    DOCDESTTREE,
    EXEDESTTREE,
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
    pub(crate) fn internal<I>(self, scopes: I) -> BuildVariable
    where
        I: IntoIterator,
        I::Item: Into<EbuildScope>,
    {
        BuildVariable {
            var: self,
            allowed: scopes.into_iter().map(Into::into).collect(),
            external: false,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BuildVariable {
    var: Variable,
    allowed: HashSet<EbuildScope>,
    external: bool,
}

impl Ord for BuildVariable {
    fn cmp(&self, other: &Self) -> Ordering {
        self.var.cmp(&other.var)
    }
}

impl PartialOrd for BuildVariable {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for BuildVariable {
    fn eq(&self, other: &Self) -> bool {
        self.var == other.var
    }
}

impl Eq for BuildVariable {}

impl Hash for BuildVariable {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.var.hash(state);
    }
}

impl fmt::Display for BuildVariable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.var)
    }
}

impl Borrow<Variable> for BuildVariable {
    fn borrow(&self) -> &Variable {
        &self.var
    }
}

impl AsRef<str> for BuildVariable {
    fn as_ref(&self) -> &str {
        self.var.as_ref()
    }
}

impl From<&BuildVariable> for Variable {
    fn from(value: &BuildVariable) -> Self {
        value.var
    }
}

impl From<Variable> for BuildVariable {
    fn from(value: Variable) -> Self {
        BuildVariable {
            var: value,
            allowed: Default::default(),
            external: false,
        }
    }
}

impl BuildVariable {
    /// Enable externally exporting the variable.
    pub(crate) fn external(mut self) -> Self {
        self.external = true;
        self
    }

    /// Determine if the variable is exported to a given scope.
    pub(crate) fn exported(&self, scope: &Scope) -> bool {
        self.allowed.iter().any(|x| x == scope)
    }

    /// Variable value does not vary between phases.
    pub(crate) fn is_static(&self) -> bool {
        !matches!(self.var, Variable::EBUILD_PHASE | Variable::EBUILD_PHASE_FUNC)
    }

    pub(crate) fn bind(&self, value: &str) -> scallop::Result<ExecStatus> {
        let attrs = if self.external {
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
    use crate::pkg::{Build, Source};
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::shell::BuildData;

    use super::*;

    #[test]
    fn set_and_export() {
        use crate::shell::scope::Scope::*;

        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();
        let all_scopes: Vec<_> = EbuildScope::All.into_iter().collect();

        for eapi in &*EAPIS_OFFICIAL {
            for var in Variable::iter() {
                for scope in &all_scopes {
                    match scope {
                        Global => {
                            let data = indoc::formatdoc! {r#"
                                EAPI={eapi}
                                DESCRIPTION="testing {var} global scope"
                                SLOT=0
                            "#};
                            temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
                            let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
                            raw_pkg.source().unwrap();
                            if eapi.env().get(&var).is_some_and(|v| v.exported(scope)) {
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
                            let internal = if eapi
                                .env()
                                .get(&var)
                                .is_some_and(|v| v.exported(scope) || v.exported(&Global))
                            {
                                "yes"
                            } else {
                                ""
                            };

                            let external = if eapi.env().get(&var).is_some_and(|v| v.external)
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
                                    # run default phase if it exists
                                    nonfatal default

                                    # verify internal export
                                    if [[ -n "{internal}" ]]; then
                                        [[ -v {var} ]] || die "EAPI {eapi}: \${var} not internally exported in {phase}"
                                    else
                                        [[ -v {var} ]] && die "EAPI {eapi}: \${var} shouldn't be internally exported in {phase}"
                                    fi

                                    # verify external export
                                    var={var}
                                    if [[ -n "{external}" ]]; then
                                        [[ "${{!var@a}}" == *x* ]] || die "EAPI {eapi}: \${var} should be externally exported"
                                    else
                                        [[ "${{!var@a}}" == *x* ]] && die "EAPI {eapi}: \${var} shouldn't be externally exported"
                                    fi

                                    :
                                }}
                            "#};
                            temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
                            let pkg = repo.get_pkg("cat/pkg-1").unwrap();
                            pkg.source().unwrap();
                            let phase = eapi.phases().get(phase).unwrap();
                            phase.run().unwrap();

                            BuildData::from_pkg(&pkg);
                            pkg.build().unwrap();
                            if !eapi.env().get(&var).is_some_and(|v| v.exported(&Global)) {
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
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();
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
        temp.create_ebuild_from_str("cat/pkg-1", data).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        pkg.build().unwrap();
    }

    #[test]
    fn vars_ebuild_phase() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        for eapi in &*EAPIS_OFFICIAL {
            // generate phase tests
            let phases = eapi.phases()
                .iter()
                .map(|phase| {
                    let name = phase.name();
                    indoc::formatdoc! {r#"
                    {phase}() {{
                        # run default phase if it exists
                        nonfatal default

                        [[ $EBUILD_PHASE == "{name}" ]] || die "invalid EBUILD_PHASE value: $EBUILD_PHASE"
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
            temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
            let pkg = repo.get_pkg("cat/pkg-1").unwrap();
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
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        for eapi in &*EAPIS_OFFICIAL {
            // generate phase tests
            let phases = eapi
                .phases()
                .iter()
                .map(|phase| {
                    indoc::formatdoc! {r#"
                    {phase}() {{
                        # run default phase if it exists
                        nonfatal default

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
            temp.create_ebuild_from_str("cat/pkg-1-r2", &data).unwrap();
            let pkg = repo.get_pkg("cat/pkg-1-r2").unwrap();
            pkg.source().unwrap();
            for phase in eapi.phases() {
                let r = phase.run();
                assert!(r.is_ok(), "EAPI {eapi}: failed running {phase}: {}", r.unwrap_err());
            }
        }
    }
}
