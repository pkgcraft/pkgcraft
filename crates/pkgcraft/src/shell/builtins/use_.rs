use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::shell::get_build_mut;

use super::{make_builtin, Scopes::Phases};

const LONG_DOC: &str = "\
Returns success if the USE flag argument is enabled, failure otherwise.
The return values are inverted if the flag name is prefixed with !.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let (negated, flag) = match args.len() {
        1 => {
            if args[0].starts_with('!') {
                Ok((true, &args[0][1..]))
            } else {
                Ok((false, args[0]))
            }
        }
        n => Err(Error::Base(format!("requires 1 arg, got {n}"))),
    }?;

    let build = get_build_mut();

    if !build.pkg()?.iuse_effective().contains(flag) {
        return Err(Error::Base(format!("USE flag {flag:?} not in IUSE")));
    }

    let mut ret = build.use_.contains(flag);
    if negated {
        ret = !ret;
    }

    Ok(ExecStatus::from(ret))
}

const USAGE: &str = "use flag";
make_builtin!("use", use_builtin, run, LONG_DOC, USAGE, [("..", [Phases])]);

#[cfg(test)]
mod tests {
    use scallop::builtins::ExecStatus;

    use crate::config::Config;
    use crate::macros::assert_err_re;
    use crate::shell::{get_build_mut, BuildData};

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as use_;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(use_, &[0, 2]);
    }

    #[test]
    fn empty_iuse_effective() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let pkg = t.create_pkg("cat/pkg-1", &[]).unwrap();
        BuildData::from_pkg(&pkg);

        assert_err_re!(use_(&["use"]), "^.* not in IUSE$");
    }

    #[test]
    fn enabled_and_disabled() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let pkg = t.create_pkg("cat/pkg-1", &["IUSE=use"]).unwrap();
        BuildData::from_pkg(&pkg);

        // disabled
        assert_eq!(use_(&["use"]).unwrap(), ExecStatus::Failure(1));
        // inverted check
        assert_eq!(use_(&["!use"]).unwrap(), ExecStatus::Success);

        // enabled
        get_build_mut().use_.insert("use".to_string());
        // use flag is enabled
        assert_eq!(use_(&["use"]).unwrap(), ExecStatus::Success);
        // inverted check
        assert_eq!(use_(&["!use"]).unwrap(), ExecStatus::Failure(1));
    }
}
