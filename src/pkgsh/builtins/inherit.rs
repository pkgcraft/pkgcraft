use scallop::builtins::{builtin_level, ExecStatus};
use scallop::variables::{string_vec, unbind, ScopedVariable, Variable, Variables};
use scallop::{source, Error, Result};

use crate::pkgsh::BUILD_DATA;

use super::{make_builtin, Scope, ECLASS, GLOBAL};

const LONG_DOC: &str = "Sources the given list of eclasses.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Base("requires 1 or more args, got 0".into()));
    }

    let eclasses: Vec<_> = args.iter().map(|s| s.to_string()).collect();

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let mut eclass_var = ScopedVariable::new("ECLASS");
        let mut inherited_var = Variable::new("INHERITED");

        let eapi = d.borrow().eapi;
        d.borrow_mut().scope = Scope::Eclass;

        // Track direct ebuild inherits, note that this assumes the first level is via an ebuild
        // inherit, i.e. calling this function directly won't increment the value and thus won't
        // work as expected.
        if builtin_level() == 1 {
            d.borrow_mut().inherit.extend(eclasses.clone());
        }

        // skip eclasses that have already been inherited
        let eclasses = eclasses
            .into_iter()
            .filter(|s| !d.borrow().inherited.contains(s));

        for eclass in eclasses {
            // unset metadata keys that incrementally accumulate
            for var in eapi.incremental_keys() {
                unbind(var)?;
            }

            eclass_var.bind(&eclass, None, None)?;
            let path = d
                .borrow()
                .repo
                .eclasses()
                .get(&eclass)
                .map(|p| p.to_string())
                .ok_or_else(|| Error::Base(format!("unknown eclass: {eclass}")))?;
            if let Err(e) = source::file(&path) {
                let msg = format!("failed loading eclass: {eclass}: {e}");
                return Err(Error::Base(msg));
            }

            let mut d = d.borrow_mut();
            // append metadata keys that incrementally accumulate
            for var in eapi.incremental_keys() {
                if let Ok(data) = string_vec(var) {
                    let deque = d.get_deque(var);
                    deque.extend(data);
                }
            }

            inherited_var.append(&format!(" {eclass}"))?;
            d.inherited.insert(eclass);
        }

        // unset metadata keys that incrementally accumulate
        for var in eapi.incremental_keys() {
            unbind(var)?;
        }

        Ok(ExecStatus::Success)
    })
}

const USAGE: &str = "inherit eclass1 eclass2";
make_builtin!("inherit", inherit_builtin, run, LONG_DOC, USAGE, &[("0-", &[GLOBAL, ECLASS])]);

#[cfg(test)]
mod tests {
    use std::fs;

    use scallop::variables::bind;

    use crate::config::Config;
    use crate::macros::assert_err_re;
    use crate::pkg::ebuild::Pkg;

    use super::super::{assert_invalid_args, builtin_scope_tests};
    use super::run as inherit;
    use super::*;

    builtin_scope_tests!(USAGE);

    #[test]
    fn invalid_args() {
        assert_invalid_args(inherit, &[0]);
    }

    #[test]
    fn test_nonexistent() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (_t, repo) = config.temp_repo("test", 0).unwrap();

        BUILD_DATA.with(|d| {
            d.borrow_mut().repo = repo.clone();
            let r = inherit(&["nonexistent"]);
            assert_err_re!(r, r"^unknown eclass: nonexistent");
        });
    }

    #[test]
    fn test_source_failure() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        // create eclass
        let eclass = indoc::indoc! {r#"
            # stub eclass
            unknown_cmd
        "#};
        t.create_eclass("e1", eclass).unwrap();

        BUILD_DATA.with(|d| {
            d.borrow_mut().repo = repo.clone();
            let r = inherit(&["e1"]);
            assert_err_re!(r, r"^failed loading eclass: e1: unknown command: unknown_cmd$");
        });
    }

    #[test]
    fn test_single() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        // create eclass
        let eclass = indoc::indoc! {r#"
            # stub eclass
            [[ ${ECLASS} == e1 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e1", eclass).unwrap();

        BUILD_DATA.with(|d| {
            d.borrow_mut().repo = repo.clone();
            inherit(&["e1"]).unwrap();
            assert_eq!(string_vec("INHERITED").unwrap(), ["e1"]);
        });
    }

    #[test]
    fn test_multiple() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        // create eclasses
        let eclass = indoc::indoc! {r#"
            # stub eclass
            [[ ${ECLASS} == e1 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e1", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            # stub eclass
            [[ ${ECLASS} == e2 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e2", eclass).unwrap();

        BUILD_DATA.with(|d| {
            d.borrow_mut().repo = repo.clone();
            inherit(&["e1", "e2"]).unwrap();
            assert_eq!(string_vec("INHERITED").unwrap(), ["e1", "e2"]);
        });
    }

    #[test]
    fn test_nested_single() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        // create eclasses
        let eclass = indoc::indoc! {r#"
            # stub eclass
            [[ ${ECLASS} == e1 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e1", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            # stub eclass
            inherit e1
            [[ ${ECLASS} == e2 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e2", eclass).unwrap();

        BUILD_DATA.with(|d| {
            d.borrow_mut().repo = repo.clone();
            inherit(&["e2"]).unwrap();
            assert_eq!(string_vec("INHERITED").unwrap(), ["e1", "e2"]);
        });
    }

    #[test]
    fn test_nested_multiple() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        // create eclasses
        let eclass = indoc::indoc! {r#"
            # stub eclass
            [[ ${ECLASS} == e1 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e1", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            # stub eclass
            inherit e1
            [[ ${ECLASS} == e2 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e2", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            # stub eclass
            inherit e2
            [[ ${ECLASS} == e3 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e3", eclass).unwrap();

        BUILD_DATA.with(|d| {
            d.borrow_mut().repo = repo.clone();
            inherit(&["e3"]).unwrap();
            assert_eq!(string_vec("INHERITED").unwrap(), ["e1", "e2", "e3"]);
        });
    }

    #[test]
    fn test_pkg_env() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        // create eclass
        let eclass = indoc::indoc! {r#"
            # stub eclass
            [[ ${ECLASS} == e1 ]] || die "\$ECLASS isn't correct"
        "#};
        t.create_eclass("e1", eclass).unwrap();

        let data = indoc::indoc! {r#"
            inherit e1
            DESCRIPTION="testing for eclass env transit"
            SLOT=0
            [[ -z ${ECLASS} ]] || die "\$ECLASS shouldn't be defined"
            [[ -n ${INHERITED} ]] || die "\$INHERITED should be defined"
        "#};
        let path = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        Pkg::new(&path, &repo).unwrap();
    }

    #[test]
    fn test_skip_reinherits() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        bind("TEMP_FILE", temp_file.path().to_string_lossy(), None, None).unwrap();

        // create eclasses
        let eclass = indoc::indoc! {r#"
            # stub eclass
            echo e1 >> ${TEMP_FILE}
        "#};
        t.create_eclass("e1", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            # stub eclass
            inherit e1
            echo e2 >> ${TEMP_FILE}
        "#};
        t.create_eclass("e2", eclass).unwrap();

        BUILD_DATA.with(|d| {
            d.borrow_mut().repo = repo.clone();
            inherit(&["e1", "e2"]).unwrap();
            let inherits = fs::read_to_string(temp_file.path()).unwrap();
            let inherits: Vec<_> = inherits.split_whitespace().collect();
            assert_eq!(inherits, ["e1", "e2"]);
        });
    }
}
