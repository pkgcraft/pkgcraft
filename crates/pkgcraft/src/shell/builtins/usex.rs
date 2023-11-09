use scallop::{Error, ExecStatus};

use crate::shell::write_stdout;

use super::{make_builtin, use_::run as use_};

const LONG_DOC: &str = "\
Tests if a given USE flag is enabled and outputs a string related to its status.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    // default output values
    let mut vals = ["yes", "no", "", ""];

    let flag = match args {
        [flag, args @ ..] if args.len() <= 4 => {
            // override default output values with args
            vals[0..args.len()].copy_from_slice(args);
            flag
        }
        _ => return Err(Error::Base(format!("requires 1 to 5 args, got {}", args.len()))),
    };

    match use_(&[flag])? {
        ExecStatus::Success => write_stdout!("{}{}", vals[0], vals[2])?,
        ExecStatus::Failure(_) => write_stdout!("{}{}", vals[1], vals[3])?,
    }

    Ok(ExecStatus::Success)
}

const USAGE: &str = "usex flag";
make_builtin!("usex", usex_builtin, run, LONG_DOC, USAGE, BUILTIN);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::eapi::EAPIS_OFFICIAL;
    use crate::macros::assert_err_re;
    use crate::pkg::BuildPackage;
    use crate::shell::{assert_stdout, get_build_mut, BuildData};

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as usex;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(usex, &[0, 6]);
    }

    #[test]
    fn empty_iuse_effective() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let pkg = t.create_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_pkg(&pkg);

        assert_err_re!(usex(&["use"]), "^.* not in IUSE$");
    }

    #[test]
    fn enabled_and_disabled() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let pkg = t.create_pkg("cat/pkg-1", &["IUSE=use"]).unwrap();
        BuildData::from_pkg(&pkg);

        // disabled
        for (args, expected) in [
            (vec!["use"], "no"),
            (vec!["use", "arg2", "arg3", "arg4", "arg5"], "arg3arg5"),
            (vec!["!use"], "yes"),
            (vec!["!use", "arg2", "arg3", "arg4", "arg5"], "arg2arg4"),
        ] {
            usex(&args).unwrap();
            assert_stdout!(expected);
        }

        // enabled
        get_build_mut().use_.insert("use".to_string());
        for (args, expected) in [
            (vec!["use"], "yes"),
            (vec!["use", "arg2", "arg3", "arg4", "arg5"], "arg2arg4"),
            (vec!["!use"], "no"),
            (vec!["!use", "arg2", "arg3", "arg4", "arg5"], "arg3arg5"),
        ] {
            usex(&args).unwrap();
            assert_stdout!(expected);
        }
    }

    #[test]
    fn subshell() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        for eapi in &*EAPIS_OFFICIAL {
            let data = indoc::formatdoc! {r#"
                EAPI={eapi}
                DESCRIPTION="subshell usex success"
                SLOT=0
                IUSE="use1 use2"
                pkg_setup() {{
                    local disabled=$(usex use1)
                    [[ ${{disabled}} == "no" ]] || die "usex failed disabled"
                    local enabled=$(usex use2)
                    [[ ${{enabled}} == "yes" ]] || die "usex failed enabled"
                }}
            "#};
            let pkg = t.create_pkg_from_str("cat/pkg-1", &data).unwrap();
            BuildData::from_pkg(&pkg);
            get_build_mut().use_.insert("use2".to_string());
            let r = pkg.build();
            assert!(r.is_ok(), "{}", r.unwrap_err());

            let data = indoc::formatdoc! {r#"
                EAPI={eapi}
                DESCRIPTION="subshell usex failure"
                SLOT=0
                IUSE="use1 use2"
                VAR=1
                pkg_setup() {{
                    local disabled=$(usex)
                    VAR=2
                }}
            "#};
            let pkg = t.create_pkg_from_str("cat/pkg-1", &data).unwrap();
            BuildData::from_pkg(&pkg);
            let r = pkg.build();
            assert_err_re!(r, "line 7: usex: error: requires 1 to 5 args, got 0$");
            assert_eq!(scallop::variables::optional("VAR").as_deref(), Some("1"));
        }
    }
}
