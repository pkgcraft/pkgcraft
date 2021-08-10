use peg;

use super::version::{Revision, Version};
use super::{Atom, Blocker, Operator};
use crate::eapi::Eapi;
use crate::macros::vec_str;

peg::parser! {
    pub grammar pkg() for str {
        // Categories must not begin with a hyphen, dot, or plus sign.
        pub rule category() -> &'input str
            = s:$(quiet!{
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '.' | '-']*
            } / expected!("category name")
            ) { s }

        // Packages must not begin with a hyphen or plus sign and must not end in a
        // hyphen followed by anything matching a version.
        pub rule package() -> &'input str
            = s:$(quiet!{
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
                (['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_'] / ("-" !version()))*
            } / expected!("package name")
            ) { s }

        pub rule version() -> (&'input str, Option<&'input str>)
            = ver:$(quiet!{
                ['0'..='9']+ ("." ['0'..='9']+)*
                ['a'..='z']?
                ("_" ("alpha" / "beta" / "pre" / "rc" / "p") ['0'..='9']*)*
            } / expected!("version")
            ) rev:revision()? { (ver, rev) }

        rule revision() -> &'input str
            = quiet!{"-r"} s:$(quiet!{['0'..='9']+} / expected!("revision"))
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
            = quiet!{":"} slot_parts:slot_str(eapi) {?
                if !eapi.has("slot_deps") {
                    return Err("slot deps are supported in >= EAPI 1");
                }
                Ok(slot_parts)
            }

        rule blocks(eapi: &'static Eapi) -> Blocker
            = blocks:(quiet!{"!"}*<1,2>) {?
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
            = s:$(
                (useflag() use_dep_default(eapi)? ['=' | '?']?) /
                (quiet!{"-"} useflag() use_dep_default(eapi)?) /
                (quiet!{"!"} useflag() use_dep_default(eapi)? ['=' | '?']) /
                expected!("use dep")
            ) { s }

        rule use_deps(eapi: &'static Eapi) -> Vec<&'input str>
            = quiet!{"["} use_deps:use_dep(eapi) ++ "," quiet!{"]"} {?
                if eapi.has("use_deps") {
                    Ok(use_deps)
                } else {
                    Err("use deps are supported in >= EAPI 2")
                }
            }

        rule use_dep_default(eapi: &'static Eapi) -> &'input str
            = s:$("(+)" / "(-)" / expected!("use dep default")) {?
                if eapi.has("use_dep_defaults") {
                    Ok(s)
                } else {
                    Err("use dep defaults are supported in >= EAPI 4")
                }
            }

        rule subslot(eapi: &'static Eapi) -> &'input str
            = quiet!{"/"} s:slot_name() {?
                if eapi.has("subslots") {
                    Ok(s)
                } else {
                    Err("subslots are supported in >= EAPI 5")
                }
            }

        rule pkg_dep() -> (&'input str, &'input str, Option<Operator>, Option<Version>)
            = cat:category() "/" pkg:package() {
                (cat, pkg, None, None)
            } / op:$(quiet!{("<" "="?) / "=" / "~" / (">" "="?)})
                    cat:category() "/" pkg:package()
                    quiet!{"-"} ver_rev:version() glob:"*"? {?
                let op = match (op, glob) {
                    ("<", None) => Ok(Operator::Less),
                    ("<=", None) => Ok(Operator::LessEqual),
                    ("=", None) => Ok(Operator::Equal),
                    ("=", Some(_)) => Ok(Operator::EqualGlob),
                    ("~", None) => Ok(Operator::Approximate),
                    (">=", None) => Ok(Operator::GreaterEqual),
                    (">", None) => Ok(Operator::Greater),
                    _ => Err("invalid version operator"),
                }?;

                // construct version struct
                let (ver, rev) = ver_rev;
                let version = Version {
                    base: ver.to_string(),
                    revision: Revision::new(rev),
                };

                Ok((cat, pkg, Some(op), Some(version)))
            }

        // repo must not begin with a hyphen and must also be a valid package name
        pub rule repo() -> &'input str
            = s:$(quiet!{
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
                (['a'..='z' | 'A'..='Z' | '0'..='9' | '_'] / ("-" !version()))*
            } / expected!("repo name")
            ) { s }

        rule repo_dep(eapi: &'static Eapi) -> &'input str
            = quiet!{"::"} repo:repo() {?
                if !eapi.has("repo_ids") {
                    return Err("repo deps aren't supported in EAPIs");
                }
                Ok(repo)
            }

        pub rule cpv() -> (&'input str, &'input str, &'input str)
            = cat:category() "/" pkg:package() quiet!{"-"} ver:$(version()) {
                (cat, pkg, ver)
            }

        pub rule dep(eapi: &'static Eapi) -> Atom
            = block:blocks(eapi)? pkg_dep:pkg_dep() slot_dep:slot_dep(eapi)?
                    use_deps:use_deps(eapi)? repo:repo_dep(eapi)? {
                let (cat, pkg, op, version) = pkg_dep;
                let (slot, subslot, slot_op) = slot_dep.unwrap_or_default();
                Atom {
                    category: cat.to_string(),
                    package: pkg.to_string(),
                    block,
                    op,
                    version,
                    slot: slot.map(|s| s.to_string()),
                    subslot: subslot.map(|s| s.to_string()),
                    slot_op: slot_op.map(|s| s.to_string()),
                    use_deps: use_deps.map(|u| vec_str!(u)),
                    repo: repo.map(|s| s.to_string()),
                }
            }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::atom::version::Version;
    use crate::atom::{Atom, Blocker, Operator, ParseError};
    use crate::eapi;
    use crate::macros::opt_str;

    use super::pkg::dep as parse;

    #[test]
    fn test_parse_versions() {
        let mut result: Result<Atom, ParseError>;

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
            // '*' suffix can only be used with the '=' operator
            ">=a/b-0*",
            "~a/b-0*",
            "a/b-0*",
            // '*' suffix can only be used with valid version strings
            "=a/b-0.*",
            "=a/b-0-r*",
        ] {
            for eapi in eapi::KNOWN_EAPIS.values() {
                result = parse(&s, eapi);
                assert!(result.is_err(), "{} didn't fail", s);
            }
        }

        // convert &str to Option<Version>
        let version = |s| Version::from_str(s).ok();

        // good deps
        let mut atom;
        for (s, cat, pkg, op, ver) in [
            ("a/b", "a", "b", None, None),
            ("_/_", "_", "_", None, None),
            ("_.+-/_+-", "_.+-", "_+-", None, None),
            ("a/b-", "a", "b-", None, None),
            ("a/b-r100", "a", "b-r100", None, None),
            (
                "<a/b-r0-1-r2",
                "a",
                "b-r0",
                Some(Operator::Less),
                version("1-r2"),
            ),
            ("<=a/b-1", "a", "b", Some(Operator::LessEqual), version("1")),
            (
                "=a/b-1-r1",
                "a",
                "b",
                Some(Operator::Equal),
                version("1-r1"),
            ),
            ("=a/b-3*", "a", "b", Some(Operator::EqualGlob), version("3")),
            (
                "=a/b-3-r1*",
                "a",
                "b",
                Some(Operator::EqualGlob),
                version("3-r1"),
            ),
            (
                "~a/b-0-r1",
                "a",
                "b",
                Some(Operator::Approximate),
                version("0-r1"),
            ),
            (
                ">=a/b-2",
                "a",
                "b",
                Some(Operator::GreaterEqual),
                version("2"),
            ),
            (
                ">a/b-3-r0",
                "a",
                "b",
                Some(Operator::Greater),
                version("3-r0"),
            ),
        ] {
            for eapi in eapi::KNOWN_EAPIS.values() {
                result = parse(&s, eapi);
                assert!(result.is_ok(), "{} failed: {}", s, result.err().unwrap());
                atom = result.unwrap();
                assert_eq!(atom.category, cat);
                assert_eq!(atom.package, pkg);
                assert_eq!(atom.op, op);
                assert_eq!(atom.version, ver);
                assert_eq!(format!("{}", atom), s);
            }
        }
    }

    #[test]
    fn test_parse_slots() {
        // invalid deps
        let mut s;
        for slot in ["", "+", "+0", ".a", "-b", "a@b", "0/1"] {
            s = format!("cat/pkg:{}", slot);
            assert!(parse(&s, &eapi::EAPI1).is_err(), "{} didn't fail", s);
        }

        // good deps
        let mut atom;
        let mut result: Result<Atom, ParseError>;
        for (slot_str, slot) in [
            ("0", opt_str!("0")),
            ("a", opt_str!("a")),
            ("_", opt_str!("_")),
            ("_a", opt_str!("_a")),
            ("99", opt_str!("99")),
            ("aBc", opt_str!("aBc")),
            ("a+b_c.d-e", opt_str!("a+b_c.d-e")),
        ] {
            for eapi in eapi::KNOWN_EAPIS.values() {
                s = format!("cat/pkg:{}", slot_str);
                result = parse(&s, eapi);
                match eapi.has("slot_deps") {
                    false => assert!(result.is_err(), "{} didn't fail", s),
                    true => {
                        assert!(result.is_ok(), "{} failed: {}", s, result.err().unwrap());
                        atom = result.unwrap();
                        assert_eq!(atom.slot, slot);
                        assert_eq!(format!("{}", atom), s);
                    }
                };
            }
        }
    }

    #[test]
    fn test_parse_blockers() {
        // invalid deps
        for s in ["!!!cat/pkg", "!cat/pkg-0", "!!cat/pkg-0-r1"] {
            assert!(parse(&s, &eapi::EAPI2).is_err(), "{} didn't fail", s);
        }

        // non-blocker
        let atom = parse("cat/pkg", &eapi::EAPI2).unwrap();
        assert!(atom.block.is_none());

        // good deps
        let mut atom: Atom;
        let mut result: Result<Atom, ParseError>;
        for (s, block) in [
            ("!cat/pkg", Some(Blocker::Weak)),
            ("!cat/pkg:0", Some(Blocker::Weak)),
            ("!!cat/pkg", Some(Blocker::Strong)),
            ("!!<cat/pkg-1", Some(Blocker::Strong)),
        ] {
            for eapi in eapi::KNOWN_EAPIS.values() {
                result = parse(&s, eapi);
                match eapi.has("blockers") {
                    false => assert!(result.is_err(), "{} didn't fail", s),
                    true => {
                        assert!(result.is_ok(), "{} failed: {}", s, result.err().unwrap());
                        atom = result.unwrap();
                        assert_eq!(atom.block, block);
                        assert_eq!(format!("{}", atom), s);
                    }
                };
            }
        }
    }

    #[test]
    fn test_parse_use_deps() {
        // invalid deps
        let mut s;
        for use_deps in ["", "-", "-a?", "!a"] {
            s = format!("cat/pkg[{}]", use_deps);
            assert!(parse(&s, &eapi::EAPI2).is_err(), "{} didn't fail", s);
        }

        // good deps
        let mut atom;
        let mut result: Result<Atom, ParseError>;
        for use_deps in ["a", "!a?", "a,b", "-a,-b", "a?,b?", "a,b=,!c=,d?,!e?,-f"] {
            for eapi in eapi::KNOWN_EAPIS.values() {
                s = format!("cat/pkg[{}]", use_deps);
                result = parse(&s, eapi);
                match eapi.has("use_deps") {
                    false => assert!(result.is_err(), "{} didn't fail", s),
                    true => {
                        assert!(result.is_ok(), "{} failed: {}", s, result.err().unwrap());
                        atom = result.unwrap();
                        let expected = use_deps.split(',').map(|s| s.to_string()).collect();
                        assert_eq!(atom.use_deps, Some(expected));
                        assert_eq!(format!("{}", atom), s);
                    }
                };
            }
        }
    }

    #[test]
    fn test_parse_use_dep_defaults() {
        // invalid deps
        let mut s;
        for use_dep in [
            "(-)", "(+)", "a()", "a(?)", "a(b)", "a(-+)", "a(++)", "a((+))", "a(-)b",
        ] {
            s = format!("cat/pkg[{}]", use_dep);
            assert!(parse(&s, &eapi::EAPI4).is_err(), "{} didn't fail", s);
        }

        // good deps
        let mut atom;
        let mut result: Result<Atom, ParseError>;
        for use_deps in ["a(+)", "-a(-)", "a(+)?,!b(-)?", "a(-)=,!b(+)="] {
            for eapi in eapi::KNOWN_EAPIS.values() {
                s = format!("cat/pkg[{}]", use_deps);
                result = parse(&s, eapi);
                match eapi.has("use_dep_defaults") {
                    false => assert!(result.is_err(), "{} didn't fail", s),
                    true => {
                        assert!(result.is_ok(), "{} failed: {}", s, result.err().unwrap());
                        atom = result.unwrap();
                        let expected = use_deps.split(',').map(|s| s.to_string()).collect();
                        assert_eq!(atom.use_deps, Some(expected));
                        assert_eq!(format!("{}", atom), s);
                    }
                };
            }
        }
    }

    #[test]
    fn test_parse_subslots() {
        // invalid deps
        let mut s;
        for slot in ["/", "/0", "0/", "0/+1", "0//1", "0/1/2"] {
            s = format!("cat/pkg:{}", slot);
            assert!(parse(&s, &eapi::EAPI5).is_err(), "{} didn't fail", s);
        }

        // good deps
        let mut atom;
        let mut result: Result<Atom, ParseError>;
        for (slot_str, slot, subslot, slot_op) in [
            ("0/1", opt_str!("0"), opt_str!("1"), None),
            ("a/b", opt_str!("a"), opt_str!("b"), None),
            ("A/B", opt_str!("A"), opt_str!("B"), None),
            ("_/_", opt_str!("_"), opt_str!("_"), None),
            ("0/a.b+c-d_e", opt_str!("0"), opt_str!("a.b+c-d_e"), None),
        ] {
            for eapi in eapi::KNOWN_EAPIS.values() {
                s = format!("cat/pkg:{}", slot_str);
                result = parse(&s, eapi);
                match eapi.has("slot_ops") {
                    false => assert!(result.is_err(), "{} didn't fail", s),
                    true => {
                        assert!(result.is_ok(), "{} failed: {}", s, result.err().unwrap());
                        atom = result.unwrap();
                        assert_eq!(atom.slot, slot);
                        assert_eq!(atom.subslot, subslot);
                        assert_eq!(atom.slot_op, slot_op);
                        assert_eq!(format!("{}", atom), s);
                    }
                };
            }
        }
    }

    #[test]
    fn test_parse_slot_ops() {
        // invalid deps
        let mut s;
        for slot in ["*0", "=0", "*=", "=="] {
            s = format!("cat/pkg:{}", slot);
            assert!(parse(&s, &eapi::EAPI5).is_err(), "{} didn't fail", s);
        }

        // good deps
        let mut atom;
        let mut result: Result<Atom, ParseError>;
        for (slot_str, slot, subslot, slot_op) in [
            ("*", None, None, opt_str!("*")),
            ("=", None, None, opt_str!("=")),
            ("0=", opt_str!("0"), None, opt_str!("=")),
            ("a=", opt_str!("a"), None, opt_str!("=")),
            ("0/1=", opt_str!("0"), opt_str!("1"), opt_str!("=")),
            ("a/b=", opt_str!("a"), opt_str!("b"), opt_str!("=")),
        ] {
            for eapi in eapi::KNOWN_EAPIS.values() {
                s = format!("cat/pkg:{}", slot_str);
                result = parse(&s, eapi);
                match eapi.has("slot_ops") {
                    false => assert!(result.is_err(), "{} didn't fail", s),
                    true => {
                        assert!(result.is_ok(), "{} failed: {}", s, result.err().unwrap());
                        atom = result.unwrap();
                        assert_eq!(atom.slot, slot);
                        assert_eq!(atom.subslot, subslot);
                        assert_eq!(atom.slot_op, slot_op);
                        assert_eq!(format!("{}", atom), s);
                    }
                };
            }
        }
    }

    #[test]
    fn test_parse_repos() {
        let mut s;
        let mut result: Result<Atom, ParseError>;

        // invalid deps
        for slot in ["", "-repo", "repo-1", "repo@path"] {
            s = format!("cat/pkg::{}", slot);
            result = parse(&s, &eapi::EAPI_EXTENDED);
            assert!(result.is_err(), "{} didn't fail", s);
        }

        let mut atom;

        // good deps
        for repo in ["_", "a", "repo", "repo_a", "repo-a"] {
            s = format!("cat/pkg::{}", repo);

            // repo ids aren't supported in regular EAPIs
            for eapi in eapi::KNOWN_EAPIS.values() {
                assert!(parse(&s, eapi).is_err(), "{} didn't fail", s);
            }

            result = parse(&s, &eapi::EAPI_EXTENDED);
            assert!(result.is_ok(), "{} failed: {}", s, result.err().unwrap());
            atom = result.unwrap();
            assert_eq!(atom.repo, opt_str!(repo));
            assert_eq!(format!("{}", atom), s);
        }
    }
}
