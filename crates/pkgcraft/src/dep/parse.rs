use cached::{proc_macro::cached, SizedCache};

use crate::dep::cpv::ParsedCpv;
use crate::dep::pkg::ParsedDep;
use crate::dep::version::{ParsedVersion, Suffix};
use crate::dep::{Blocker, Cpv, Dep, DepSet, DepSpec, SlotOperator, Uri, Version};
use crate::eapi::{Eapi, Feature};
use crate::error::peg_error;
use crate::types::Ordered;
use crate::Error;

peg::parser!(grammar depspec() for str {
    // Categories must not begin with a hyphen, dot, or plus sign.
    pub(super) rule category() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '.' | '-']*
        } / expected!("category name"))
        { s }

    // Packages must not begin with a hyphen or plus sign and must not end in a
    // hyphen followed by anything matching a version.
    pub(super) rule package() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
            (['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_'] /
                ("-" !(version() ("-" version())? ![_])))*
        } / expected!("package name"))
        { s }

    rule version_suffix() -> Suffix
        = "_" suffix:$("alpha" / "beta" / "pre" / "rc" / "p") ver:$(['0'..='9']+)? {?
            let num = ver.map(|s| s.parse().map_err(|_| "version suffix integer overflow"));
            let suffix = match suffix {
                "alpha" => Suffix::Alpha,
                "beta" => Suffix::Beta,
                "pre" => Suffix::Pre,
                "rc" => Suffix::Rc,
                "p" => Suffix::P,
                _ => panic!("invalid suffix"),
            };
            Ok(suffix(num.transpose()?))
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
        = op:$(("<" "="?) / "=" / "~" / (">" "="?)) v:version() glob:$("*")? {?
            v.with_op(op, glob)
        }

    rule revision() -> &'input str
        = "-r" s:$(quiet!{['0'..='9']+} / expected!("revision"))
        { s }

    // Slot names must not begin with a hyphen, dot, or plus sign.
    pub(super) rule slot_name() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '.' | '-']*
        } / expected!("slot name")
        ) { s }

    rule slot_dep() -> (Option<&'input str>, Option<&'input str>, Option<SlotOperator>)
        = ":" slot_parts:slot_str() { slot_parts }

    rule slot_str() -> (Option<&'input str>, Option<&'input str>, Option<SlotOperator>)
        = s:$("*" / "=") {?
            let op = s.parse().map_err(|_| "invalid slot operator")?;
            Ok((None, None, Some(op)))
        } / slot:slot() op:$("=")? {?
            Ok((Some(slot.0), slot.1, op.map(|_| SlotOperator::Equal)))
        }

    rule slot() -> (&'input str, Option<&'input str>)
        = slot:slot_name() subslot:subslot()? {
            (slot, subslot)
        }

    rule subslot() -> &'input str
        = "/" s:slot_name() { s }

    rule blocker() -> Blocker
        = s:$("!" "!"?) {?
            s.parse().map_err(|_| "invalid blocker")
        }

    pub(super) rule use_flag() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9']
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '@' | '-']*
        } / expected!("USE flag name")
        ) { s }

    pub(super) rule iuse() -> (Option<char>, &'input str)
        = default:(['+' | '-'])? flag:use_flag() { (default, flag) }

    rule use_dep() -> &'input str
        = s:$(quiet!{
            (use_flag() use_dep_default()? ['=' | '?']?) /
            ("-" use_flag() use_dep_default()?) /
            ("!" use_flag() use_dep_default()? ['=' | '?'])
        } / expected!("use dep")
        ) { s }

    rule use_deps() -> Vec<&'input str>
        = "[" use_deps:use_dep() ++ "," "]" {
            use_deps
        }

    rule use_dep_default() -> &'input str
        = s:$("(+)" / "(-)") {
            s
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
            if eapi.has(Feature::RepoIds) {
                Ok(repo)
            } else {
                Err("repo deps aren't supported in EAPIs")
            }
        }

    pub(super) rule cpv() -> ParsedCpv<'input>
        = category:category() "/" package:package() "-" version:version() {
            ParsedCpv {
                category,
                package,
                version,
                version_str: "",
            }
        }

    pub(super) rule cpv_with_op() -> (&'input str, &'input str, Option<&'input str>)
        = op:$(("<" "="?) / "=" / "~" / (">" "="?)) cpv:$([^'*']+) glob:$("*")?
        { (op, cpv, glob) }

    pub(super) rule cpn() -> ParsedDep<'input>
        = category:category() "/" package:package() {
            ParsedDep { category, package, ..Default::default() }
        }

    pub(super) rule dep(eapi: &'static Eapi) -> (&'input str, ParsedDep<'input>)
        = blocker:blocker()? dep:$([^':' | '[']+) slot_dep:slot_dep()?
                use_deps:use_deps()? repo:repo_dep(eapi)? {
            let (slot, subslot, slot_op) = slot_dep.unwrap_or_default();
            (dep, ParsedDep {
                blocker,
                slot,
                subslot,
                slot_op,
                use_deps,
                repo,
                ..Default::default()
            })
        }

    rule _ = [' ']

    // Technically PROPERTIES and RESTRICT tokens have no restrictions, but use license
    // restrictions in order to properly parse use restrictions.
    rule properties_restrict_val() -> DepSpec<String, String>
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '.' | '-']*
        } / expected!("string value")
        ) { DepSpec::Enabled(s.to_string()) }

    // licenses must not begin with a hyphen, dot, or plus sign.
    rule license_val() -> DepSpec<String, String>
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '.' | '-']*
        } / expected!("license name")
        ) { DepSpec::Enabled(s.to_string()) }

    rule use_flag_val() -> DepSpec<String, String>
        = disabled:"!"? s:use_flag() {
            let val = s.to_string();
            if disabled.is_none() {
                DepSpec::Enabled(val)
            } else {
                DepSpec::Disabled(val)
            }
        }

    rule dependencies_val(eapi: &'static Eapi) -> DepSpec<String, Dep>
        = s:$(quiet!{!")" [^' ']+}) {?
            let dep = match Dep::new(s, eapi) {
                Ok(x) => x,
                Err(e) => return Err("failed parsing dep"),
            };
            Ok(DepSpec::Enabled(dep))
        }

    rule uri_val() -> DepSpec<String, Uri>
        = s:$(quiet!{!")" [^' ']+}) rename:(_ "->" _ s:$([^' ']+) {s})? {?
            let uri = Uri::new(s, rename).map_err(|_| "invalid URI")?;
            Ok(DepSpec::Enabled(uri))
        }

    rule parens<T: Ordered>(expr: rule<T>) -> Vec<T>
        = "(" _ v:expr() ++ " " _ ")" { v }

    rule all_of<T: Ordered>(expr: rule<DepSpec<String, T>>) -> DepSpec<String, T>
        = vals:parens(<expr()>)
        { DepSpec::AllOf(vals.into_iter().map(Box::new).collect()) }

    rule any_of<T: Ordered>(expr: rule<DepSpec<String, T>>) -> DepSpec<String, T>
        = "||" _ vals:parens(<expr()>)
        { DepSpec::AnyOf(vals.into_iter().map(Box::new).collect()) }

    rule use_cond<T: Ordered>(expr: rule<DepSpec<String, T>>) -> DepSpec<String, T>
        = negate:"!"? u:use_flag() "?" _ vals:parens(<expr()>) {
            let f = match negate {
                None => DepSpec::UseEnabled,
                Some(_) => DepSpec::UseDisabled,
            };
            f(u.to_string(), vals.into_iter().map(Box::new).collect())
        }

    rule exactly_one_of<T: Ordered>(expr: rule<DepSpec<String, T>>) -> DepSpec<String, T>
        = "^^" _ vals:parens(<expr()>)
        { DepSpec::ExactlyOneOf(vals.into_iter().map(Box::new).collect()) }

    rule at_most_one_of<T: Ordered>(eapi: &'static Eapi, expr: rule<DepSpec<String, T>>) -> DepSpec<String, T>
        = "??" _ vals:parens(<expr()>) {
            DepSpec::AtMostOneOf(vals.into_iter().map(Box::new).collect())
        }

    pub(super) rule license_dep_spec() -> DepSpec<String, String>
        = use_cond(<license_dep_spec()>)
            / any_of(<license_dep_spec()>)
            / all_of(<license_dep_spec()>)
            / license_val()

    pub(super) rule src_uri_dep_spec(eapi: &'static Eapi) -> DepSpec<String, Uri>
        = use_cond(<src_uri_dep_spec(eapi)>)
            / all_of(<src_uri_dep_spec(eapi)>)
            / uri_val()

    pub(super) rule properties_dep_spec() -> DepSpec<String, String>
        = use_cond(<properties_dep_spec()>)
            / all_of(<properties_dep_spec()>)
            / properties_restrict_val()

    pub(super) rule required_use_dep_spec(eapi: &'static Eapi) -> DepSpec<String, String>
        = use_cond(<required_use_dep_spec(eapi)>)
            / any_of(<required_use_dep_spec(eapi)>)
            / all_of(<required_use_dep_spec(eapi)>)
            / exactly_one_of(<required_use_dep_spec(eapi)>)
            / at_most_one_of(eapi, <required_use_dep_spec(eapi)>)
            / use_flag_val()

    pub(super) rule restrict_dep_spec() -> DepSpec<String, String>
        = use_cond(<restrict_dep_spec()>)
            / all_of(<restrict_dep_spec()>)
            / properties_restrict_val()

    pub(super) rule dependencies_dep_spec(eapi: &'static Eapi) -> DepSpec<String, Dep>
        = use_cond(<dependencies_dep_spec(eapi)>)
            / any_of(<dependencies_dep_spec(eapi)>)
            / all_of(<dependencies_dep_spec(eapi)>)
            / dependencies_val(eapi)

    pub(super) rule license_dep_set() -> DepSet<String, String>
        = v:license_dep_spec() ++ " " { DepSet::from_iter(v) }

    pub(super) rule src_uri_dep_set(eapi: &'static Eapi) -> DepSet<String, Uri>
        = v:src_uri_dep_spec(eapi) ++ " " { DepSet::from_iter(v) }

    pub(super) rule properties_dep_set() -> DepSet<String, String>
        = v:properties_dep_spec() ++ " " { DepSet::from_iter(v) }

    pub(super) rule required_use_dep_set(eapi: &'static Eapi) -> DepSet<String, String>
        = v:required_use_dep_spec(eapi) ++ " " { DepSet::from_iter(v) }

    pub(super) rule restrict_dep_set() -> DepSet<String, String>
        = v:restrict_dep_spec() ++ " " { DepSet::from_iter(v) }

    pub(super) rule dependencies_dep_set(eapi: &'static Eapi) -> DepSet<String, Dep>
        = v:dependencies_dep_spec(eapi) ++ " " { DepSet::from_iter(v) }
});

pub fn category(s: &str) -> crate::Result<&str> {
    depspec::category(s).map_err(|e| peg_error(format!("invalid category name: {s}"), s, e))?;
    Ok(s)
}

pub fn package(s: &str) -> crate::Result<&str> {
    depspec::package(s).map_err(|e| peg_error(format!("invalid package name: {s}"), s, e))?;
    Ok(s)
}

pub(super) fn version_str(s: &str) -> crate::Result<ParsedVersion> {
    depspec::version(s).map_err(|e| peg_error(format!("invalid version: {s}"), s, e))
}

#[cached(
    type = "SizedCache<String, crate::Result<Version>>",
    create = "{ SizedCache::with_size(1000) }",
    convert = r#"{ s.to_string() }"#
)]
pub fn version(s: &str) -> crate::Result<Version> {
    version_str(s)?.into_owned(s)
}

pub fn version_with_op(s: &str) -> crate::Result<Version> {
    let ver = depspec::version_with_op(s)
        .map_err(|e| peg_error(format!("invalid version: {s}"), s, e))?;
    ver.into_owned(s)
}

pub fn slot(s: &str) -> crate::Result<&str> {
    depspec::slot_name(s).map_err(|e| peg_error(format!("invalid slot: {s}"), s, e))?;
    Ok(s)
}

pub fn use_flag(s: &str) -> crate::Result<&str> {
    depspec::use_flag(s).map_err(|e| peg_error(format!("invalid USE flag: {s}"), s, e))?;
    Ok(s)
}

pub(crate) fn iuse(s: &str) -> crate::Result<(Option<char>, &str)> {
    depspec::iuse(s).map_err(|e| peg_error(format!("invalid IUSE: {s}"), s, e))
}

pub fn repo(s: &str) -> crate::Result<&str> {
    depspec::repo(s).map_err(|e| peg_error(format!("invalid repo name: {s}"), s, e))?;
    Ok(s)
}

pub(super) fn cpv_str(s: &str) -> crate::Result<ParsedCpv> {
    depspec::cpv(s).map_err(|e| peg_error(format!("invalid cpv: {s}"), s, e))
}

pub(super) fn cpv(s: &str) -> crate::Result<Cpv> {
    let mut cpv = cpv_str(s)?;
    cpv.version_str = s;
    cpv.into_owned()
}

pub(super) fn dep_str<'a>(s: &'a str, eapi: &'static Eapi) -> crate::Result<ParsedDep<'a>> {
    let (dep_s, mut dep) =
        depspec::dep(s, eapi).map_err(|e| peg_error(format!("invalid dep: {s}"), s, e))?;
    match depspec::cpv_with_op(dep_s) {
        Ok((op, cpv_s, glob)) => {
            let cpv = depspec::cpv(cpv_s)
                .map_err(|e| peg_error(format!("invalid dep: {s}"), cpv_s, e))?;
            dep.category = cpv.category;
            dep.package = cpv.package;
            dep.version = Some(
                cpv.version
                    .with_op(op, glob)
                    .map_err(|e| Error::InvalidValue(format!("invalid dep: {s}: {e}")))?,
            );
            dep.version_str = Some(cpv_s);
        }
        _ => {
            let d = depspec::cpn(dep_s)
                .map_err(|e| peg_error(format!("invalid dep: {s}"), dep_s, e))?;
            dep.category = d.category;
            dep.package = d.package;
        }
    }

    Ok(dep)
}

#[cached(
    type = "SizedCache<(String, &Eapi), crate::Result<Dep>>",
    create = "{ SizedCache::with_size(1000) }",
    convert = r#"{ (s.to_string(), eapi) }"#
)]
pub(super) fn dep(s: &str, eapi: &'static Eapi) -> crate::Result<Dep> {
    let dep = dep_str(s, eapi)?;
    dep.into_owned()
}

pub(super) fn cpn(s: &str) -> crate::Result<Dep> {
    let dep =
        depspec::cpn(s).map_err(|e| peg_error(format!("invalid unversioned dep: {s}"), s, e))?;
    dep.into_owned()
}

pub fn license_dep_set(s: &str) -> crate::Result<Option<DepSet<String, String>>> {
    if s.is_empty() {
        Ok(None)
    } else {
        depspec::license_dep_set(s)
            .map(Some)
            .map_err(|e| peg_error(format!("invalid LICENSE: {s:?}"), s, e))
    }
}

pub fn license_dep_spec(s: &str) -> crate::Result<DepSpec<String, String>> {
    depspec::license_dep_spec(s)
        .map_err(|e| peg_error(format!("invalid LICENSE DepSpec: {s:?}"), s, e))
}

pub fn src_uri_dep_set(s: &str, eapi: &'static Eapi) -> crate::Result<Option<DepSet<String, Uri>>> {
    if s.is_empty() {
        Ok(None)
    } else {
        depspec::src_uri_dep_set(s, eapi)
            .map(Some)
            .map_err(|e| peg_error(format!("invalid SRC_URI: {s:?}"), s, e))
    }
}

pub fn src_uri_dep_spec(s: &str, eapi: &'static Eapi) -> crate::Result<DepSpec<String, Uri>> {
    depspec::src_uri_dep_spec(s, eapi)
        .map_err(|e| peg_error(format!("invalid SRC_URI DepSpec: {s:?}"), s, e))
}

pub fn properties_dep_set(s: &str) -> crate::Result<Option<DepSet<String, String>>> {
    if s.is_empty() {
        Ok(None)
    } else {
        depspec::properties_dep_set(s)
            .map(Some)
            .map_err(|e| peg_error(format!("invalid PROPERTIES: {s:?}"), s, e))
    }
}

pub fn properties_dep_spec(s: &str) -> crate::Result<DepSpec<String, String>> {
    depspec::properties_dep_spec(s)
        .map_err(|e| peg_error(format!("invalid PROPERTIES DepSpec: {s:?}"), s, e))
}

pub fn required_use_dep_set(
    s: &str,
    eapi: &'static Eapi,
) -> crate::Result<Option<DepSet<String, String>>> {
    if s.is_empty() {
        Ok(None)
    } else {
        depspec::required_use_dep_set(s, eapi)
            .map(Some)
            .map_err(|e| peg_error(format!("invalid REQUIRED_USE: {s:?}"), s, e))
    }
}

pub fn required_use_dep_spec(
    s: &str,
    eapi: &'static Eapi,
) -> crate::Result<DepSpec<String, String>> {
    depspec::required_use_dep_spec(s, eapi)
        .map_err(|e| peg_error(format!("invalid REQUIRED_USE DepSpec: {s:?}"), s, e))
}

pub fn restrict_dep_set(s: &str) -> crate::Result<Option<DepSet<String, String>>> {
    if s.is_empty() {
        Ok(None)
    } else {
        depspec::restrict_dep_set(s)
            .map(Some)
            .map_err(|e| peg_error(format!("invalid RESTRICT: {s:?}"), s, e))
    }
}

pub fn restrict_dep_spec(s: &str) -> crate::Result<DepSpec<String, String>> {
    depspec::restrict_dep_spec(s)
        .map_err(|e| peg_error(format!("invalid RESTRICT DepSpec: {s:?}"), s, e))
}

pub fn dependencies_dep_set(
    s: &str,
    eapi: &'static Eapi,
) -> crate::Result<Option<DepSet<String, Dep>>> {
    if s.is_empty() {
        Ok(None)
    } else {
        depspec::dependencies_dep_set(s, eapi)
            .map(Some)
            .map_err(|e| peg_error(format!("invalid dependency: {s:?}"), s, e))
    }
}

pub fn dependencies_dep_spec(s: &str, eapi: &'static Eapi) -> crate::Result<DepSpec<String, Dep>> {
    depspec::dependencies_dep_spec(s, eapi)
        .map_err(|e| peg_error(format!("invalid dependency DepSpec: {s:?}"), s, e))
}

#[cfg(test)]
mod tests {
    use crate::eapi::{self, EAPIS, EAPIS_OFFICIAL, EAPI_LATEST_OFFICIAL};

    use super::*;

    #[test]
    fn test_parse_slots() {
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
    fn test_parse_blockers() {
        let d = dep("cat/pkg", &eapi::EAPI_LATEST_OFFICIAL).unwrap();
        assert!(d.blocker().is_none());

        for (s, blocker) in [
            ("!cat/pkg", Some(Blocker::Weak)),
            ("!cat/pkg:0", Some(Blocker::Weak)),
            ("!!cat/pkg", Some(Blocker::Strong)),
            ("!!<cat/pkg-1", Some(Blocker::Strong)),
        ] {
            for eapi in &*EAPIS {
                let result = dep(s, eapi);
                assert!(result.is_ok(), "{s:?} failed for EAPI {eapi}: {}", result.err().unwrap());
                let d = result.unwrap();
                assert_eq!(d.blocker(), blocker);
                assert_eq!(d.to_string(), s);
            }
        }
    }

    #[test]
    fn test_parse_use_deps() {
        for use_deps in ["a", "!a?", "a,b", "-a,-b", "a?,b?", "a,b=,!c=,d?,!e?,-f"] {
            for eapi in &*EAPIS {
                let s = format!("cat/pkg[{use_deps}]");
                let result = dep(&s, eapi);
                assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
                let d = result.unwrap();
                let expected = use_deps.split(',').map(|s| s.to_string()).collect();
                assert_eq!(d.use_deps(), Some(&expected));
                assert_eq!(d.to_string(), s);
            }
        }
    }

    #[test]
    fn test_parse_use_dep_defaults() {
        for use_deps in ["a(+)", "-a(-)", "a(+)?,!b(-)?", "a(-)=,!b(+)="] {
            for eapi in &*EAPIS {
                let s = format!("cat/pkg[{use_deps}]");
                let result = dep(&s, eapi);
                assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
                let d = result.unwrap();
                let expected = use_deps.split(',').map(|s| s.to_string()).collect();
                assert_eq!(d.use_deps(), Some(&expected));
                assert_eq!(d.to_string(), s);
            }
        }
    }

    #[test]
    fn test_parse_subslots() {
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
    fn test_parse_slot_ops() {
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
    fn test_parse_repos() {
        for repo in ["_", "a", "repo", "repo_a", "repo-a"] {
            let s = format!("cat/pkg::{repo}");

            // repo ids aren't supported in official EAPIs
            for eapi in &*EAPIS_OFFICIAL {
                assert!(dep(&s, eapi).is_err(), "{s:?} didn't fail");
            }

            let result = dep(&s, &eapi::EAPI_PKGCRAFT);
            assert!(result.is_ok(), "{s:?} failed: {}", result.err().unwrap());
            let d = result.unwrap();
            assert_eq!(d.repo(), Some(repo));
            assert_eq!(d.to_string(), s);
        }
    }

    #[test]
    fn test_license() {
        // invalid
        for s in ["(", ")", "( )", "( l1)", "| ( l1 )", "!use ( l1 )"] {
            assert!(license_dep_set(s).is_err(), "{s:?} didn't fail");
            assert!(license_dep_spec(s).is_err(), "{s:?} didn't fail");
        }

        // empty string
        assert!(license_dep_set("").unwrap().is_none());

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
            let depset = license_dep_set(s).unwrap().unwrap();
            assert_eq!(depset.to_string(), s);
            let flatten: Vec<_> = depset.iter_flatten().collect();
            assert_eq!(flatten, expected_flatten);
        }
    }

    #[test]
    fn test_src_uri() {
        // empty string
        assert!(src_uri_dep_set("", &EAPI_LATEST_OFFICIAL)
            .unwrap()
            .is_none());

        // valid
        for (s, expected_flatten) in [
            ("uri", vec!["uri"]),
            ("http://uri", vec!["http://uri"]),
            ("uri1 uri2", vec!["uri1", "uri2"]),
            ("( http://uri1 http://uri2 )", vec!["http://uri1", "http://uri2"]),
            ("u1? ( http://uri1 !u2? ( http://uri2 ) )", vec!["http://uri1", "http://uri2"]),
        ] {
            for eapi in &*EAPIS {
                let depset = src_uri_dep_set(s, eapi).unwrap().unwrap();
                assert_eq!(depset.to_string(), s);
                let flatten: Vec<_> = depset.iter_flatten().map(|x| x.to_string()).collect();
                assert_eq!(flatten, expected_flatten);
            }
        }

        // SRC_URI renames
        for (s, expected_flatten) in [
            ("http://uri -> file", vec!["http://uri -> file"]),
            ("u? ( http://uri -> file )", vec!["http://uri -> file"]),
        ] {
            for eapi in &*EAPIS {
                let depset = src_uri_dep_set(s, eapi).unwrap().unwrap();
                assert_eq!(depset.to_string(), s);
                let flatten: Vec<_> = depset.iter_flatten().map(|x| x.to_string()).collect();
                assert_eq!(flatten, expected_flatten);
            }
        }

        for s in ["https://", "https://web/site/root.com/"] {
            let r = src_uri_dep_set(s, &EAPI_LATEST_OFFICIAL);
            assert!(r.is_err(), "{s:?} didn't fail");
        }
    }

    #[test]
    fn test_required_use() {
        // invalid
        for s in ["(", ")", "( )", "( u)", "| ( u )", "|| ( )", "^^ ( )", "?? ( )"] {
            assert!(required_use_dep_set(s, &EAPI_LATEST_OFFICIAL).is_err(), "{s:?} didn't fail");
            assert!(required_use_dep_spec(s, &EAPI_LATEST_OFFICIAL).is_err(), "{s:?} didn't fail");
        }

        // empty string
        assert!(required_use_dep_set("", &EAPI_LATEST_OFFICIAL)
            .unwrap()
            .is_none());

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
            let depset = required_use_dep_set(s, &EAPI_LATEST_OFFICIAL)
                .unwrap()
                .unwrap();
            assert_eq!(depset.to_string(), s);
            let flatten: Vec<_> = depset.iter_flatten().collect();
            assert_eq!(flatten, expected_flatten);
        }

        // ?? operator
        for (s, expected_flatten) in [("?? ( u1 u2 )", vec!["u1", "u2"])] {
            for eapi in &*EAPIS {
                let depset = required_use_dep_set(s, eapi).unwrap().unwrap();
                assert_eq!(depset.to_string(), s);
                let flatten: Vec<_> = depset.iter_flatten().collect();
                assert_eq!(flatten, expected_flatten);
            }
        }
    }

    #[test]
    fn test_dependencies() {
        // invalid
        for s in ["(", ")", "( )", "|| ( )", "( a/b)", "| ( a/b )", "use ( a/b )", "!use ( a/b )"] {
            assert!(dependencies_dep_set(s, &EAPI_LATEST_OFFICIAL).is_err(), "{s:?} didn't fail");
            assert!(dependencies_dep_spec(s, &EAPI_LATEST_OFFICIAL).is_err(), "{s:?} didn't fail");
        }

        // empty string
        assert!(dependencies_dep_set("", &EAPI_LATEST_OFFICIAL)
            .unwrap()
            .is_none());

        // valid
        for (s, expected_flatten) in [
            ("a/b", vec!["a/b"]),
            ("a/b c/d", vec!["a/b", "c/d"]),
            ("( a/b c/d )", vec!["a/b", "c/d"]),
            ("u? ( a/b c/d )", vec!["a/b", "c/d"]),
            ("!u? ( a/b c/d )", vec!["a/b", "c/d"]),
            ("u1? ( a/b !u2? ( c/d ) )", vec!["a/b", "c/d"]),
        ] {
            let depset = dependencies_dep_set(s, &EAPI_LATEST_OFFICIAL)
                .unwrap()
                .unwrap();
            assert_eq!(depset.to_string(), s);
            let flatten: Vec<_> = depset.iter_flatten().map(|x| x.to_string()).collect();
            assert_eq!(flatten, expected_flatten);
        }
    }

    #[test]
    fn test_properties() {
        // invalid
        for s in ["(", ")", "( )", "( v)", "| ( v )", "!use ( v )", "|| ( v )", "|| ( v1 v2 )"] {
            assert!(properties_dep_set(s).is_err(), "{s:?} didn't fail");
            assert!(properties_dep_spec(s).is_err(), "{s:?} didn't fail");
        }

        // empty string
        assert!(properties_dep_set("").unwrap().is_none());

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
            let depset = properties_dep_set(s).unwrap().unwrap();
            assert_eq!(depset.to_string(), s);
            let flatten: Vec<_> = depset.iter_flatten().collect();
            assert_eq!(flatten, expected_flatten);
        }
    }

    #[test]
    fn test_restrict() {
        // invalid
        for s in ["(", ")", "( )", "( v)", "| ( v )", "!use ( v )", "|| ( v )", "|| ( v1 v2 )"] {
            assert!(restrict_dep_set(s).is_err(), "{s:?} didn't fail");
            assert!(restrict_dep_spec(s).is_err(), "{s:?} didn't fail");
        }

        // empty string
        assert!(restrict_dep_set("").unwrap().is_none());

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
            let depset = restrict_dep_set(s).unwrap().unwrap();
            assert_eq!(depset.to_string(), s);
            let flatten: Vec<_> = depset.iter_flatten().collect();
            assert_eq!(flatten, expected_flatten);
        }
    }
}
