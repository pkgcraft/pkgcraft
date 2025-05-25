use scallop::ExecStatus;

use super::make_builtin;
use super::use_;

// TODO: convert to clap parser
//const LONG_DOC: &str = "Deprecated synonym for use.";

fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    use_(args)
}

make_builtin!("useq", useq_builtin);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::shell::{BuildData, get_build_mut};
    use crate::test::assert_err_re;
    use crate::test::test_data;

    use super::super::{assert_invalid_cmd, cmd_scope_tests, useq};
    use super::*;

    cmd_scope_tests!("useq flag");

    #[test]
    fn invalid_args() {
        assert_invalid_cmd(useq, &[0, 2]);
    }

    #[test]
    fn empty_iuse_effective() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        assert_err_re!(useq(&["use"]), "^USE flag not in IUSE: use$");
    }

    #[test]
    fn enabled_and_disabled() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();
        temp.create_ebuild("cat/pkg-1", &["IUSE=use"]).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);

        // disabled
        assert_eq!(useq(&["use"]).unwrap(), ExecStatus::Failure(1));
        // inverted check
        assert_eq!(useq(&["!use"]).unwrap(), ExecStatus::Success);

        // enabled
        get_build_mut().use_.insert("use".to_string());
        // use flag is enabled
        assert_eq!(useq(&["use"]).unwrap(), ExecStatus::Success);
        // inverted check
        assert_eq!(useq(&["!use"]).unwrap(), ExecStatus::Failure(1));
    }
}
