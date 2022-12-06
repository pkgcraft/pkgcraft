use cached::{proc_macro::cached, SizedCache};

use crate::eapi::{Eapi, Feature};
use crate::peg::peg_error;
use crate::Error;

use super::version::{ParsedVersion, Suffix};
use super::{Atom, Blocker, ParsedAtom, SlotOperator, Version};

peg::parser! {
    grammar pkg() for str {
        // Categories must not begin with a hyphen, dot, or plus sign.
        pub(super) rule category() -> (usize, usize)
            = start:position!() s:$(quiet!{
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '.' | '-']*
            } / expected!("category name")) end:position!()
            { (start, end) }

        // Packages must not begin with a hyphen or plus sign and must not end in a
        // hyphen followed by anything matching a version.
        pub(super) rule package() -> (usize, usize)
            = start:position!() s:$(quiet!{
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
                (['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_'] /
                 ("-" !(version() ("-" version())? ![_])))*
            } / expected!("package name")) end:position!()
            { (start, end) }

        rule version_suffix() -> Suffix
            = "_" suffix:$("alpha" / "beta" / "pre" / "rc" / "p") ver:$(['0'..='9']+)? {?
                let num = match ver {
                    None => None,
                    Some(s) => match s.parse() {
                        Err(_) => return Err("version suffix integer overflow"),
                        Ok(n) => Some(n),
                    }
                };
                let suffix = match suffix {
                    "alpha" => Suffix::Alpha,
                    "beta" => Suffix::Beta,
                    "pre" => Suffix::Pre,
                    "rc" => Suffix::Rc,
                    "p" => Suffix::P,
                    _ => panic!("invalid suffix"),
                };
                Ok(suffix(num))
            }

        // TODO: figure out how to return string slice instead of positions
        // Related issue: https://github.com/kevinmehall/rust-peg/issues/283
        pub(super) rule version() -> ParsedVersion<'input>
            = start:position!() numbers:$(['0'..='9']+) ++ "." letter:['a'..='z']?
                    suffixes:version_suffix()*
                    end_base:position!() revision:revision()? end:position!() {
                ParsedVersion {
                    start,
                    end,
                    base_end: end_base-start,
                    op: None,
                    numbers,
                    letter,
                    suffixes,
                    revision,
                }
            }

        pub(super) rule version_with_op() -> ParsedVersion<'input>
            = op:$(("<" "="?) / "=" / "~" / (">" "="?)) ver:version() glob:$("*")? {?
                ver.with_op(op, glob)
            }

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

        rule slot_str(eapi: &'static Eapi) -> (Option<&'input str>, Option<&'input str>, Option<SlotOperator>)
            = op:$("*" / "=") {?
                if !eapi.has(Feature::SlotOps) {
                    return Err("slot operators are supported in >= EAPI 5");
                }
                let op = match op {
                    "*" => SlotOperator::Star,
                    "=" => SlotOperator::Equal,
                    _ => return Err("invalid slot operator"),
                };
                Ok((None, None, Some(op)))
            } / slot:slot(eapi) op:$("=")? {?
                if op.is_some() && !eapi.has(Feature::SlotOps) {
                    return Err("slot operators are supported in >= EAPI 5");
                }
                let op = op.map(|_| SlotOperator::Equal);
                Ok((Some(slot.0), slot.1, op))
            }

        rule slot_dep(eapi: &'static Eapi) -> (Option<&'input str>, Option<&'input str>, Option<SlotOperator>)
            = ":" slot_parts:slot_str(eapi) {?
                if !eapi.has(Feature::SlotDeps) {
                    return Err("slot deps are supported in >= EAPI 1");
                }
                Ok(slot_parts)
            }

        rule blocker(eapi: &'static Eapi) -> Blocker
            = blocker:("!"*<1,2>) {?
                if eapi.has(Feature::Blockers) {
                    match blocker.len() {
                        1 => Ok(Blocker::Weak),
                        2 => Ok(Blocker::Strong),
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
                if eapi.has(Feature::UseDeps) {
                    Ok(use_deps)
                } else {
                    Err("use deps are supported in >= EAPI 2")
                }
            }

        rule use_dep_default(eapi: &'static Eapi) -> &'input str
            = s:$("(+)" / "(-)") {?
                if eapi.has(Feature::UseDepDefaults) {
                    Ok(s)
                } else {
                    Err("use dep defaults are supported in >= EAPI 4")
                }
            }

        rule subslot(eapi: &'static Eapi) -> &'input str
            = "/" s:slot_name() {?
                if eapi.has(Feature::Subslots) {
                    Ok(s)
                } else {
                    Err("subslots are supported in >= EAPI 5")
                }
            }

        // repo must not begin with a hyphen and must also be a valid package name
        pub(super) rule repo() -> &'input str
            = s:$(quiet!{
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
                (['a'..='z' | 'A'..='Z' | '0'..='9' | '_'] / ("-" !version()))*
            } / expected!("repo name")
            ) { s }

        rule repo_dep(eapi: &'static Eapi) -> &'input str
            = "::" repo:repo() {?
                if !eapi.has(Feature::RepoIds) {
                    return Err("repo deps aren't supported in EAPIs");
                }
                Ok(repo)
            }

        pub(super) rule cpv() -> ParsedAtom<'input>
            = cat:category() "/" pkg:package() "-" ver:version() {
                ParsedAtom {
                    category: cat,
                    package: pkg,
                    version: Some(ver),
                    ..Default::default()
                }
            }

        pub(super) rule cpv_with_op() -> (&'input str, usize, &'input str, Option<&'input str>)
            = op:$(("<" "="?) / "=" / "~" / (">" "="?)) start:position!() cpv:$([^'*']+) glob:$("*")?
            { (op, start, cpv, glob) }

        pub(super) rule cp() -> ((usize, usize), (usize, usize))
            = cat:category() "/" pkg:package() { (cat, pkg) }

        pub(super) rule dep(eapi: &'static Eapi) -> (&'input str, usize, ParsedAtom<'input>)
            = blocker:blocker(eapi)? start:position!() dep:$([^':' | '[']+) slot_dep:slot_dep(eapi)?
                    use_deps:use_deps(eapi)? repo:repo_dep(eapi)? {
                let (slot, subslot, slot_op) = slot_dep.unwrap_or_default();
                (dep, start, ParsedAtom {
                    blocker,
                    slot,
                    subslot,
                    slot_op,
                    use_deps,
                    repo,
                    ..Default::default()
                })
            }
    }
}

pub fn category(s: &str) -> crate::Result<()> {
    pkg::category(s).map_err(|e| peg_error(format!("invalid category name: {s}"), s, e))?;
    Ok(())
}

pub fn package(s: &str) -> crate::Result<()> {
    pkg::package(s).map_err(|e| peg_error(format!("invalid package name: {s}"), s, e))?;
    Ok(())
}

pub(crate) fn version_str(s: &str) -> crate::Result<ParsedVersion> {
    pkg::version(s).map_err(|e| peg_error(format!("invalid version: {s}"), s, e))
}

#[cached(
    type = "SizedCache<String, crate::Result<Version>>",
    create = "{ SizedCache::with_size(1000) }",
    convert = r#"{ s.to_string() }"#
)]
pub(crate) fn version(s: &str) -> crate::Result<Version> {
    version_str(s)?.into_owned(s)
}

pub(crate) fn version_with_op(s: &str) -> crate::Result<Version> {
    let ver =
        pkg::version_with_op(s).map_err(|e| peg_error(format!("invalid version: {s}"), s, e))?;
    ver.into_owned(s)
}

pub fn repo(s: &str) -> crate::Result<()> {
    pkg::repo(s).map_err(|e| peg_error(format!("invalid repo name: {s}"), s, e))?;
    Ok(())
}

pub(crate) fn cpv(s: &str) -> crate::Result<ParsedAtom> {
    pkg::cpv(s).map_err(|e| peg_error(format!("invalid cpv: {s}"), s, e))
}

pub(crate) fn dep_str<'a>(s: &'a str, eapi: &'static Eapi) -> crate::Result<ParsedAtom<'a>> {
    let (dep, mut start, mut atom) =
        pkg::dep(s, eapi).map_err(|e| peg_error(format!("invalid atom: {s}"), s, e))?;
    match pkg::cpv_with_op(dep) {
        Ok((op, cpv_start, cpv, glob)) => {
            let cpv_atom =
                pkg::cpv(cpv).map_err(|e| peg_error(format!("invalid atom: {s}"), cpv, e))?;
            start += cpv_start;
            atom.category = (start + cpv_atom.category.0, start + cpv_atom.category.1);
            atom.package = (start + cpv_atom.package.0, start + cpv_atom.package.1);
            let ver = cpv_atom.version.unwrap();
            atom.version = Some(
                ver.with_op(op, glob)
                    .map_err(|e| Error::InvalidValue(format!("invalid atom: {s}: {e}")))?,
            );
            atom.version_str = Some(cpv);
        }
        _ => {
            let (cat, pkg) =
                pkg::cp(dep).map_err(|e| peg_error(format!("invalid atom: {s}"), dep, e))?;
            atom.category = (start + cat.0, start + cat.1);
            atom.package = (start + pkg.0, start + pkg.1);
        }
    }

    Ok(atom)
}

#[cached(
    type = "SizedCache<(String, &Eapi), crate::Result<Atom>>",
    create = "{ SizedCache::with_size(1000) }",
    convert = r#"{ (s.to_string(), eapi) }"#
)]
pub(crate) fn dep(s: &str, eapi: &'static Eapi) -> crate::Result<Atom> {
    let atom = dep_str(s, eapi)?;
    atom.into_owned(s)
}

#[cfg(test)]
mod tests {
    use crate::eapi;
    use crate::macros::opt_str;
    use crate::test::AtomData;

    use super::*;

    #[test]
    fn test_parse() {
        let atoms = AtomData::load().unwrap();

        // invalid deps
        for (s, eapis) in atoms.invalid {
            let failing_eapis = eapi::range(eapis).expect("failed to parse EAPI range");
            // verify parse failures
            for eapi in &failing_eapis {
                let result = dep(&s, eapi);
                assert!(result.is_err(), "{s:?} didn't fail for EAPI={eapi}");
            }
            // verify parse successes
            for eapi in eapi::EAPIS.difference(&failing_eapis) {
                let result = dep(&s, eapi);
                assert!(result.is_ok(), "{s:?} failed for EAPI={eapi}");
            }
        }

        // valid deps
        for a in atoms.valid {
            let s = a.atom.as_str();
            let passing_eapis = eapi::range(&a.eapis).expect("failed to parse EAPI range");
            // verify parse successes
            for eapi in &passing_eapis {
                let result = dep(&s, eapi);
                assert!(result.is_ok(), "{s:?} failed for EAPI={eapi}");
                let atom = result.unwrap();
                assert_eq!(atom.category(), a.category, "{s:?} failed for EAPI={eapi}");
                assert_eq!(atom.package(), a.package, "{s:?} failed for EAPI={eapi}");
                assert_eq!(atom.blocker(), a.blocker, "{s:?} failed for EAPI={eapi}");
                assert_eq!(atom.version(), a.version.as_ref(), "{s:?} failed for EAPI={eapi}");
                assert_eq!(atom.revision(), a.revision.as_ref(), "{s:?} failed for EAPI={eapi}");
                assert_eq!(atom.slot(), a.slot.as_deref(), "{s:?} failed for EAPI={eapi}");
                assert_eq!(atom.subslot(), a.subslot.as_deref(), "{s:?} failed for EAPI={eapi}");
                assert_eq!(atom.slot_op(), a.slot_op, "{s:?} failed for EAPI={eapi}");
                assert_eq!(atom.use_deps(), a.use_deps.as_ref(), "{s:?} failed for EAPI={eapi}");
                assert_eq!(format!("{atom}"), s, "{s:?} failed for EAPI={eapi}");
            }
            // verify parse failures
            for eapi in eapi::EAPIS.difference(&passing_eapis) {
                let result = dep(&s, eapi);
                assert!(result.is_err(), "{s:?} didn't fail for EAPI={eapi}");
            }
        }
    }

    #[test]
    fn test_parse_slots() {
        // good deps
        for slot in ["0", "a", "_", "_a", "99", "aBc", "a+b_c.d-e"] {
            for eapi in eapi::EAPIS.iter() {
                let s = format!("cat/pkg:{slot}");
                let result = dep(&s, eapi);
                match eapi.has(Feature::SlotDeps) {
                    false => assert!(result.is_err(), "{s:?} didn't fail"),
                    true => {
                        assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
                        let atom = result.unwrap();
                        assert_eq!(atom.slot, Some(slot.into()));
                        assert_eq!(format!("{atom}"), s);
                    }
                };
            }
        }
    }

    #[test]
    fn test_parse_blockers() {
        // non-blocker
        let atom = dep("cat/pkg", &eapi::EAPI2).unwrap();
        assert!(atom.blocker.is_none());

        // good deps
        for (s, blocker) in [
            ("!cat/pkg", Some(Blocker::Weak)),
            ("!cat/pkg:0", Some(Blocker::Weak)),
            ("!!cat/pkg", Some(Blocker::Strong)),
            ("!!<cat/pkg-1", Some(Blocker::Strong)),
        ] {
            for eapi in eapi::EAPIS.iter() {
                let result = dep(s, eapi);
                match eapi.has(Feature::Blockers) {
                    false => assert!(result.is_err(), "{s:?} didn't fail"),
                    true => {
                        assert!(
                            result.is_ok(),
                            "{s:?} failed for EAPI {eapi}: {}",
                            result.err().unwrap()
                        );
                        let atom = result.unwrap();
                        assert_eq!(atom.blocker, blocker);
                        assert_eq!(format!("{atom}"), s);
                    }
                };
            }
        }
    }

    #[test]
    fn test_parse_use_deps() {
        // good deps
        for use_deps in ["a", "!a?", "a,b", "-a,-b", "a?,b?", "a,b=,!c=,d?,!e?,-f"] {
            for eapi in eapi::EAPIS.iter() {
                let s = format!("cat/pkg[{use_deps}]");
                let result = dep(&s, eapi);
                match eapi.has(Feature::UseDeps) {
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
        // good deps
        for use_deps in ["a(+)", "-a(-)", "a(+)?,!b(-)?", "a(-)=,!b(+)="] {
            for eapi in eapi::EAPIS.iter() {
                let s = format!("cat/pkg[{use_deps}]");
                let result = dep(&s, eapi);
                match eapi.has(Feature::UseDepDefaults) {
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
        // good deps
        for (slot_str, slot, subslot, slot_op) in [
            ("0/1", opt_str!("0"), opt_str!("1"), None),
            ("a/b", opt_str!("a"), opt_str!("b"), None),
            ("A/B", opt_str!("A"), opt_str!("B"), None),
            ("_/_", opt_str!("_"), opt_str!("_"), None),
            ("0/a.b+c-d_e", opt_str!("0"), opt_str!("a.b+c-d_e"), None),
        ] {
            for eapi in eapi::EAPIS.iter() {
                let s = format!("cat/pkg:{slot_str}");
                let result = dep(&s, eapi);
                match eapi.has(Feature::SlotOps) {
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
        // good deps
        for (slot_str, slot, subslot, slot_op) in [
            ("*", None, None, Some(SlotOperator::Star)),
            ("=", None, None, Some(SlotOperator::Equal)),
            ("0=", opt_str!("0"), None, Some(SlotOperator::Equal)),
            ("a=", opt_str!("a"), None, Some(SlotOperator::Equal)),
            ("0/1=", opt_str!("0"), opt_str!("1"), Some(SlotOperator::Equal)),
            ("a/b=", opt_str!("a"), opt_str!("b"), Some(SlotOperator::Equal)),
        ] {
            for eapi in eapi::EAPIS.iter() {
                let s = format!("cat/pkg:{slot_str}");
                let result = dep(&s, eapi);
                match eapi.has(Feature::SlotOps) {
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
        // repo deps
        for repo in ["_", "a", "repo", "repo_a", "repo-a"] {
            let s = format!("cat/pkg::{repo}");

            // repo ids aren't supported in official EAPIs
            for eapi in eapi::EAPIS_OFFICIAL.iter() {
                assert!(dep(&s, eapi).is_err(), "{s:?} didn't fail");
            }

            let result = dep(&s, &eapi::EAPI_PKGCRAFT);
            assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
            let atom = result.unwrap();
            assert_eq!(atom.repo, opt_str!(repo));
            assert_eq!(format!("{atom}"), s);
        }
    }
}
