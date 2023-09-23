use std::str::FromStr;

use scallop::builtins::ExecStatus;
use scallop::{functions, source, variables, Error};

use crate::shell::get_build_mut;
use crate::shell::phase::PhaseKind;

use super::{make_builtin, Scopes::Eclass};

const LONG_DOC: &str = "\
Export stub functions that call the eclass's functions, thereby making them default.
For example, if ECLASS=base and `EXPORT_FUNCTIONS src_unpack` is called the following
function is defined:

src_unpack() { base_src_unpack; }";

/// Create function aliases for EXPORT_FUNCTIONS calls.
pub(super) fn export_functions<I>(functions: I) -> scallop::Result<ExecStatus>
where
    I: IntoIterator<Item = (PhaseKind, String)>,
{
    for (phase, eclass) in functions {
        let func = format!("{eclass}_{phase}");
        if functions::find(&func).is_some() {
            source::string(format!("{phase}() {{ {func} \"$@\"; }}"))?;
        } else {
            return Err(Error::Base(format!("{eclass}: undefined phase function: {func}")));
        }
    }

    Ok(ExecStatus::Success)
}

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    let eclass = variables::required("ECLASS")?;
    let build = get_build_mut();
    let eapi = build.eapi();
    let phases = eapi.phases();

    for arg in args {
        let phase =
            PhaseKind::from_str(arg).map_err(|_| Error::Base(format!("invalid phase: {arg}")))?;

        if phases.contains(&phase) {
            build.export_functions.insert(phase, eclass.clone());
        } else {
            return Err(Error::Base(format!("{phase} phase undefined in EAPI {eapi}")));
        }
    }

    Ok(ExecStatus::Success)
}

const USAGE: &str = "EXPORT_FUNCTIONS src_configure src_compile";
make_builtin!(
    "EXPORT_FUNCTIONS",
    export_functions_builtin,
    run,
    LONG_DOC,
    USAGE,
    [("..", [Eclass])]
);

#[cfg(test)]
mod tests {
    use scallop::functions;
    use scallop::variables::optional;

    use crate::config::Config;
    use crate::macros::assert_err_re;
    use crate::pkg::SourcePackage;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as export_functions;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(export_functions, &[0]);
    }

    #[test]
    fn test_single() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // create eclass
        let eclass = indoc::indoc! {r#"
            # stub eclass
            EXPORT_FUNCTIONS src_compile

            e1_src_compile() {
                VAR=1
            }
        "#};
        t.create_eclass("e1", eclass).unwrap();

        let data = indoc::indoc! {r#"
            EAPI=8
            inherit e1
            DESCRIPTION="testing EXPORT_FUNCTIONS support"
            SLOT=0
        "#};
        let raw_pkg = t.create_raw_pkg_from_str("cat/pkg-1", data).unwrap();
        raw_pkg.source().unwrap();
        // execute eclass-defined function
        let mut func = functions::find("src_compile").unwrap();
        // verify the function runs
        assert!(optional("VAR").is_none());
        func.execute(&[]).unwrap();
        assert_eq!(optional("VAR").unwrap(), "1");
    }

    #[test]
    fn invalid_phase() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // create eclass
        let eclass = indoc::indoc! {r#"
            # stub eclass
            EXPORT_FUNCTIONS src_compile invalid_phase

            e1_src_compile() { :; }
            e1_invalid_phase() { :; }
        "#};
        t.create_eclass("e1", eclass).unwrap();

        let data = indoc::indoc! {r#"
            EAPI=8
            inherit e1
            DESCRIPTION="testing EXPORT_FUNCTIONS support"
            SLOT=0
        "#};
        let raw_pkg = t.create_raw_pkg_from_str("cat/pkg-1", data).unwrap();
        let r = raw_pkg.source();
        assert_err_re!(r, "invalid phase: invalid_phase$");
    }

    #[test]
    fn undefined_phase() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // create eclass
        let eclass = indoc::indoc! {r#"
            # stub eclass
            EXPORT_FUNCTIONS src_compile src_configure

            e1_src_compile() { :; }
        "#};
        t.create_eclass("e1", eclass).unwrap();

        let data = indoc::indoc! {r#"
            EAPI=8
            inherit e1
            DESCRIPTION="testing EXPORT_FUNCTIONS support"
            SLOT=0
        "#};
        let raw_pkg = t.create_raw_pkg_from_str("cat/pkg-1", data).unwrap();
        let r = raw_pkg.source();
        assert_err_re!(r, "e1: undefined phase function: e1_src_configure$");
    }
}
