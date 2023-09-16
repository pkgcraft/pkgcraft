use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::eapi::Feature;
use crate::shell::{get_build_mut, write_stdout};

use super::{make_builtin, use_::run as use_, Scopes::Phases};

const LONG_DOC: &str = "\
The same as use, but also prints the flag name if the condition is met.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let (flag, output) = match args.len() {
        1 => {
            let output = args[0].strip_prefix('!').unwrap_or(args[0]);
            Ok((&args[..1], output))
        }
        2 => {
            if get_build_mut().eapi().has(Feature::UsevTwoArgs) {
                Ok((&args[..1], args[1]))
            } else {
                Err(Error::Base("requires 1 arg, got 2".into()))
            }
        }
        n => Err(Error::Base(format!("requires 1 or 2 args, got {n}"))),
    }?;

    let ret = use_(flag)?;
    if bool::from(&ret) {
        write_stdout!("{output}")?;
    }

    Ok(ret)
}

const USAGE: &str = "usev flag";
make_builtin!("usev", usev_builtin, run, LONG_DOC, USAGE, [("..", [Phases])]);

#[cfg(test)]
mod tests {
    use scallop::builtins::ExecStatus;

    use crate::config::Config;
    use crate::eapi::{Feature, EAPIS_OFFICIAL};
    use crate::macros::assert_err_re;
    use crate::shell::{assert_stdout, get_build_mut, BuildData};

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as usev;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(usev, &[0, 3]);

        for eapi in EAPIS_OFFICIAL
            .iter()
            .filter(|e| !e.has(Feature::UsevTwoArgs))
        {
            BuildData::empty(eapi);
            assert_invalid_args(usev, &[2]);
        }
    }

    #[test]
    fn empty_iuse_effective() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let pkg = t.create_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_pkg(&pkg);

        assert_err_re!(usev(&["use"]), "^.* not in IUSE$");
    }

    #[test]
    fn enabled_and_disabled() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let pkg = t.create_pkg("cat/pkg-1", &["IUSE=use"]).unwrap();
        BuildData::from_pkg(&pkg);

        // disabled
        for (args, status, expected) in
            [(&["use"], ExecStatus::Failure(1), ""), (&["!use"], ExecStatus::Success, "use")]
        {
            assert_eq!(usev(args).unwrap(), status);
            assert_stdout!(expected);
        }

        // check EAPIs that support two arg variant
        for eapi in EAPIS_OFFICIAL
            .iter()
            .filter(|e| e.has(Feature::UsevTwoArgs))
        {
            let pkg = t
                .create_pkg("cat/pkg-1", &["IUSE=use", &format!("EAPI={eapi}")])
                .unwrap();
            BuildData::from_pkg(&pkg);

            for (args, status, expected) in [
                (&["use", "out"], ExecStatus::Failure(1), ""),
                (&["!use", "out"], ExecStatus::Success, "out"),
            ] {
                assert_eq!(usev(args).unwrap(), status);
                assert_stdout!(expected);
            }
        }

        // enabled
        let pkg = t.create_pkg("cat/pkg-1", &["IUSE=use"]).unwrap();
        BuildData::from_pkg(&pkg);
        get_build_mut().use_.insert("use".to_string());

        for (args, status, expected) in
            [(&["use"], ExecStatus::Success, "use"), (&["!use"], ExecStatus::Failure(1), "")]
        {
            assert_eq!(usev(args).unwrap(), status);
            assert_stdout!(expected);
        }

        // check EAPIs that support two arg variant
        for eapi in EAPIS_OFFICIAL
            .iter()
            .filter(|e| e.has(Feature::UsevTwoArgs))
        {
            let pkg = t
                .create_pkg("cat/pkg-1", &["IUSE=use", &format!("EAPI={eapi}")])
                .unwrap();
            BuildData::from_pkg(&pkg);
            get_build_mut().use_.insert("use".to_string());

            for (args, status, expected) in [
                (&["use", "out"], ExecStatus::Success, "out"),
                (&["!use", "out"], ExecStatus::Failure(1), ""),
            ] {
                assert_eq!(usev(args).unwrap(), status);
                assert_stdout!(expected);
            }
        }
    }
}
