use scallop::builtins::ExecStatus;

use super::_use_conf::use_conf;
use super::{make_builtin, PHASE};

const LONG_DOC: &str = "\
Returns --enable-${opt} and --disable-${opt} configure flags based on a given USE flag.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> scallop::Result<ExecStatus> {
    use_conf(args, "enable", "disable")
}

const USAGE: &str = "use_enable flag";
make_builtin!("use_enable", use_enable_builtin, run, LONG_DOC, USAGE, &[("..", &[PHASE])]);

#[cfg(test)]
mod tests {
    use scallop::builtins::ExecStatus;

    use crate::config::Config;
    use crate::eapi::{Feature, EAPIS_OFFICIAL};
    use crate::macros::assert_err_re;
    use crate::pkg::ebuild::Pkg;
    use crate::pkgsh::{assert_stdout, BuildData, BUILD_DATA};

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as use_enable;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(use_enable, &[0, 4]);

        for eapi in EAPIS_OFFICIAL
            .iter()
            .filter(|e| !e.has(Feature::UseConfArg))
        {
            BuildData::empty(eapi);
            assert_invalid_args(use_enable, &[3]);
        }
    }

    #[test]
    fn empty_iuse_effective() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();
        let (path, cpv) = t.create_ebuild("cat/pkg-1", &[]).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        BuildData::from_pkg(&pkg);

        assert_err_re!(use_enable(&["use"]), "^.* not in IUSE$");
    }

    #[test]
    fn enabled_and_disabled() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();
        let (path, cpv) = t.create_ebuild("cat/pkg-1", &["IUSE=use"]).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        BuildData::from_pkg(&pkg);

        // disabled
        assert!(use_enable(&["!use"]).is_err());
        for (args, status, expected) in [
            (vec!["use"], ExecStatus::Failure(1), "--disable-use"),
            (vec!["use", "opt"], ExecStatus::Failure(1), "--disable-opt"),
            (vec!["!use", "opt"], ExecStatus::Success, "--enable-opt"),
        ] {
            assert_eq!(use_enable(&args).unwrap(), status);
            assert_stdout!(expected);
        }

        // check EAPIs that support three arg variant
        for eapi in EAPIS_OFFICIAL.iter().filter(|e| e.has(Feature::UseConfArg)) {
            let (path, cpv) = t
                .create_ebuild("cat/pkg-1", &["IUSE=use", &format!("EAPI={eapi}")])
                .unwrap();
            let pkg = Pkg::new(path, cpv, &repo).unwrap();
            BuildData::from_pkg(&pkg);

            for (args, status, expected) in [
                (&["use", "opt", "val"], ExecStatus::Failure(1), "--disable-opt=val"),
                (&["!use", "opt", "val"], ExecStatus::Success, "--enable-opt=val"),
            ] {
                assert_eq!(use_enable(args).unwrap(), status);
                assert_stdout!(expected);
            }
        }

        // enabled
        BUILD_DATA.with(|d| {
            let (path, cpv) = t.create_ebuild("cat/pkg-1", &["IUSE=use"]).unwrap();
            let pkg = Pkg::new(path, cpv, &repo).unwrap();
            BuildData::from_pkg(&pkg);
            d.borrow_mut().use_.insert("use".to_string());

            assert!(use_enable(&["!use"]).is_err());
            for (args, status, expected) in [
                (vec!["use"], ExecStatus::Success, "--enable-use"),
                (vec!["use", "opt"], ExecStatus::Success, "--enable-opt"),
                (vec!["!use", "opt"], ExecStatus::Failure(1), "--disable-opt"),
            ] {
                assert_eq!(use_enable(&args).unwrap(), status);
                assert_stdout!(expected);
            }

            // check EAPIs that support three arg variant
            for eapi in EAPIS_OFFICIAL.iter().filter(|e| e.has(Feature::UseConfArg)) {
                let (path, cpv) = t
                    .create_ebuild("cat/pkg-1", &["IUSE=use", &format!("EAPI={eapi}")])
                    .unwrap();
                let pkg = Pkg::new(path, cpv, &repo).unwrap();
                BuildData::from_pkg(&pkg);
                d.borrow_mut().use_.insert("use".to_string());

                for (args, status, expected) in [
                    (&["use", "opt", "val"], ExecStatus::Success, "--enable-opt=val"),
                    (&["!use", "opt", "val"], ExecStatus::Failure(1), "--disable-opt=val"),
                ] {
                    assert_eq!(use_enable(args).unwrap(), status);
                    assert_stdout!(expected);
                }
            }
        });
    }
}
