use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::pkgsh::write_stdout;

use super::{make_builtin, use_::run as use_, PHASE};

const LONG_DOC: &str = "\
Tests if a given USE flag is enabled and outputs a string related to its status.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let defaults = ["", "yes", "no", "", ""];
    let (flag, vals) = match args.len() {
        1 => Ok((&args[..1], defaults)),
        2..=5 => {
            // override default values with args
            let stop = args.len();
            let mut vals = defaults;
            vals[1..stop].clone_from_slice(&args[1..stop]);
            Ok((&args[..1], vals))
        }
        n => Err(Error::Base(format!("requires 1 to 5 args, got {n}"))),
    }?;

    match use_(flag)? {
        ExecStatus::Success => write_stdout!("{}{}", vals[1], vals[3])?,
        ExecStatus::Failure(_) => write_stdout!("{}{}", vals[2], vals[4])?,
    }

    Ok(ExecStatus::Success)
}

const USAGE: &str = "usex flag";
make_builtin!("usex", usex_builtin, run, LONG_DOC, USAGE, &[("5..", &[PHASE])]);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::macros::assert_err_re;
    use crate::pkgsh::{assert_stdout, get_build_mut, BuildData};

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
        let raw_pkg = t.create_ebuild("cat/pkg-1", &[]).unwrap();
        let pkg = raw_pkg.into_pkg().unwrap();
        BuildData::from_pkg(&pkg);

        assert_err_re!(usex(&["use"]), "^.* not in IUSE$");
    }

    #[test]
    fn enabled_and_disabled() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let raw_pkg = t.create_ebuild("cat/pkg-1", &["IUSE=use"]).unwrap();
        let pkg = raw_pkg.into_pkg().unwrap();
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
}
