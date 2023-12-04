use cached::{proc_macro::cached, SizedCache};

use crate::dep::cpv::ParsedCpv;
use crate::dep::pkg::ParsedDep;
use crate::dep::version::{ParsedNumber, ParsedSuffix, ParsedVersion, SuffixKind};
use crate::dep::{
    Blocker, Cpv, Dep, DepSet, DepSpec, Operator, Slot, SlotDep, SlotOperator, Uri, UseDep,
    UseDepDefault, UseDepKind, Version,
};
use crate::eapi::{Eapi, Feature};
use crate::error::peg_error;
use crate::shell::metadata::{Iuse, Keyword, KeywordStatus};
use crate::traits::IntoOwned;
use crate::types::Ordered;

peg::parser!(grammar depspec() for str {
    // Keywords must not begin with a hyphen.
    rule keyword_name() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-']*
        } / expected!("keyword name"))
        { s }

    // The "-*" keyword is allowed in KEYWORDS for package metadata.
    pub(super) rule keyword() -> Keyword<&'input str>
        = arch:keyword_name() { Keyword { status: KeywordStatus::Stable, arch } }
        / "~" arch:keyword_name() { Keyword { status: KeywordStatus::Unstable, arch } }
        / "-" arch:keyword_name() { Keyword { status: KeywordStatus::Disabled, arch } }
        / "-*" { Keyword { status: KeywordStatus::Disabled, arch: "*" } }

    // License names must not begin with a hyphen, dot, or plus sign.
    pub(super) rule license_name(err: &'static str) -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '.' | '-']*
        } / expected!(err)) { s }

    // Eclass names must not begin with a hyphen or dot and cannot be named "default".
    pub(super) rule eclass_name() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '.' | '-']*
        } / expected!("eclass name")) {?
            if s == "default" {
                Err("eclass cannot be named: default")
            } else {
                Ok(s)
            }
        }

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
                ("-" !(version() ("-" version())? (__ / "*" / ":" / "[" / ![_]))))*
        } / expected!("package name"))
        { s }

    rule number() -> ParsedNumber<'input>
        = s:$(['0'..='9']+) {?
            let value = s.parse().map_err(|_| "integer overflow")?;
            Ok(ParsedNumber { raw: s, value })
        }

    rule suffix() -> SuffixKind
        = "alpha" { SuffixKind::Alpha }
        / "beta" { SuffixKind::Beta }
        / "pre" { SuffixKind::Pre }
        / "rc" { SuffixKind::Rc }
        / "p" { SuffixKind::P }

    rule version_suffix() -> ParsedSuffix<'input>
        = "_" kind:suffix() version:number()? { ParsedSuffix { kind, version } }

    // TODO: figure out how to return string slice instead of positions
    // Related issue: https://github.com/kevinmehall/rust-peg/issues/283
    pub(super) rule version() -> ParsedVersion<'input>
        = numbers:number() ++ "." letter:['a'..='z']?
                suffixes:version_suffix()* revision:revision()? {
            ParsedVersion {
                op: None,
                numbers,
                letter,
                suffixes,
                revision,
            }
        }

    pub(super) rule version_with_op() -> ParsedVersion<'input>
        = "<=" v:version() { v.with_op(Operator::LessOrEqual) }
        / "<" v:version() { v.with_op(Operator::Less) }
        / ">=" v:version() { v.with_op(Operator::GreaterOrEqual) }
        / ">" v:version() { v.with_op(Operator::Greater) }
        / "=" v:version() glob:$("*")? {
            if glob.is_none() {
                v.with_op(Operator::Equal)
            } else {
                v.with_op(Operator::EqualGlob)
            }
        } / "~" v:version() {?
            if v.revision.is_some() {
                Err("~ version operator can't be used with a revision")
            } else {
                Ok(v.with_op(Operator::Approximate))
            }
        }

    rule revision() -> ParsedNumber<'input>
        = "-r" rev:number() { rev }
        / expected!("revision")

    // Slot names must not begin with a hyphen, dot, or plus sign.
    pub(super) rule slot_name() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '.' | '-']*
        } / expected!("slot name")
        ) { s }

    pub(super) rule slot() -> Slot<&'input str>
        = name:$(slot_name() ("/" slot_name())?)
        { Slot { name } }

    pub(super) rule slot_dep() -> SlotDep<&'input str>
        = "=" { SlotDep { slot: None, op: Some(SlotOperator::Equal) } }
        / "*" { SlotDep { slot: None, op: Some(SlotOperator::Star) } }
        / slot:slot() op:$("=")? {
            let op = op.map(|_| SlotOperator::Equal);
            SlotDep { slot: Some(slot), op }
        }

    rule slot_dep_str() -> SlotDep<&'input str>
        = ":" slot_dep:slot_dep() { slot_dep }

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

    pub(super) rule iuse() -> Iuse<&'input str>
        = flag:use_flag() { Iuse { flag, default: None } }
        / "+" flag:use_flag() { Iuse { flag, default: Some(true) } }
        / "-" flag:use_flag() { Iuse { flag, default: Some(false) } }

    rule use_dep_default() -> UseDepDefault
        = "(+)" { UseDepDefault::Enabled }
        / "(-)" { UseDepDefault::Disabled }

    pub(super) rule use_dep() -> UseDep<&'input str>
        = flag:use_flag() default:use_dep_default()? kind:$(['=' | '?'])? {
            let kind = match kind {
                Some("=") => UseDepKind::Equal,
                Some("?") => UseDepKind::EnabledConditional,
                None => UseDepKind::Enabled,
                _ => panic!("invalid use dep kind"),
            };
            UseDep { kind, flag, default }
        } / "-" flag:use_flag() default:use_dep_default()? {
            UseDep { kind: UseDepKind::Disabled, flag, default }
        } / "!" flag:use_flag() default:use_dep_default()? kind:$(['=' | '?']) {
            let kind = match kind {
                "=" => UseDepKind::NotEqual,
                "?" => UseDepKind::DisabledConditional,
                _ => panic!("invalid use dep kind"),
            };
            UseDep { kind, flag, default }
        } / expected!("use dep")

    rule use_deps() -> Vec<UseDep<&'input str>>
        = "[" use_deps:use_dep() ++ "," "]" { use_deps }

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
                Err("repo deps aren't supported in official EAPIs")
            }
        }

    pub(super) rule cpv() -> ParsedCpv<'input>
        = category:category() "/" package:package() "-" version:version()
        { ParsedCpv { category, package, version } }

    rule dep_pkg() -> ParsedDep<'input>
        = dep:cpn() { dep }
        / "<=" cpv:cpv() { cpv.with_op(Operator::LessOrEqual) }
        / "<" cpv:cpv() { cpv.with_op(Operator::Less) }
        / ">=" cpv:cpv() { cpv.with_op(Operator::GreaterOrEqual) }
        / ">" cpv:cpv() { cpv.with_op(Operator::Greater) }
        / "=" cpv:cpv() glob:$("*")? {
            if glob.is_none() {
                cpv.with_op(Operator::Equal)
            } else {
                cpv.with_op(Operator::EqualGlob)
            }
        } / "~" cpv:cpv() {?
            if cpv.version.revision.is_some() {
                Err("~ operator can't be used with a revision")
            } else {
                Ok(cpv.with_op(Operator::Approximate))
            }
        }

    pub(super) rule cpn() -> ParsedDep<'input>
        = category:category() "/" package:package() {
            ParsedDep { category, package, ..Default::default() }
        }

    pub(super) rule dep(eapi: &'static Eapi) -> ParsedDep<'input>
        = blocker:blocker()? dep:dep_pkg() slot:slot_dep_str()?
                repo:repo_dep(eapi)? use_deps:use_deps()? {
            dep.with(blocker, slot, use_deps, repo)
        }

    rule _ = quiet!{[^ ' ' | '\n' | '\t']+}
    rule __ = quiet!{[' ' | '\n' | '\t']+}

    rule parens<T: Ordered>(expr: rule<T>) -> Vec<T>
        = "(" __ v:expr() ++ __ __ ")" { v }

    rule all_of<T: Ordered>(expr: rule<DepSpec<String, T>>) -> DepSpec<String, T>
        = vals:parens(<expr()>)
        { DepSpec::AllOf(vals.into_iter().map(Box::new).collect()) }

    rule any_of<T: Ordered>(expr: rule<DepSpec<String, T>>) -> DepSpec<String, T>
        = "||" __ vals:parens(<expr()>)
        { DepSpec::AnyOf(vals.into_iter().map(Box::new).collect()) }

    rule use_cond<T: Ordered>(expr: rule<DepSpec<String, T>>) -> DepSpec<String, T>
        = negate:"!"? u:use_flag() "?" __ vals:parens(<expr()>) {
            let f = match negate {
                None => DepSpec::UseEnabled,
                Some(_) => DepSpec::UseDisabled,
            };
            f(u.to_string(), vals.into_iter().map(Box::new).collect())
        }

    rule exactly_one_of<T: Ordered>(expr: rule<DepSpec<String, T>>) -> DepSpec<String, T>
        = "^^" __ vals:parens(<expr()>)
        { DepSpec::ExactlyOneOf(vals.into_iter().map(Box::new).collect()) }

    rule at_most_one_of<T: Ordered>(eapi: &'static Eapi, expr: rule<DepSpec<String, T>>) -> DepSpec<String, T>
        = "??" __ vals:parens(<expr()>) {
            DepSpec::AtMostOneOf(vals.into_iter().map(Box::new).collect())
        }

    pub(super) rule license_dep_spec() -> DepSpec<String, String>
        = use_cond(<license_dep_spec()>)
            / any_of(<license_dep_spec()>)
            / all_of(<license_dep_spec()>)
            / s:license_name("license name") { DepSpec::Enabled(s.to_string()) }

    pub(super) rule src_uri_dep_spec(eapi: &'static Eapi) -> DepSpec<String, Uri>
        = use_cond(<src_uri_dep_spec(eapi)>)
            / all_of(<src_uri_dep_spec(eapi)>)
            / s:$(quiet!{!")" _+}) rename:(__ "->" __ s:$(_+) {s})? {?
                let uri = Uri::new(s, rename).map_err(|_| "invalid URI")?;
                Ok(DepSpec::Enabled(uri))
            }

    // Technically RESTRICT tokens have no restrictions, but license
    // restrictions are currently used in order to properly parse use restrictions.
    pub(super) rule properties_dep_spec() -> DepSpec<String, String>
        = use_cond(<properties_dep_spec()>)
            / all_of(<properties_dep_spec()>)
            / s:license_name("properties name") { DepSpec::Enabled(s.to_string()) }

    pub(super) rule required_use_dep_spec(eapi: &'static Eapi) -> DepSpec<String, String>
        = use_cond(<required_use_dep_spec(eapi)>)
            / any_of(<required_use_dep_spec(eapi)>)
            / all_of(<required_use_dep_spec(eapi)>)
            / exactly_one_of(<required_use_dep_spec(eapi)>)
            / at_most_one_of(eapi, <required_use_dep_spec(eapi)>)
            / "!" s:use_flag() { DepSpec::Disabled(s.to_string()) }
            / s:use_flag() { DepSpec::Enabled(s.to_string()) }

    // Technically RESTRICT tokens have no restrictions, but license
    // restrictions are currently used in order to properly parse use restrictions.
    pub(super) rule restrict_dep_spec() -> DepSpec<String, String>
        = use_cond(<restrict_dep_spec()>)
            / all_of(<restrict_dep_spec()>)
            / s:license_name("restrict name") { DepSpec::Enabled(s.to_string()) }

    pub(super) rule dependencies_dep_spec(eapi: &'static Eapi) -> DepSpec<String, Dep>
        = use_cond(<dependencies_dep_spec(eapi)>)
            / any_of(<dependencies_dep_spec(eapi)>)
            / all_of(<dependencies_dep_spec(eapi)>)
            / dep:dep(eapi) { DepSpec::Enabled(dep.into_owned()) }

    pub(super) rule license_dep_set() -> DepSet<String, String>
        = v:license_dep_spec() ** __ { DepSet::from_iter(v) }

    pub(super) rule src_uri_dep_set(eapi: &'static Eapi) -> DepSet<String, Uri>
        = v:src_uri_dep_spec(eapi) ** __ { DepSet::from_iter(v) }

    pub(super) rule properties_dep_set() -> DepSet<String, String>
        = v:properties_dep_spec() ** __ { DepSet::from_iter(v) }

    pub(super) rule required_use_dep_set(eapi: &'static Eapi) -> DepSet<String, String>
        = v:required_use_dep_spec(eapi) ** __ { DepSet::from_iter(v) }

    pub(super) rule restrict_dep_set() -> DepSet<String, String>
        = v:restrict_dep_spec() ** __ { DepSet::from_iter(v) }

    pub(super) rule dependencies_dep_set(eapi: &'static Eapi) -> DepSet<String, Dep>
        = v:dependencies_dep_spec(eapi) ** __ { DepSet::from_iter(v) }
});

pub fn category(s: &str) -> crate::Result<&str> {
    depspec::category(s).map_err(|e| peg_error("invalid category name", s, e))
}

pub fn package(s: &str) -> crate::Result<&str> {
    depspec::package(s).map_err(|e| peg_error("invalid package name", s, e))
}

pub(super) fn version_str(s: &str) -> crate::Result<ParsedVersion> {
    depspec::version(s).map_err(|e| peg_error("invalid version", s, e))
}

#[cached(
    type = "SizedCache<String, crate::Result<Version>>",
    create = "{ SizedCache::with_size(1000) }",
    convert = r#"{ s.to_string() }"#
)]
pub fn version(s: &str) -> crate::Result<Version> {
    version_str(s).into_owned()
}

pub(super) fn version_with_op_str(s: &str) -> crate::Result<ParsedVersion> {
    depspec::version_with_op(s).map_err(|e| peg_error("invalid version", s, e))
}

pub fn version_with_op(s: &str) -> crate::Result<Version> {
    version_with_op_str(s).into_owned()
}

pub fn license_name(s: &str) -> crate::Result<&str> {
    depspec::license_name(s, "license name").map_err(|e| peg_error("invalid license name", s, e))
}

pub fn eclass_name(s: &str) -> crate::Result<&str> {
    depspec::eclass_name(s).map_err(|e| peg_error("invalid eclass name", s, e))
}

pub fn slot(s: &str) -> crate::Result<Slot<&str>> {
    depspec::slot(s).map_err(|e| peg_error("invalid slot", s, e))
}

pub(super) fn use_dep(s: &str) -> crate::Result<UseDep<&str>> {
    depspec::use_dep(s).map_err(|e| peg_error("invalid use dep", s, e))
}

pub(super) fn slot_dep(s: &str) -> crate::Result<SlotDep<&str>> {
    depspec::slot_dep(s).map_err(|e| peg_error("invalid slot", s, e))
}

pub fn use_flag(s: &str) -> crate::Result<&str> {
    depspec::use_flag(s).map_err(|e| peg_error("invalid USE flag", s, e))
}

pub(crate) fn iuse(s: &str) -> crate::Result<Iuse<&str>> {
    depspec::iuse(s).map_err(|e| peg_error("invalid IUSE", s, e))
}

pub(crate) fn keyword(s: &str) -> crate::Result<Keyword<&str>> {
    depspec::keyword(s).map_err(|e| peg_error("invalid KEYWORD", s, e))
}

pub fn repo(s: &str) -> crate::Result<&str> {
    depspec::repo(s).map_err(|e| peg_error("invalid repo name", s, e))
}

pub(super) fn cpv_str(s: &str) -> crate::Result<ParsedCpv> {
    depspec::cpv(s).map_err(|e| peg_error("invalid cpv", s, e))
}

pub(super) fn cpv(s: &str) -> crate::Result<Cpv> {
    cpv_str(s).into_owned()
}

pub(super) fn dep_str<'a>(s: &'a str, eapi: &'static Eapi) -> crate::Result<ParsedDep<'a>> {
    depspec::dep(s, eapi).map_err(|e| peg_error("invalid dep", s, e))
}

#[cached(
    type = "SizedCache<(String, &Eapi), crate::Result<Dep>>",
    create = "{ SizedCache::with_size(1000) }",
    convert = r#"{ (s.to_string(), eapi) }"#
)]
pub(crate) fn dep(s: &str, eapi: &'static Eapi) -> crate::Result<Dep> {
    dep_str(s, eapi).into_owned()
}

pub(super) fn cpn(s: &str) -> crate::Result<ParsedDep> {
    depspec::cpn(s).map_err(|e| peg_error("invalid unversioned dep", s, e))
}

pub fn license_dep_set(s: &str) -> crate::Result<DepSet<String, String>> {
    depspec::license_dep_set(s).map_err(|e| peg_error("invalid LICENSE", s, e))
}

pub fn license_dep_spec(s: &str) -> crate::Result<DepSpec<String, String>> {
    depspec::license_dep_spec(s).map_err(|e| peg_error("invalid LICENSE DepSpec", s, e))
}

pub fn src_uri_dep_set(s: &str, eapi: &'static Eapi) -> crate::Result<DepSet<String, Uri>> {
    depspec::src_uri_dep_set(s, eapi).map_err(|e| peg_error("invalid SRC_URI", s, e))
}

pub fn src_uri_dep_spec(s: &str, eapi: &'static Eapi) -> crate::Result<DepSpec<String, Uri>> {
    depspec::src_uri_dep_spec(s, eapi).map_err(|e| peg_error("invalid SRC_URI DepSpec", s, e))
}

pub fn properties_dep_set(s: &str) -> crate::Result<DepSet<String, String>> {
    depspec::properties_dep_set(s).map_err(|e| peg_error("invalid PROPERTIES", s, e))
}

pub fn properties_dep_spec(s: &str) -> crate::Result<DepSpec<String, String>> {
    depspec::properties_dep_spec(s).map_err(|e| peg_error("invalid PROPERTIES DepSpec", s, e))
}

pub fn required_use_dep_set(s: &str, eapi: &'static Eapi) -> crate::Result<DepSet<String, String>> {
    depspec::required_use_dep_set(s, eapi).map_err(|e| peg_error("invalid REQUIRED_USE", s, e))
}

pub fn required_use_dep_spec(
    s: &str,
    eapi: &'static Eapi,
) -> crate::Result<DepSpec<String, String>> {
    depspec::required_use_dep_spec(s, eapi)
        .map_err(|e| peg_error("invalid REQUIRED_USE DepSpec", s, e))
}

pub fn restrict_dep_set(s: &str) -> crate::Result<DepSet<String, String>> {
    depspec::restrict_dep_set(s).map_err(|e| peg_error("invalid RESTRICT", s, e))
}

pub fn restrict_dep_spec(s: &str) -> crate::Result<DepSpec<String, String>> {
    depspec::restrict_dep_spec(s).map_err(|e| peg_error("invalid RESTRICT DepSpec", s, e))
}

pub fn dependencies_dep_set(s: &str, eapi: &'static Eapi) -> crate::Result<DepSet<String, Dep>> {
    depspec::dependencies_dep_set(s, eapi).map_err(|e| peg_error("invalid dependency", s, e))
}

pub fn dependencies_dep_spec(s: &str, eapi: &'static Eapi) -> crate::Result<DepSpec<String, Dep>> {
    depspec::dependencies_dep_spec(s, eapi)
        .map_err(|e| peg_error("invalid dependency DepSpec", s, e))
}

#[cfg(test)]
mod tests {
    use crate::eapi::{self, EAPIS, EAPIS_OFFICIAL, EAPI_LATEST_OFFICIAL};

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

            let result = dep(&s, &eapi::EAPI_PKGCRAFT);
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
            assert!(license_dep_set(s).is_err(), "{s:?} didn't fail");
            assert!(license_dep_spec(s).is_err(), "{s:?} didn't fail");
        }

        // empty set
        assert!(license_dep_set("").unwrap().is_empty());

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
            let depset = license_dep_set(s).unwrap();
            assert_eq!(depset.to_string(), s);
            let flatten: Vec<_> = depset.iter_flatten().collect();
            assert_eq!(flatten, expected_flatten);
        }
    }

    #[test]
    fn src_uri() {
        // invalid
        for s in ["http://", "https://a/uri/with/no/filename/"] {
            assert!(src_uri_dep_set(s, &EAPI_LATEST_OFFICIAL).is_err(), "{s:?} didn't fail");
            assert!(src_uri_dep_spec(s, &EAPI_LATEST_OFFICIAL).is_err(), "{s:?} didn't fail");
        }

        // empty set
        assert!(src_uri_dep_set("", &EAPI_LATEST_OFFICIAL)
            .unwrap()
            .is_empty());

        // valid
        for (s, expected_flatten) in [
            ("uri", vec!["uri"]),
            ("http://uri", vec!["http://uri"]),
            ("uri1 uri2", vec!["uri1", "uri2"]),
            ("( http://uri1 http://uri2 )", vec!["http://uri1", "http://uri2"]),
            ("u1? ( http://uri1 !u2? ( http://uri2 ) )", vec!["http://uri1", "http://uri2"]),
        ] {
            for eapi in &*EAPIS {
                let depset = src_uri_dep_set(s, eapi).unwrap();
                assert_eq!(depset.to_string(), s);
                let flatten: Vec<_> = depset.iter_flatten().map(|x| x.to_string()).collect();
                assert_eq!(flatten, expected_flatten);
            }
        }

        // renames
        for (s, expected_flatten) in [
            ("http://uri -> file", vec!["http://uri -> file"]),
            ("u? ( http://uri -> file )", vec!["http://uri -> file"]),
        ] {
            for eapi in &*EAPIS {
                let depset = src_uri_dep_set(s, eapi).unwrap();
                assert_eq!(depset.to_string(), s);
                let flatten: Vec<_> = depset.iter_flatten().map(|x| x.to_string()).collect();
                assert_eq!(flatten, expected_flatten);
            }
        }
    }

    #[test]
    fn required_use() {
        // invalid
        for s in ["(", ")", "( )", "( u)", "| ( u )", "|| ( )", "^^ ( )", "?? ( )"] {
            assert!(required_use_dep_set(s, &EAPI_LATEST_OFFICIAL).is_err(), "{s:?} didn't fail");
            assert!(required_use_dep_spec(s, &EAPI_LATEST_OFFICIAL).is_err(), "{s:?} didn't fail");
        }

        // empty set
        assert!(required_use_dep_set("", &EAPI_LATEST_OFFICIAL)
            .unwrap()
            .is_empty());

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
            let depset = required_use_dep_set(s, &EAPI_LATEST_OFFICIAL).unwrap();
            assert_eq!(depset.to_string(), s);
            let flatten: Vec<_> = depset.iter_flatten().collect();
            assert_eq!(flatten, expected_flatten);
        }

        // ?? operator
        for (s, expected_flatten) in [("?? ( u1 u2 )", vec!["u1", "u2"])] {
            for eapi in &*EAPIS {
                let depset = required_use_dep_set(s, eapi).unwrap();
                assert_eq!(depset.to_string(), s);
                let flatten: Vec<_> = depset.iter_flatten().collect();
                assert_eq!(flatten, expected_flatten);
            }
        }
    }

    #[test]
    fn dependencies() {
        // invalid
        for s in ["(", ")", "( )", "|| ( )", "( a/b)", "| ( a/b )", "use ( a/b )", "!use ( a/b )"] {
            assert!(dependencies_dep_set(s, &EAPI_LATEST_OFFICIAL).is_err(), "{s:?} didn't fail");
            assert!(dependencies_dep_spec(s, &EAPI_LATEST_OFFICIAL).is_err(), "{s:?} didn't fail");
        }

        // empty set
        assert!(dependencies_dep_set("", &EAPI_LATEST_OFFICIAL)
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
            let depset = dependencies_dep_set(s, &EAPI_LATEST_OFFICIAL).unwrap();
            assert_eq!(depset.to_string(), s);
            let flatten: Vec<_> = depset.iter_flatten().map(|x| x.to_string()).collect();
            assert_eq!(flatten, expected_flatten);
        }
    }

    #[test]
    fn properties() {
        // invalid
        for s in ["(", ")", "( )", "( v)", "| ( v )", "!use ( v )", "|| ( v )", "|| ( v1 v2 )"] {
            assert!(properties_dep_set(s).is_err(), "{s:?} didn't fail");
            assert!(properties_dep_spec(s).is_err(), "{s:?} didn't fail");
        }

        // empty set
        assert!(properties_dep_set("").unwrap().is_empty());

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
            let depset = properties_dep_set(s).unwrap();
            assert_eq!(depset.to_string(), s);
            let flatten: Vec<_> = depset.iter_flatten().collect();
            assert_eq!(flatten, expected_flatten);
        }
    }

    #[test]
    fn restrict() {
        // invalid
        for s in ["(", ")", "( )", "( v)", "| ( v )", "!use ( v )", "|| ( v )", "|| ( v1 v2 )"] {
            assert!(restrict_dep_set(s).is_err(), "{s:?} didn't fail");
            assert!(restrict_dep_spec(s).is_err(), "{s:?} didn't fail");
        }

        // empty set
        assert!(restrict_dep_set("").unwrap().is_empty());

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
            let depset = restrict_dep_set(s).unwrap();
            assert_eq!(depset.to_string(), s);
            let flatten: Vec<_> = depset.iter_flatten().collect();
            assert_eq!(flatten, expected_flatten);
        }
    }
}
