use once_cell::sync::Lazy;
use scallop::builtins::{make_builtin, ExecStatus};
use scallop::variables::{array_to_vec, string_vec, unbind, ScopedVariable, Variable, Variables};
use scallop::{source, Error, Result};

use crate::macros::build_from_paths;
use crate::pkgsh::BUILD_DATA;
use crate::repo::Repository;

use super::{PkgBuiltin, Scope, ECLASS, GLOBAL};

const LONG_DOC: &str = "Sources the given list of eclasses.";

#[doc = stringify!(LONG_DOC)]
pub(crate) fn run(args: &[&str]) -> Result<ExecStatus> {
    if args.is_empty() {
        return Err(Error::Builtin("requires 1 or more args, got 0".into()));
    }

    let eclasses: Vec<_> = args.iter().map(|s| s.to_string()).collect();

    BUILD_DATA.with(|d| -> Result<ExecStatus> {
        let mut eclass_var = ScopedVariable::new("ECLASS");
        let mut inherited_var = Variable::new("INHERITED");
        let eapi = d.borrow().eapi;
        // enable eclass builtins
        let _builtins = eapi.scoped_builtins(Scope::Eclass)?;

        // track direct ebuild inherits
        if let Ok(source) = array_to_vec("BASH_SOURCE") {
            if source.len() == 1 && source[0].ends_with(".ebuild") {
                d.borrow_mut().inherit.extend(eclasses.clone());
            }
        }

        for eclass in eclasses {
            // don't re-inherit eclasses
            if d.borrow().inherited.contains(&eclass) {
                continue;
            }

            // unset metadata keys that incrementally accumulate
            for var in eapi.incremental_keys() {
                unbind(var)?;
            }

            eclass_var.bind(&eclass, None, None)?;
            let path =
                build_from_paths!(d.borrow().repo.path(), "eclass", format!("{eclass}.eclass"));
            if let Err(e) = source::file(&path) {
                let msg = format!("failed loading eclass: {eclass}: {e}");
                return Err(Error::Builtin(msg));
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

make_builtin!("inherit", inherit_builtin, run, LONG_DOC, "inherit eclass1 eclass2");

pub(super) static PKG_BUILTIN: Lazy<PkgBuiltin> =
    Lazy::new(|| PkgBuiltin::new(BUILTIN, &[("0-", &[GLOBAL, ECLASS])]));

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::macros::assert_err_re;

    use super::super::assert_invalid_args;
    use super::run as inherit;
    use super::*;

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
            assert_err_re!(r, r"^failed loading eclass: nonexistent");
        });
    }

    #[test]
    fn test_single() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        // create eclass
        let eclass = indoc::indoc! {r#"
            # stub eclass
        "#};
        t.create_eclass("e1", eclass).unwrap();

        // nonexistent eclass
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
        "#};
        t.create_eclass("e1", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            # stub eclass
        "#};
        t.create_eclass("e2", eclass).unwrap();

        // nonexistent eclass
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
        "#};
        t.create_eclass("e1", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            # stub eclass
            inherit e1
        "#};
        t.create_eclass("e2", eclass).unwrap();

        // nonexistent eclass
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
        "#};
        t.create_eclass("e1", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            # stub eclass
            inherit e1
        "#};
        t.create_eclass("e2", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            # stub eclass
            inherit e2
        "#};
        t.create_eclass("e3", eclass).unwrap();

        // nonexistent eclass
        BUILD_DATA.with(|d| {
            d.borrow_mut().repo = repo.clone();
            inherit(&["e3"]).unwrap();
            assert_eq!(string_vec("INHERITED").unwrap(), ["e1", "e2", "e3"]);
        });
    }
}
