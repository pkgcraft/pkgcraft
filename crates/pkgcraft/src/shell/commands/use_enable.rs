use scallop::ExecStatus;

use super::_use_conf::use_conf;
use super::make_builtin;

const LONG_DOC: &str = "\
Returns --enable-${opt} and --disable-${opt} configure flags based on a given USE flag.";

#[doc = stringify!(LONG_DOC)]
fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    use_conf(args, "enable", "disable")
}

const USAGE: &str = "use_enable flag";
make_builtin!("use_enable", use_enable_builtin);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::eapi::EAPIS_OFFICIAL;
    use crate::io::stdout;
    use crate::shell::{get_build_mut, BuildData};
    use crate::test::assert_err_re;
    use crate::test::test_data;

    use super::super::{assert_invalid_args, cmd_scope_tests, use_enable};
    use super::*;

    cmd_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(use_enable, &[0, 4]);
    }

    #[test]
    fn empty_iuse_effective() {
        let data = test_data();
        let (_pool, repo) = data.ebuild_repo("commands").unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        BuildData::from_pkg(&pkg);
        assert_err_re!(use_enable(&["use"]), "^USE flag not in IUSE: use$");
    }

    #[test]
    fn enabled_and_disabled() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test", 0, None).unwrap();
        let _pool = config.pool();
        let pkg = temp.create_pkg("cat/pkg-1", &["IUSE=use"]).unwrap();
        BuildData::from_pkg(&pkg);

        // disabled
        assert!(use_enable(&["!use"]).is_err());
        for (args, status, expected) in [
            (vec!["use"], ExecStatus::Failure(1), "--disable-use"),
            (vec!["use", "opt"], ExecStatus::Failure(1), "--disable-opt"),
            (vec!["!use", "opt"], ExecStatus::Success, "--enable-opt"),
        ] {
            assert_eq!(use_enable(&args).unwrap(), status);
            assert_eq!(stdout().get(), expected);
        }

        // check EAPIs that support three arg variant
        for eapi in &*EAPIS_OFFICIAL {
            let pkg = temp
                .create_pkg("cat/pkg-1", &["IUSE=use", &format!("EAPI={eapi}")])
                .unwrap();
            BuildData::from_pkg(&pkg);

            for (args, status, expected) in [
                (&["use", "opt", "val"], ExecStatus::Failure(1), "--disable-opt=val"),
                (&["!use", "opt", "val"], ExecStatus::Success, "--enable-opt=val"),
            ] {
                assert_eq!(use_enable(args).unwrap(), status);
                assert_eq!(stdout().get(), expected);
            }
        }

        // enabled
        let pkg = temp.create_pkg("cat/pkg-1", &["IUSE=use"]).unwrap();
        BuildData::from_pkg(&pkg);
        get_build_mut().use_.insert("use".to_string());

        assert!(use_enable(&["!use"]).is_err());
        for (args, status, expected) in [
            (vec!["use"], ExecStatus::Success, "--enable-use"),
            (vec!["use", "opt"], ExecStatus::Success, "--enable-opt"),
            (vec!["!use", "opt"], ExecStatus::Failure(1), "--disable-opt"),
        ] {
            assert_eq!(use_enable(&args).unwrap(), status);
            assert_eq!(stdout().get(), expected);
        }

        // check EAPIs that support three arg variant
        for eapi in &*EAPIS_OFFICIAL {
            let pkg = temp
                .create_pkg("cat/pkg-1", &["IUSE=use", &format!("EAPI={eapi}")])
                .unwrap();
            BuildData::from_pkg(&pkg);
            get_build_mut().use_.insert("use".to_string());

            for (args, status, expected) in [
                (&["use", "opt", "val"], ExecStatus::Success, "--enable-opt=val"),
                (&["!use", "opt", "val"], ExecStatus::Failure(1), "--disable-opt=val"),
            ] {
                assert_eq!(use_enable(args).unwrap(), status);
                assert_eq!(stdout().get(), expected);
            }
        }
    }
}
