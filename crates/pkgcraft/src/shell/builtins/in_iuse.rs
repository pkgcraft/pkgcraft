use scallop::builtins::ExecStatus;
use scallop::Error;

use crate::shell::get_build_mut;

use super::{make_builtin, Scopes::Phases};

const LONG_DOC: &str = "\
Returns success if the USE flag argument is found in IUSE_EFFECTIVE, failure otherwise.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let flag = match args.len() {
        1 => Ok(args[0]),
        n => Err(Error::Base(format!("requires 1 arg, got {n}"))),
    }?;

    let pkg = get_build_mut().pkg()?;
    Ok(ExecStatus::from(pkg.iuse_effective().contains(flag)))
}

const USAGE: &str = "in_iuse flag";
make_builtin!("in_iuse", in_iuse_builtin, run, LONG_DOC, USAGE, [("6..", [Phases])]);

#[cfg(test)]
mod tests {
    use scallop::builtins::ExecStatus;

    use crate::config::Config;
    use crate::shell::BuildData;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as in_iuse;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(in_iuse, &[0, 2]);
    }

    #[test]
    fn known_and_unknown() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let pkg = t.create_pkg("cat/pkg-1", &["IUSE=use"]).unwrap();
        BuildData::from_pkg(&pkg);

        // unknown
        assert_eq!(in_iuse(&["unknown"]).unwrap(), ExecStatus::Failure(1));

        // known
        assert_eq!(in_iuse(&["use"]).unwrap(), ExecStatus::Success);
    }
}
