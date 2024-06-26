use scallop::{Error, ExecStatus};

use crate::shell::get_build_mut;

use super::make_builtin;

const LONG_DOC: &str = "\
Returns success if the USE flag argument is found in IUSE_EFFECTIVE, failure otherwise.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    let flag = match args {
        [flag] => flag,
        _ => return Err(Error::Base(format!("requires 1 arg, got {}", args.len()))),
    };

    let pkg = get_build_mut().ebuild_pkg()?;
    Ok(ExecStatus::from(pkg.iuse_effective().contains(*flag)))
}

const USAGE: &str = "in_iuse flag";
make_builtin!("in_iuse", in_iuse_builtin);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::shell::BuildData;

    use super::super::{assert_invalid_args, cmd_scope_tests, in_iuse};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(in_iuse, &[0, 2]);
    }

    #[test]
    fn known_and_unknown() {
        let mut config = Config::default();
        let repo = config.temp_repo("test", 0, None).unwrap();
        let pkg = repo.create_pkg("cat/pkg-1", &["IUSE=use"]).unwrap();
        BuildData::from_pkg(&pkg);

        // unknown
        assert_eq!(in_iuse(&["unknown"]).unwrap(), ExecStatus::Failure(1));

        // known
        assert_eq!(in_iuse(&["use"]).unwrap(), ExecStatus::Success);
    }
}
