use cached::{proc_macro::cached, SizedCache};
use winnow::prelude::*;

use crate::dep::cpn::Cpn;
use crate::dep::cpv::{Cpv, CpvOrDep};
use crate::dep::pkg::{Dep, Slot, SlotDep};
use crate::dep::uri::Uri;
use crate::dep::use_dep::UseDep;
use crate::dep::version::{Revision, Version};
use crate::dep::{Dependency, DependencySet};
use crate::eapi::Eapi;
use crate::pkg::ebuild::iuse::Iuse;
use crate::pkg::ebuild::keyword::Keyword;
use crate::Error;

pub fn category(s: &str) -> crate::Result<&str> {
    crate::parser::category_name
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

pub fn package(s: &str) -> crate::Result<&str> {
    crate::parser::package_name
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

pub(super) fn version(s: &str) -> crate::Result<Version> {
    crate::parser::version
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

pub(super) fn version_with_op(s: &str) -> crate::Result<Version> {
    crate::parser::version_with_op
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

pub fn license_name(s: &str) -> crate::Result<&str> {
    crate::parser::license_name
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

pub fn eclass_name(s: &str) -> crate::Result<&str> {
    crate::parser::eclass_name
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

pub fn slot(s: &str) -> crate::Result<Slot> {
    crate::parser::slot
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

pub(super) fn use_dep(s: &str) -> crate::Result<UseDep> {
    crate::parser::use_dep
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

pub(super) fn slot_dep(s: &str) -> crate::Result<SlotDep> {
    crate::parser::slot_dep
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

pub fn use_flag(s: &str) -> crate::Result<&str> {
    crate::parser::use_flag_name
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

pub(crate) fn iuse(s: &str) -> crate::Result<Iuse> {
    crate::parser::iuse
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

pub(crate) fn keyword(s: &str) -> crate::Result<Keyword> {
    crate::parser::keyword
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

pub(crate) fn revision(s: &str) -> crate::Result<Revision> {
    crate::parser::number
        .parse(s)
        .map(Revision)
        .map_err(|err| Error::ParseError(err.to_string()))
}

pub fn repo(s: &str) -> crate::Result<&str> {
    crate::parser::repository_name
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

/// Parse a string into a [`Cpv`].
pub(super) fn cpv(s: &str) -> crate::Result<Cpv> {
    crate::parser::cpv
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}
/// Parse a string into a [`CpvOrDep`].
pub(super) fn cpv_or_dep(s: &str) -> crate::Result<CpvOrDep> {
    crate::parser::cpv_or_dep
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

#[cached(
    ty = "SizedCache<(String, &Eapi), crate::Result<Dep>>",
    create = "{ SizedCache::with_size(1000) }",
    convert = r#"{ (s.to_string(), eapi) }"#
)]
pub(crate) fn dep(s: &str, eapi: &'static Eapi) -> crate::Result<Dep> {
    crate::parser::dep(eapi)
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}
pub(super) fn cpn(s: &str) -> crate::Result<Cpn> {
    crate::parser::cpn
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

pub(super) fn license_dependency_set(s: &str) -> crate::Result<DependencySet<String>> {
    crate::parser::license_dependency_set
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

pub(super) fn license_dependency(s: &str) -> crate::Result<Dependency<String>> {
    crate::parser::license_dependency
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

pub(super) fn src_uri_dependency_set(s: &str) -> crate::Result<DependencySet<Uri>> {
    crate::parser::src_uri_dependency_set
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

pub(super) fn src_uri_dependency(s: &str) -> crate::Result<Dependency<Uri>> {
    crate::parser::src_uri_dependency
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

pub(super) fn properties_dependency_set(s: &str) -> crate::Result<DependencySet<String>> {
    crate::parser::properties_dependency_set
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

pub(super) fn properties_dependency(s: &str) -> crate::Result<Dependency<String>> {
    crate::parser::properties_dependency
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

pub(super) fn required_use_dependency_set(s: &str) -> crate::Result<DependencySet<String>> {
    crate::parser::required_use_dependency_set
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

pub(super) fn required_use_dependency(s: &str) -> crate::Result<Dependency<String>> {
    crate::parser::required_use_dependency
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

pub(super) fn restrict_dependency_set(s: &str) -> crate::Result<DependencySet<String>> {
    crate::parser::restrict_dependency_set
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

pub(super) fn restrict_dependency(s: &str) -> crate::Result<Dependency<String>> {
    crate::parser::restrict_dependency
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

pub(super) fn package_dependency_set(
    s: &str,
    eapi: &'static Eapi,
) -> crate::Result<DependencySet<Dep>> {
    crate::parser::package_dependency_set(eapi)
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

pub(super) fn package_dependency(
    s: &str,
    eapi: &'static Eapi,
) -> crate::Result<Dependency<Dep>> {
    crate::parser::package_dependency(eapi)
        .parse(s)
        .map_err(|err| Error::ParseError(err.to_string()))
}

#[cfg(test)]
mod tests {
    use crate::{
        dep::{Blocker, SlotOperator},
        eapi::{EAPIS, EAPIS_OFFICIAL, EAPI_LATEST_OFFICIAL, EAPI_PKGCRAFT},
    };

    use super::*;

    #[test]
    fn slots() {
        for slot in ["0", "a", "_", "_a", "99", "aBc", "a+b_c.d-e"] {
            for eapi in &*EAPIS {
                let s = format!("cat/pkg:{slot}");
                let result = dep(&s, eapi);
                assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
                let d = result.unwrap();
                assert_eq!(d.slot(), Some(slot));
                assert_eq!(d.to_string(), s);
            }
        }
    }

    #[test]
    fn blockers() {
        let d = dep("cat/pkg", &EAPI_LATEST_OFFICIAL).unwrap();
        assert!(d.blocker().is_none());

        for (s, blocker) in [
            ("!cat/pkg", Some(Blocker::Weak)),
            ("!cat/pkg:0", Some(Blocker::Weak)),
            ("!!cat/pkg", Some(Blocker::Strong)),
            ("!!<cat/pkg-1", Some(Blocker::Strong)),
        ] {
            for eapi in &*EAPIS {
                let result = dep(s, eapi);
                assert!(
                    result.is_ok(),
                    "{s:?} failed for EAPI {eapi}: {}",
                    result.err().unwrap()
                );
                let d = result.unwrap();
                assert_eq!(d.blocker(), blocker);
                assert_eq!(d.to_string(), s);
            }
        }
    }

    #[test]
    fn use_deps() {
        for use_deps in ["a", "!a?", "a,b", "-a,-b", "a?,b?", "a,b=,!c=,d?,!e?,-f"] {
            for eapi in &*EAPIS {
                let s = format!("cat/pkg[{use_deps}]");
                let result = dep(&s, eapi);
                assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
                let d = result.unwrap();
                let expected = use_deps.parse().unwrap();
                assert_eq!(d.use_deps(), Some(&expected));
                assert_eq!(d.to_string(), s);
            }
        }
    }

    #[test]
    fn use_dep_defaults() {
        for use_deps in ["a(+)", "-a(-)", "a(+)?,!b(-)?", "a(-)=,!b(+)="] {
            for eapi in &*EAPIS {
                let s = format!("cat/pkg[{use_deps}]");
                let result = dep(&s, eapi);
                assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
                let d = result.unwrap();
                let expected = use_deps.parse().unwrap();
                assert_eq!(d.use_deps(), Some(&expected));
                assert_eq!(d.to_string(), s);
            }
        }
    }

    #[test]
    fn subslots() {
        for (slot_str, slot, subslot, slot_op) in [
            ("0/1", Some("0"), Some("1"), None),
            ("a/b", Some("a"), Some("b"), None),
            ("A/B", Some("A"), Some("B"), None),
            ("_/_", Some("_"), Some("_"), None),
            ("0/a.b+c-d_e", Some("0"), Some("a.b+c-d_e"), None),
        ] {
            for eapi in &*EAPIS {
                let s = format!("cat/pkg:{slot_str}");
                let result = dep(&s, eapi);
                assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
                let d = result.unwrap();
                assert_eq!(d.slot(), slot);
                assert_eq!(d.subslot(), subslot);
                assert_eq!(d.slot_op(), slot_op);
                assert_eq!(d.to_string(), s);
            }
        }
    }

    #[test]
    fn slot_ops() {
        for (slot_str, slot, subslot, slot_op) in [
            ("*", None, None, Some(SlotOperator::Star)),
            ("=", None, None, Some(SlotOperator::Equal)),
            ("0=", Some("0"), None, Some(SlotOperator::Equal)),
            ("a=", Some("a"), None, Some(SlotOperator::Equal)),
            ("0/1=", Some("0"), Some("1"), Some(SlotOperator::Equal)),
            ("a/b=", Some("a"), Some("b"), Some(SlotOperator::Equal)),
        ] {
            for eapi in &*EAPIS {
                let s = format!("cat/pkg:{slot_str}");
                let result = dep(&s, eapi);
                assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
                let d = result.unwrap();
                assert_eq!(d.slot(), slot);
                assert_eq!(d.subslot(), subslot);
                assert_eq!(d.slot_op(), slot_op);
                assert_eq!(d.to_string(), s);
            }
        }
    }

    #[test]
    fn repos() {
        for repo in ["_", "a", "repo", "repo_a", "repo-a"] {
            let s = format!("cat/pkg::{repo}");

            // repo ids aren't supported in official EAPIs
            for eapi in &*EAPIS_OFFICIAL {
                assert!(dep(&s, eapi).is_err(), "{s:?} didn't fail");
            }

            let result = dep(&s, &EAPI_PKGCRAFT);
            assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
            let d = result.unwrap();
            assert_eq!(d.repo(), Some(repo));
            assert_eq!(d.to_string(), s);
        }
    }

    #[test]
    fn license() {
        // invalid
        for s in ["(", ")", "( )", "( l1)", "| ( l1 )", "!use ( l1 )"] {
            assert!(license_dependency_set(s).is_err(), "{s:?} didn't fail");
            assert!(license_dependency(s).is_err(), "{s:?} didn't fail");
        }

        // empty set
        assert!(license_dependency_set("").unwrap().is_empty());

        // valid
        for (s, expected_flatten) in [
            // simple values
            ("v", vec!["v"]),
            ("v1 v2", vec!["v1", "v2"]),
            // groupings
            ("( v )", vec!["v"]),
            ("( v1 v2 )", vec!["v1", "v2"]),
            ("( v1 ( v2 ) )", vec!["v1", "v2"]),
            ("( ( v ) )", vec!["v"]),
            ("|| ( v )", vec!["v"]),
            ("|| ( v1 v2 )", vec!["v1", "v2"]),
            // conditionals
            ("u? ( v )", vec!["v"]),
            ("u? ( v1 v2 )", vec!["v1", "v2"]),
            // combinations
            ("v1 u? ( v2 )", vec!["v1", "v2"]),
            ("!u? ( || ( v1 v2 ) )", vec!["v1", "v2"]),
        ] {
            let depset = license_dependency_set(s).unwrap();
            assert_eq!(depset.to_string(), s);
            let flatten: Vec<_> = depset.iter_flatten().collect();
            assert_eq!(flatten, expected_flatten);
        }
    }

    #[test]
    fn src_uri() {
        // empty set
        assert!(src_uri_dependency_set("").unwrap().is_empty());

        for (s, expected_flatten) in [
            // invalid URIs are flagged when converting to fetchables
            ("http://", vec!["http://"]),
            ("https://a/uri/with/no/filename/", vec!["https://a/uri/with/no/filename/"]),
            // valid
            ("uri", vec!["uri"]),
            ("http://uri", vec!["http://uri"]),
            ("uri1 uri2", vec!["uri1", "uri2"]),
            ("( http://uri1 http://uri2 )", vec!["http://uri1", "http://uri2"]),
            ("u1? ( http://uri1 !u2? ( http://uri2 ) )", vec!["http://uri1", "http://uri2"]),
        ] {
            let depset = src_uri_dependency_set(s).unwrap();
            assert_eq!(depset.to_string(), s);
            let flatten: Vec<_> = depset.iter_flatten().map(|x| x.to_string()).collect();
            assert_eq!(flatten, expected_flatten);
        }

        // renames
        for (s, expected_flatten) in [
            ("http://uri -> file", vec!["http://uri -> file"]),
            ("u? ( http://uri -> file )", vec!["http://uri -> file"]),
        ] {
            let depset = src_uri_dependency_set(s).unwrap();
            assert_eq!(depset.to_string(), s);
            let flatten: Vec<_> = depset.iter_flatten().map(|x| x.to_string()).collect();
            assert_eq!(flatten, expected_flatten);
        }
    }

    #[test]
    fn required_use() {
        // invalid
        for s in ["(", ")", "( )", "( u)", "| ( u )", "|| ( )", "^^ ( )", "?? ( )"] {
            assert!(required_use_dependency_set(s).is_err(), "{s:?} didn't fail");
            assert!(required_use_dependency(s).is_err(), "{s:?} didn't fail");
        }

        // empty set
        assert!(required_use_dependency_set("").unwrap().is_empty());

        // valid
        for (s, expected_flatten) in [
            ("u", vec!["u"]),
            ("!u", vec!["u"]),
            ("u1 !u2", vec!["u1", "u2"]),
            ("( u )", vec!["u"]),
            ("( u1 u2 )", vec!["u1", "u2"]),
            ("|| ( u )", vec!["u"]),
            ("|| ( !u1 u2 )", vec!["u1", "u2"]),
            ("^^ ( u1 !u2 )", vec!["u1", "u2"]),
            ("u1? ( u2 )", vec!["u2"]),
            ("u1? ( u2 !u3 )", vec!["u2", "u3"]),
            ("!u1? ( || ( u2 u3 ) )", vec!["u2", "u3"]),
        ] {
            let depset = required_use_dependency_set(s).unwrap();
            assert_eq!(depset.to_string(), s);
            let flatten: Vec<_> = depset.iter_flatten().collect();
            assert_eq!(flatten, expected_flatten);
        }

        // ?? operator
        let (s, expected_flatten) = ("?? ( u1 u2 )", vec!["u1", "u2"]);
        let depset = required_use_dependency_set(s).unwrap();
        assert_eq!(depset.to_string(), s);
        let flatten: Vec<_> = depset.iter_flatten().collect();
        assert_eq!(flatten, expected_flatten);
    }

    #[test]
    fn package() {
        // invalid
        for s in
            ["(", ")", "( )", "|| ( )", "( a/b)", "| ( a/b )", "use ( a/b )", "!use ( a/b )"]
        {
            assert!(
                package_dependency_set(s, &EAPI_LATEST_OFFICIAL).is_err(),
                "{s:?} didn't fail"
            );
            assert!(
                package_dependency(s, &EAPI_LATEST_OFFICIAL).is_err(),
                "{s:?} didn't fail"
            );
        }

        // empty set
        assert!(package_dependency_set("", &EAPI_LATEST_OFFICIAL)
            .unwrap()
            .is_empty());

        // valid
        for (s, expected_flatten) in [
            ("a/b", vec!["a/b"]),
            ("a/b c/d", vec!["a/b", "c/d"]),
            ("( a/b c/d )", vec!["a/b", "c/d"]),
            ("u? ( a/b c/d )", vec!["a/b", "c/d"]),
            ("!u? ( a/b c/d )", vec!["a/b", "c/d"]),
            ("u1? ( a/b !u2? ( c/d ) )", vec!["a/b", "c/d"]),
        ] {
            let depset = package_dependency_set(s, &EAPI_LATEST_OFFICIAL).unwrap();
            assert_eq!(depset.to_string(), s);
            let flatten: Vec<_> = depset.iter_flatten().map(|x| x.to_string()).collect();
            assert_eq!(flatten, expected_flatten);
        }
    }

    #[test]
    fn properties() {
        // invalid
        for s in ["(", ")", "( )", "( v)", "| ( v )", "!use ( v )", "|| ( v )", "|| ( v1 v2 )"]
        {
            assert!(properties_dependency_set(s).is_err(), "{s:?} didn't fail");
            assert!(properties_dependency(s).is_err(), "{s:?} didn't fail");
        }

        // empty set
        assert!(properties_dependency_set("").unwrap().is_empty());

        // valid
        for (s, expected_flatten) in [
            // simple values
            ("v", vec!["v"]),
            ("v1 v2", vec!["v1", "v2"]),
            // groupings
            ("( v )", vec!["v"]),
            ("( v1 v2 )", vec!["v1", "v2"]),
            ("( v1 ( v2 ) )", vec!["v1", "v2"]),
            ("( ( v ) )", vec!["v"]),
            // conditionals
            ("u? ( v )", vec!["v"]),
            ("u? ( v1 v2 )", vec!["v1", "v2"]),
            ("!u? ( v1 v2 )", vec!["v1", "v2"]),
            // combinations
            ("v1 u? ( v2 )", vec!["v1", "v2"]),
        ] {
            let depset = properties_dependency_set(s).unwrap();
            assert_eq!(depset.to_string(), s);
            let flatten: Vec<_> = depset.iter_flatten().collect();
            assert_eq!(flatten, expected_flatten);
        }
    }

    #[test]
    fn restrict() {
        // invalid
        for s in ["(", ")", "( )", "( v)", "| ( v )", "!use ( v )", "|| ( v )", "|| ( v1 v2 )"]
        {
            assert!(restrict_dependency_set(s).is_err(), "{s:?} didn't fail");
            assert!(restrict_dependency(s).is_err(), "{s:?} didn't fail");
        }

        // empty set
        assert!(restrict_dependency_set("").unwrap().is_empty());

        // valid
        for (s, expected_flatten) in [
            // simple values
            ("v", vec!["v"]),
            ("v1 v2", vec!["v1", "v2"]),
            // groupings
            ("( v )", vec!["v"]),
            ("( v1 v2 )", vec!["v1", "v2"]),
            ("( v1 ( v2 ) )", vec!["v1", "v2"]),
            ("( ( v ) )", vec!["v"]),
            // conditionals
            ("u? ( v )", vec!["v"]),
            ("u? ( v1 v2 )", vec!["v1", "v2"]),
            ("!u? ( v1 v2 )", vec!["v1", "v2"]),
            // combinations
            ("v1 u? ( v2 )", vec!["v1", "v2"]),
        ] {
            let depset = restrict_dependency_set(s).unwrap();
            assert_eq!(depset.to_string(), s);
            let flatten: Vec<_> = depset.iter_flatten().collect();
            assert_eq!(flatten, expected_flatten);
        }
    }
}
