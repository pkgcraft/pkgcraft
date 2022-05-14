use peg;

use super::version::ParsedVersion;
use super::{Blocker, Operator, ParsedAtom};
use crate::eapi::Eapi;

peg::parser! {
    pub(crate) grammar pkg() for str {
        // Categories must not begin with a hyphen, dot, or plus sign.
        pub(crate) rule category() -> &'input str
            = s:$(quiet!{
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '.' | '-']*
            } / expected!("category name")
            ) { s }

        // Packages must not begin with a hyphen or plus sign and must not end in a
        // hyphen followed by anything matching a version.
        pub(crate) rule package() -> &'input str
            = s:$(quiet!{
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
                (['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_'] / ("-" !version()))*
            } / expected!("package name")
            ) { s }

        rule version_suffix() -> (&'input str, Option<&'input str>)
            = suffix:$("alpha" / "beta" / "pre" / "rc" / "p") ver:$(['0'..='9']+)? {?
                Ok((suffix, ver))
            }

        // TODO: figure out how to return string slice instead of positions
        // Related issue: https://github.com/kevinmehall/rust-peg/issues/283
        pub(crate) rule version() -> ParsedVersion<'input>
            = start:position!() numbers:$(['0'..='9']+) ++ "." letter:['a'..='z']?
                suffixes:("_" s:version_suffix() ++ "_" {s})?
                end_base:position!() revision:revision()? end:position!()
            { ParsedVersion { start, end_base, end, numbers, letter, suffixes, revision } }

        rule revision() -> &'input str
            = "-r" s:$(quiet!{['0'..='9']+} / expected!("revision"))
            { s }

        // Slot names must not begin with a hyphen, dot, or plus sign.
        rule slot_name() -> &'input str
            = s:$(quiet!{
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '.' | '-']*
            } / expected!("slot name")
            ) { s }

        rule slot(eapi: &'static Eapi) -> (&'input str, Option<&'input str>)
            = slot:slot_name() subslot:subslot(eapi)? {
                (slot, subslot)
            }

        rule slot_str(eapi: &'static Eapi) -> (Option<&'input str>, Option<&'input str>, Option<&'input str>)
            = op:$("*" / "=") {?
                if !eapi.has("slot_ops") {
                    return Err("slot operators are supported in >= EAPI 5");
                }
                Ok((None, None, Some(op)))
            } / slot:slot(eapi) op:$("=")? {?
                if op.is_some() && !eapi.has("slot_ops") {
                    return Err("slot operators are supported in >= EAPI 5");
                }
                Ok((Some(slot.0), slot.1, op))
            }

        rule slot_dep(eapi: &'static Eapi) -> (Option<&'input str>, Option<&'input str>, Option<&'input str>)
            = ":" slot_parts:slot_str(eapi) {?
                if !eapi.has("slot_deps") {
                    return Err("slot deps are supported in >= EAPI 1");
                }
                Ok(slot_parts)
            }

        rule blocks(eapi: &'static Eapi) -> Blocker
            = blocks:("!"*<1,2>) {?
                if eapi.has("blockers") {
                    match blocks[..] {
                        [_] => Ok(Blocker::Weak),
                        [_, _] => Ok(Blocker::Strong),
                        _ => Err("invalid blocker"),
                    }
                } else {
                    Err("blockers are supported in >= EAPI 2")
                }
            }

        rule useflag() -> &'input str
            = s:$(quiet!{
                ['a'..='z' | 'A'..='Z' | '0'..='9']
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '@' | '-']*
            } / expected!("useflag name")
            ) { s }

        rule use_dep(eapi: &'static Eapi) -> &'input str
            = s:$(quiet!{
                (useflag() use_dep_default(eapi)? ['=' | '?']?) /
                ("-" useflag() use_dep_default(eapi)?) /
                ("!" useflag() use_dep_default(eapi)? ['=' | '?'])
            } / expected!("use dep")
            ) { s }

        rule use_deps(eapi: &'static Eapi) -> Vec<&'input str>
            = "[" use_deps:use_dep(eapi) ++ "," "]" {?
                if eapi.has("use_deps") {
                    Ok(use_deps)
                } else {
                    Err("use deps are supported in >= EAPI 2")
                }
            }

        rule use_dep_default(eapi: &'static Eapi) -> &'input str
            = s:$("(+)" / "(-)") {?
                if eapi.has("use_dep_defaults") {
                    Ok(s)
                } else {
                    Err("use dep defaults are supported in >= EAPI 4")
                }
            }

        rule subslot(eapi: &'static Eapi) -> &'input str
            = "/" s:slot_name() {?
                if eapi.has("subslots") {
                    Ok(s)
                } else {
                    Err("subslots are supported in >= EAPI 5")
                }
            }

        rule pkg_dep() -> (&'input str, &'input str, Option<Operator>, Option<ParsedVersion<'input>>)
            = cat:category() "/" pkg:package() {
                (cat, pkg, None, None)
            } / op:$(("<" "="?) / "=" / "~" / (">" "="?))
                    cat:category() "/" pkg:package()
                    "-" ver:version() glob:"*"? {?
                let op = match (op, glob) {
                    ("<", None) => Ok(Operator::Less),
                    ("<=", None) => Ok(Operator::LessOrEqual),
                    ("=", None) => Ok(Operator::Equal),
                    ("=", Some(_)) => Ok(Operator::EqualGlob),
                    ("~", None) => match ver.revision {
                        None => Ok(Operator::Approximate),
                        Some(r) => Err("~ version operator can't be used with a revision"),
                    },
                    (">=", None) => Ok(Operator::GreaterOrEqual),
                    (">", None) => Ok(Operator::Greater),
                    _ => Err("invalid version operator"),
                }?;

                Ok((cat, pkg, Some(op), Some(ver)))
            }

        // repo must not begin with a hyphen and must also be a valid package name
        pub rule repo() -> &'input str
            = s:$(quiet!{
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
                (['a'..='z' | 'A'..='Z' | '0'..='9' | '_'] / ("-" !version()))*
            } / expected!("repo name")
            ) { s }

        rule repo_dep(eapi: &'static Eapi) -> &'input str
            = "::" repo:repo() {?
                if !eapi.has("repo_ids") {
                    return Err("repo deps aren't supported in EAPIs");
                }
                Ok(repo)
            }

        pub(crate) rule cpv() -> ParsedAtom<'input>
            = cat:category() "/" pkg:package() "-" ver:version() {
                ParsedAtom {
                    category: cat,
                    package: pkg,
                    block: None,
                    op: None,
                    version: Some(ver),
                    slot: None,
                    subslot: None,
                    slot_op: None,
                    use_deps: None,
                    repo: None,
                }
            }

        pub(crate) rule dep(eapi: &'static Eapi) -> ParsedAtom<'input>
            = block:blocks(eapi)? pkg_dep:pkg_dep() slot_dep:slot_dep(eapi)?
                    use_deps:use_deps(eapi)? repo:repo_dep(eapi)? {
                let (cat, pkg, op, version) = pkg_dep;
                let (slot, subslot, slot_op) = slot_dep.unwrap_or_default();
                ParsedAtom {
                    category: cat,
                    package: pkg,
                    block,
                    op,
                    version,
                    slot,
                    subslot,
                    slot_op,
                    use_deps,
                    repo,
                }
            }
    }
}

// provide public parsing functionality while converting error types
pub mod parse {
    use cached::{cached_key, SizedCache};

    use crate::atom::version::Version;
    use crate::atom::Atom;
    use crate::eapi::Eapi;
    use crate::peg::peg_error;
    use crate::Result;

    use super::pkg;

    #[inline]
    pub fn category(s: &str) -> Result<&str> {
        pkg::category(s).map_err(|e| peg_error(format!("invalid category name: {s:?}"), s, e))
    }

    #[inline]
    pub fn package(s: &str) -> Result<&str> {
        pkg::package(s).map_err(|e| peg_error(format!("invalid package name: {s:?}"), s, e))
    }

    cached_key! {
        VERSION_CACHE: SizedCache<String, Result<Version>> = SizedCache::with_size(1000);
        Key = { s.to_string() };
        fn version(s: &str) -> Result<Version> = {
            let parsed_version =
                pkg::version(s).map_err(|e| peg_error(format!("invalid version: {s:?}"), s, e))?;
            parsed_version.into_owned(s)
        }
    }

    #[inline]
    pub fn repo(s: &str) -> Result<&str> {
        pkg::repo(s).map_err(|e| peg_error(format!("invalid repo name: {s:?}"), s, e))
    }

    cached_key! {
        CPV_CACHE: SizedCache<String, Result<Atom>> = SizedCache::with_size(1000);
        Key = { s.to_string() };
        fn cpv(s: &str) -> Result<Atom> = {
            let parsed_cpv = pkg::cpv(s).map_err(|e| peg_error(format!("invalid cpv: {s:?}"), s, e))?;
            parsed_cpv.into_owned(s)
        }
    }

    cached_key! {
        ATOM_CACHE: SizedCache<(String, &Eapi), Result<Atom>> = SizedCache::with_size(1000);
        Key = { (s.to_string(), eapi) };
        fn dep(s: &str, eapi: &'static Eapi) -> Result<Atom> = {
            let parsed_atom = pkg::dep(s, eapi).map_err(|e| peg_error(format!("invalid atom: {s:?}"), s, e))?;
            parsed_atom.into_owned(s)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::atom::version::Version;
    use crate::atom::{Blocker, Operator};
    use crate::eapi;
    use crate::macros::opt_str;

    use super::parse;

    #[test]
    fn test_parse_versions() {
        // invalid deps
        for s in [
            // bad/missing category and/or package names
            "",
            "a",
            "a/+b",
            ".a/.b",
            // package names can't end in a hyphen followed by anything matching a version
            "a/b-0",
            "<a/b-1-1",
            // version operator with missing version
            "~a/b",
            "~a/b-r1",
            ">a/b",
            ">=a/b-r1",
            // '~' operator can't be used with a revision
            "~a/b-1-r1",
            // '*' suffix can only be used with the '=' operator
            ">=a/b-0*",
            "~a/b-0*",
            "a/b-0*",
            // '*' suffix can only be used with valid version strings
            "=a/b-0.*",
            "=a/b-0-r*",
        ] {
            for eapi in eapi::EAPIS.values() {
                let result = parse::dep(&s, eapi);
                assert!(result.is_err(), "{s:?} didn't fail");
            }
        }

        // convert &str to Option<Version>
        let version = |s| Version::from_str(s).ok();

        // good deps
        for (s, cat, pkg, op, ver) in [
            ("a/b", "a", "b", None, None),
            ("_/_", "_", "_", None, None),
            ("_.+-/_+-", "_.+-", "_+-", None, None),
            ("a/b-", "a", "b-", None, None),
            ("a/b-r100", "a", "b-r100", None, None),
            ("<a/b-r0-1-r2", "a", "b-r0", Some(Operator::Less), version("1-r2")),
            ("<=a/b-1", "a", "b", Some(Operator::LessOrEqual), version("1")),
            ("=a/b-1-r1", "a", "b", Some(Operator::Equal), version("1-r1")),
            ("=a/b-3*", "a", "b", Some(Operator::EqualGlob), version("3")),
            ("=a/b-3-r1*", "a", "b", Some(Operator::EqualGlob), version("3-r1")),
            ("~a/b-0", "a", "b", Some(Operator::Approximate), version("0")),
            (">=a/b-2", "a", "b", Some(Operator::GreaterOrEqual), version("2")),
            (">a/b-3-r0", "a", "b", Some(Operator::Greater), version("3-r0")),
        ] {
            for eapi in eapi::EAPIS.values() {
                let result = parse::dep(&s, eapi);
                assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
                let atom = result.unwrap();
                assert_eq!(atom.category, cat);
                assert_eq!(atom.package, pkg);
                assert_eq!(atom.op, op);
                assert_eq!(atom.version, ver);
                assert_eq!(format!("{atom}"), s);
            }
        }
    }

    #[test]
    fn test_parse_slots() {
        // invalid deps
        for slot in ["", "+", "+0", ".a", "-b", "a@b", "0/1"] {
            let s = format!("cat/pkg:{slot}");
            assert!(parse::dep(&s, &eapi::EAPI1).is_err(), "{s:?} didn't fail");
        }

        // good deps
        for (slot_str, slot) in [
            ("0", opt_str!("0")),
            ("a", opt_str!("a")),
            ("_", opt_str!("_")),
            ("_a", opt_str!("_a")),
            ("99", opt_str!("99")),
            ("aBc", opt_str!("aBc")),
            ("a+b_c.d-e", opt_str!("a+b_c.d-e")),
        ] {
            for eapi in eapi::EAPIS.values() {
                let s = format!("cat/pkg:{slot_str}");
                let result = parse::dep(&s, eapi);
                match eapi.has("slot_deps") {
                    false => assert!(result.is_err(), "{s:?} didn't fail"),
                    true => {
                        assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
                        let atom = result.unwrap();
                        assert_eq!(atom.slot, slot);
                        assert_eq!(format!("{atom}"), s);
                    }
                };
            }
        }
    }

    #[test]
    fn test_parse_blockers() {
        // invalid deps
        for s in ["!!!cat/pkg", "!cat/pkg-0", "!!cat/pkg-0-r1"] {
            assert!(parse::dep(&s, &eapi::EAPI2).is_err(), "{s:?} didn't fail");
        }

        // non-blocker
        let atom = parse::dep("cat/pkg", &eapi::EAPI2).unwrap();
        assert!(atom.block.is_none());

        // good deps
        for (s, block) in [
            ("!cat/pkg", Some(Blocker::Weak)),
            ("!cat/pkg:0", Some(Blocker::Weak)),
            ("!!cat/pkg", Some(Blocker::Strong)),
            ("!!<cat/pkg-1", Some(Blocker::Strong)),
        ] {
            for eapi in eapi::EAPIS.values() {
                let result = parse::dep(&s, eapi);
                match eapi.has("blockers") {
                    false => assert!(result.is_err(), "{s:?} didn't fail"),
                    true => {
                        assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
                        let atom = result.unwrap();
                        assert_eq!(atom.block, block);
                        assert_eq!(format!("{atom}"), s);
                    }
                };
            }
        }
    }

    #[test]
    fn test_parse_use_deps() {
        // invalid deps
        for use_deps in ["", "-", "-a?", "!a"] {
            let s = format!("cat/pkg[{use_deps}]");
            assert!(parse::dep(&s, &eapi::EAPI2).is_err(), "{s:?} didn't fail");
        }

        // good deps
        for use_deps in ["a", "!a?", "a,b", "-a,-b", "a?,b?", "a,b=,!c=,d?,!e?,-f"] {
            for eapi in eapi::EAPIS.values() {
                let s = format!("cat/pkg[{use_deps}]");
                let result = parse::dep(&s, eapi);
                match eapi.has("use_deps") {
                    false => assert!(result.is_err(), "{s:?} didn't fail"),
                    true => {
                        assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
                        let atom = result.unwrap();
                        let expected = use_deps.split(',').map(|s| s.to_string()).collect();
                        assert_eq!(atom.use_deps, Some(expected));
                        assert_eq!(format!("{atom}"), s);
                    }
                };
            }
        }
    }

    #[test]
    fn test_parse_use_dep_defaults() {
        // invalid deps
        for use_dep in ["(-)", "(+)", "a()", "a(?)", "a(b)", "a(-+)", "a(++)", "a((+))", "a(-)b"] {
            let s = format!("cat/pkg[{use_dep}]");
            assert!(parse::dep(&s, &eapi::EAPI4).is_err(), "{s:?} didn't fail");
        }

        // good deps
        for use_deps in ["a(+)", "-a(-)", "a(+)?,!b(-)?", "a(-)=,!b(+)="] {
            for eapi in eapi::EAPIS.values() {
                let s = format!("cat/pkg[{use_deps}]");
                let result = parse::dep(&s, eapi);
                match eapi.has("use_dep_defaults") {
                    false => assert!(result.is_err(), "{s:?} didn't fail"),
                    true => {
                        assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
                        let atom = result.unwrap();
                        let expected = use_deps.split(',').map(|s| s.to_string()).collect();
                        assert_eq!(atom.use_deps, Some(expected));
                        assert_eq!(format!("{atom}"), s);
                    }
                };
            }
        }
    }

    #[test]
    fn test_parse_subslots() {
        // invalid deps
        for slot in ["/", "/0", "0/", "0/+1", "0//1", "0/1/2"] {
            let s = format!("cat/pkg:{slot}");
            assert!(parse::dep(&s, &eapi::EAPI5).is_err(), "{s:?} didn't fail");
        }

        // good deps
        for (slot_str, slot, subslot, slot_op) in [
            ("0/1", opt_str!("0"), opt_str!("1"), None),
            ("a/b", opt_str!("a"), opt_str!("b"), None),
            ("A/B", opt_str!("A"), opt_str!("B"), None),
            ("_/_", opt_str!("_"), opt_str!("_"), None),
            ("0/a.b+c-d_e", opt_str!("0"), opt_str!("a.b+c-d_e"), None),
        ] {
            for eapi in eapi::EAPIS.values() {
                let s = format!("cat/pkg:{slot_str}");
                let result = parse::dep(&s, eapi);
                match eapi.has("slot_ops") {
                    false => assert!(result.is_err(), "{s:?} didn't fail"),
                    true => {
                        assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
                        let atom = result.unwrap();
                        assert_eq!(atom.slot, slot);
                        assert_eq!(atom.subslot, subslot);
                        assert_eq!(atom.slot_op, slot_op);
                        assert_eq!(format!("{atom}"), s);
                    }
                };
            }
        }
    }

    #[test]
    fn test_parse_slot_ops() {
        // invalid deps
        for slot in ["*0", "=0", "*=", "=="] {
            let s = format!("cat/pkg:{slot}");
            assert!(parse::dep(&s, &eapi::EAPI5).is_err(), "{s:?} didn't fail");
        }

        // good deps
        for (slot_str, slot, subslot, slot_op) in [
            ("*", None, None, opt_str!("*")),
            ("=", None, None, opt_str!("=")),
            ("0=", opt_str!("0"), None, opt_str!("=")),
            ("a=", opt_str!("a"), None, opt_str!("=")),
            ("0/1=", opt_str!("0"), opt_str!("1"), opt_str!("=")),
            ("a/b=", opt_str!("a"), opt_str!("b"), opt_str!("=")),
        ] {
            for eapi in eapi::EAPIS.values() {
                let s = format!("cat/pkg:{slot_str}");
                let result = parse::dep(&s, eapi);
                match eapi.has("slot_ops") {
                    false => assert!(result.is_err(), "{s:?} didn't fail"),
                    true => {
                        assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
                        let atom = result.unwrap();
                        assert_eq!(atom.slot, slot);
                        assert_eq!(atom.subslot, subslot);
                        assert_eq!(atom.slot_op, slot_op);
                        assert_eq!(format!("{atom}"), s);
                    }
                };
            }
        }
    }

    #[test]
    fn test_parse_repos() {
        // invalid repos
        for s in ["", "-repo", "repo-1", "repo@path"] {
            let result = parse::repo(&s);
            assert!(result.is_err(), "{s:?} didn't fail");
        }

        // repo deps
        for repo in ["_", "a", "repo", "repo_a", "repo-a"] {
            let s = format!("cat/pkg::{repo}");

            // repo ids aren't supported in official EAPIs
            for eapi in eapi::EAPIS_OFFICIAL.values() {
                assert!(parse::dep(&s, eapi).is_err(), "{s:?} didn't fail");
            }

            let result = parse::dep(&s, &eapi::EAPI_PKGCRAFT);
            assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
            let atom = result.unwrap();
            assert_eq!(atom.repo, opt_str!(repo));
            assert_eq!(format!("{atom}"), s);
        }
    }
}
