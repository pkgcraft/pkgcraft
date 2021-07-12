use peg;

use crate::atom::{Atom, Blocker, Operator};
use crate::atom::version::{Revision, Version};
use crate::eapi::Eapi;
use crate::macros::vec_str;

pub type ParseError = ::peg::error::ParseError<::peg::str::LineCol>;

peg::parser!{
    pub grammar pkg() for str {
        // EAPI 0

        rule version_op() -> Operator
            = s:$(quiet!{
                ("<" "="?) / "=" / "~" / (">" "="?)
            } / expected!("version operator")
            ) {?
                match s {
                    "<" => Ok(Operator::LT),
                    "<=" => Ok(Operator::LE),
                    "=" => Ok(Operator::EQ),
                    "~" => Ok(Operator::IR),
                    ">=" => Ok(Operator::GE),
                    ">" => Ok(Operator::GT),
                    _ => Err("invalid version operator"),
                }
            }

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
            ) rev:rev_str()? { (ver, rev) }

        rule rev_str() -> &'input str
            = quiet!{"-r"} rev:revision() { rev }

        pub rule revision() -> &'input str
            = s:$(quiet!{['0'..='9']+} / expected!("revision"))
            { s }

        // TODO: Ask rust-peg upstream for syntax that allows rules similar to pest's silent rule
        // support. This would allow skipping denoted strings from action results to avoid having
        // to use separate rules.
        rule ver_str() -> (Option<&'input str>, Option<&'input str>)
            = quiet!{"-"} ver_rev:version() { (Some(ver_rev.0), ver_rev.1) }

        // EAPI 1

        // Slot names must not begin with a hyphen, dot, or plus sign.
        rule slot_name() -> &'input str
            = s:$(quiet!{
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '.' | '-']*
            } / expected!("slot name")
            ) { s }

        rule slot(eapi: &'static Eapi) -> (&'input str, Option<&'input str>, Option<&'input str>)
            = slot:slot_name() subslot:subslot(eapi)? slot_op:$("=")? {
                (slot, subslot, slot_op)
            }

        rule slot_dep(eapi: &'static Eapi) -> (Option<&'input str>, Option<&'input str>, Option<&'input str>)
            = quiet!{":"} s:$("*" / "=" / slot(eapi) / expected!("slot dep")) {?
                if !eapi.has("slot_deps") {
                    return Err("slot deps are supported in >= EAPI 1");
                }

                let explode_slot = |mut s: &'input str| {
                    let (mut slot, mut subslot, mut op) = (None, None, None);
                    if s.ends_with("=") {
                        op = Some("=");
                        s = &s[..s.len()-1];
                    }
                    let slot_data: Vec<&str> = s.splitn(2, "/").collect();

                    if slot_data.len() == 1 {
                        slot = Some(slot_data[0]);
                    } else {
                        slot = Some(slot_data[0]);
                        subslot = Some(slot_data[1]);
                    }

                    (slot, subslot, op)
                };

                let (slot, subslot, slot_op) = match s {
                    "*" | "=" => (None, None, Some(s)),
                    _         => explode_slot(s),
                };

                if slot_op.is_some() && !eapi.has("slot_ops") {
                    // TODO: use custom error to allow dynamic string?
                    return Err("slot operators are supported in >= EAPI 5");
                }

                return Ok((slot, subslot, slot_op));
            }

        // EAPI 2

        rule blocks(eapi: &'static Eapi) -> Blocker
            = blocks:"!"*<1,2> {?
                if eapi.has("blockers") {
                    match blocks[..] {
                        [_] => return Ok(Blocker::Weak),
                        [_, _] => return Ok(Blocker::Strong),
                        _ => Err("invalid blocker"),
                    }
                } else {
                    // TODO: use custom error to allow dynamic string?
                    return Err("blockers are supported in >= EAPI 2");
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
                    return Ok(use_deps);
                } else {
                    // TODO: use custom error to allow dynamic string?
                    return Err("use deps are supported in >= EAPI 2");
                }
            }

        // EAPI 4

        rule use_dep_default(eapi: &'static Eapi) -> &'input str
            = s:$("(+)" / "(-)" / expected!("use dep default")) {?
                if eapi.has("use_dep_defaults") {
                    return Ok(s);
                } else {
                    // TODO: use custom error to allow dynamic string?
                    return Err("use dep defaults are supported in >= EAPI 4");
                }
            }

        // EAPI 5

        rule subslot(eapi: &'static Eapi) -> &'input str
            = quiet!{"/"} s:slot_name() {?
                if eapi.has("subslots") {
                    return Ok(s);
                } else {
                    // TODO: use custom error to allow dynamic string?
                    return Err("subslots are supported in >= EAPI 5");
                }
            }

        // public pkg atom parsing method
        pub rule atom(eapi: &'static Eapi) -> Atom
            = block:blocks(eapi)? op:version_op()? cat:category() "/" pkg:package()
                    ver_rev:ver_str()? slot_dep:slot_dep(eapi)? use_deps:use_deps(eapi)? {?
                // version operator existence must match version string existence
                if op.is_none() && !ver_rev.is_none() {
                    return Err("missing version operator");
                } else if !op.is_none() && ver_rev.is_none() {
                    return Err("missing version");
                }

                // unwrap conditionals
                let (ver, rev) = ver_rev.unwrap_or_default();
                let (slot, subslot, slot_op) = slot_dep.unwrap_or_default();

                // construct version struct
                let version = match ver {
                    None => None,
                    Some(s) => {
                        Some(Version {
                            base: s.to_string(),
                            revision: Revision::new(rev),
                        })
                    },
                };

                Ok(Atom {
                    category: cat.to_string(),
                    package: pkg.to_string(),
                    block: block,
                    op: op,
                    version: version,
                    slot: slot.and_then(|s| Some(s.to_string())),
                    subslot: subslot.and_then(|s| Some(s.to_string())),
                    slot_op: slot_op.and_then(|s| Some(s.to_string())),
                    use_deps: use_deps.and_then(|u| Some(vec_str!(u))),
                })
            }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::atom::{Atom, Blocker, Operator};
    use crate::atom::version::Version;
    use crate::eapi;
    use crate::macros::opt_str;

    use super::*;
    use super::pkg::atom as parse;

    #[test]
    fn test_parse_versions() {
        // invalid deps
        for s in [
                // bad/missing category and/or package names
                "", "a", "a/+b", ".a/.b",
                // package names can't end in a hyphen followed by anything matching a version
                "a/b-0", "<a/b-1-1",
                // version operator with missing version
                "~a/b", "~a/b-r1", ">a/b", ">=a/b-r1",
                ] {
            assert!(parse(&s, &eapi::EAPI0).is_err(), "{} didn't fail", s);
        }

        // convert &str to Option<Version>
        let version = |s| { Version::from_str(s).ok() };

        // good deps
        let mut atom;
        let mut result: Result<Atom, ParseError>;
        for (s, cat, pkg, op, ver) in [
                ("a/b", "a", "b", None, None),
                ("_/_", "_", "_", None, None),
                ("_.+-/_+-", "_.+-", "_+-", None, None),
                ("a/b-", "a", "b-", None, None),
                ("a/b-r100", "a", "b-r100", None, None),
                ("<a/b-r0-1-r2", "a", "b-r0", Some(Operator::LT), version("1-r2")),
                ("<=a/b-1", "a", "b", Some(Operator::LE), version("1")),
                ("=a/b-1-r1", "a", "b", Some(Operator::EQ), version("1-r1")),
                ("~a/b-0-r1", "a", "b", Some(Operator::IR), version("0-r1")),
                (">=a/b-2", "a", "b", Some(Operator::GE), version("2")),
                (">a/b-3-r0", "a", "b", Some(Operator::GT), version("3-r0")),
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
                    },
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
                    },
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
                        let expected = use_deps.split(",").map(|s| s.to_string()).collect();
                        assert_eq!(atom.use_deps, Some(expected));
                        assert_eq!(format!("{}", atom), s);
                    },
                };
            }
        }
    }

    #[test]
    fn test_parse_use_dep_defaults() {
        // invalid deps
        let mut s;
        for use_deps in ["a()", "a(?)", "a(b)", "a(-+)", "a(++)"] {
            s = format!("cat/pkg[{}]", use_deps);
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
                        let expected = use_deps.split(",").map(|s| s.to_string()).collect();
                        assert_eq!(atom.use_deps, Some(expected));
                        assert_eq!(format!("{}", atom), s);
                    },
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
                    },
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
                    },
                };
            }
        }
    }
}
