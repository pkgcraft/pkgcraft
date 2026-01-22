use scallop::ExecStatus;

use super::_use_conf::use_conf;
use super::make_builtin;

// TODO: convert to clap parser
//const LONG_DOC: &str = "\
//Returns --with-${opt} and --without-${opt} configure flags based on a given USE flag.";

pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    use_conf(args, "with", "without")
}

make_builtin!("use_with", use_with_builtin);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::eapi::EAPIS_OFFICIAL;
    use crate::io::stdout;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::shell::{BuildData, get_build_mut};
    use crate::test::assert_err_re;
    use crate::test::test_data;

    use super::super::{assert_invalid_args, cmd_scope_tests, functions::use_with};
    use super::*;

    cmd_scope_tests!("use_with flag");

    #[test]
    fn invalid_args() {
        assert_invalid_args(use_with, &[0, 4]);
    }

    #[test]
    fn empty_iuse_effective() {
        let data = test_data();
        let repo = data.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        assert_err_re!(use_with(&["use"]), "^USE flag not in IUSE: use$");
    }

    #[test]
    fn enabled_and_disabled() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();

        temp.create_ebuild("cat/pkg-1", &["IUSE=use"]).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);

        // disabled
        assert!(use_with(&["!use"]).is_err());
        for (args, status, expected) in [
            (vec!["use"], ExecStatus::Failure(1), "--without-use"),
            (vec!["use", "opt"], ExecStatus::Failure(1), "--without-opt"),
            (vec!["!use", "opt"], ExecStatus::Success, "--with-opt"),
        ] {
            assert_eq!(use_with(&args).unwrap(), status);
            assert_eq!(stdout().get(), expected);
        }

        // check EAPIs that support three arg variant
        for eapi in &*EAPIS_OFFICIAL {
            temp.create_ebuild("cat/pkg-1", &["IUSE=use", &format!("EAPI={eapi}")])
                .unwrap();
            let pkg = repo.get_pkg("cat/pkg-1").unwrap();
            BuildData::from_pkg(&pkg);

            for (args, status, expected) in [
                (&["use", "opt", "val"], ExecStatus::Failure(1), "--without-opt=val"),
                (&["!use", "opt", "val"], ExecStatus::Success, "--with-opt=val"),
            ] {
                assert_eq!(use_with(args).unwrap(), status);
                assert_eq!(stdout().get(), expected);
            }
        }

        // enabled
        temp.create_ebuild("cat/pkg-1", &["IUSE=use"]).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        get_build_mut().use_.insert("use".to_string());

        assert!(use_with(&["!use"]).is_err());
        for (args, status, expected) in [
            (vec!["use"], ExecStatus::Success, "--with-use"),
            (vec!["use", "opt"], ExecStatus::Success, "--with-opt"),
            (vec!["!use", "opt"], ExecStatus::Failure(1), "--without-opt"),
        ] {
            assert_eq!(use_with(&args).unwrap(), status);
            assert_eq!(stdout().get(), expected);
        }

        // check EAPIs that support three arg variant
        for eapi in &*EAPIS_OFFICIAL {
            temp.create_ebuild("cat/pkg-1", &["IUSE=use", &format!("EAPI={eapi}")])
                .unwrap();
            let pkg = repo.get_pkg("cat/pkg-1").unwrap();
            BuildData::from_pkg(&pkg);
            get_build_mut().use_.insert("use".to_string());

            for (args, status, expected) in [
                (&["use", "opt", "val"], ExecStatus::Success, "--with-opt=val"),
                (&["!use", "opt", "val"], ExecStatus::Failure(1), "--without-opt=val"),
            ] {
                assert_eq!(use_with(args).unwrap(), status);
                assert_eq!(stdout().get(), expected);
            }
        }
    }
}
