use scallop::{Error, ExecStatus};

use crate::shell::get_build_mut;

use super::make_builtin;

const LONG_DOC: &str = "\
Returns success if the USE flag argument is enabled, failure otherwise.
The return values are inverted if the flag name is prefixed with !.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let (negated, flag) = match args[..] {
        [flag] => flag
            .strip_prefix('!')
            .map_or_else(|| (false, flag), |s| (true, s)),
        _ => return Err(Error::Base(format!("requires 1 arg, got {}", args.len()))),
    };

    let build = get_build_mut();
    let pkg = build.ebuild_pkg();

    if !pkg.iuse_effective().contains(flag) {
        return Err(Error::Base(format!("USE flag {flag:?} not in IUSE")));
    }

    let mut ret = build.use_.contains(flag);
    if negated {
        ret = !ret;
    }

    Ok(ExecStatus::from(ret))
}

const USAGE: &str = "use flag";
make_builtin!("use", use_builtin);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::shell::BuildData;
    use crate::test::assert_err_re;
    use crate::test::TEST_DATA;

    use super::super::{assert_invalid_args, cmd_scope_tests, use_};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(use_, &[0, 2]);
    }

    #[test]
    fn empty_iuse_effective() {
        let repo = TEST_DATA.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        assert_err_re!(use_(&["use"]), "^.* not in IUSE$");
    }

    #[test]
    fn enabled_and_disabled() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test", 0, None).unwrap();
        let pkg = temp.create_pkg("cat/pkg-1", &["IUSE=use"]).unwrap();
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
