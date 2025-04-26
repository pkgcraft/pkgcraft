use std::cmp::Ordering;

use crate::error::peg_error;
use crate::pkg::ebuild::{MaintainerRestrict, Restrict as EbuildRestrict};

use crate::restrict::Restrict as BaseRestrict;
use crate::restrict::dep::Restrict as DepRestrict;
use crate::restrict::depset::Restrict as DepSetRestrict;
use crate::restrict::ordered::Restrict as OrderedRestrict;
use crate::restrict::set::OrderedSetRestrict;
use crate::restrict::str::Restrict as StrRestrict;

use super::dep;

// Convert string to regex restriction, with no metacharacter escaping.
fn str_to_regex_restrict(s: &str) -> Result<StrRestrict, &'static str> {
    StrRestrict::regex(s).map_err(|_| "invalid regex")
}

fn orderedset_restrict(op: &str, vals: &[&str]) -> OrderedSetRestrict<String, StrRestrict> {
    let func = match op {
        "<" => OrderedSetRestrict::ProperSubset,
        "<=" => OrderedSetRestrict::Subset,
        "==" => OrderedSetRestrict::Equal,
        ">=" => OrderedSetRestrict::Superset,
        ">" => OrderedSetRestrict::ProperSuperset,
        "%" => OrderedSetRestrict::Disjoint,
        _ => panic!("invalid set operator: {op}"),
    };
    func(vals.iter().map(|x| x.to_string()).collect())
}

fn str_restrict(op: &str, s: &str) -> Result<StrRestrict, &'static str> {
    match op {
        "==" => Ok(StrRestrict::equal(s)),
        ">=" => Ok(StrRestrict::substr(s)),
        "!=" => Ok(StrRestrict::not(StrRestrict::equal(s))),
        "=~" => str_to_regex_restrict(s),
        "!~" => Ok(StrRestrict::not(str_to_regex_restrict(s)?)),
        _ => panic!("invalid string operator: {op}"),
    }
}

fn missing_restrict(attr: &str) -> EbuildRestrict {
    use EbuildRestrict::*;
    match attr {
        "subslot" => RawSubslot(None),
        "dependencies" => Dependencies(None),
        "depend" => Depend(None),
        "bdepend" => Bdepend(None),
        "idepend" => Idepend(None),
        "pdepend" => Pdepend(None),
        "rdepend" => Rdepend(None),
        "license" => License(None),
        "properties" => Properties(None),
        "required_use" => RequiredUse(None),
        "restrict" => Restrict(None),
        "src_uri" => SrcUri(None),
        "homepage" => Homepage(None),
        "iuse" => Iuse(None),
        "inherit" => Inherit(None),
        "inherited" => Inherited(None),
        "keywords" => Keywords(None),
        "long_description" => LongDescription(None),
        "maintainers" => Maintainers(None),
        _ => panic!("unknown optional package attribute: {attr}"),
    }
}

fn depset_dep_any(attr: &str, r: DepRestrict) -> EbuildRestrict {
    use DepSetRestrict::*;
    use EbuildRestrict::*;

    match attr {
        "dependencies" => Dependencies(Some(Any(r))),
        "depend" => Depend(Some(Any(r))),
        "bdepend" => Bdepend(Some(Any(r))),
        "idepend" => Idepend(Some(Any(r))),
        "pdepend" => Pdepend(Some(Any(r))),
        "rdepend" => Rdepend(Some(Any(r))),
        _ => panic!("unknown depset dep attribute: {attr}"),
    }
}

fn depset_dep_contains(attr: &str, r: StrRestrict) -> EbuildRestrict {
    use DepSetRestrict::*;
    use EbuildRestrict::*;

    match attr {
        "dependencies" => Dependencies(Some(Contains(r))),
        "depend" => Depend(Some(Contains(r))),
        "bdepend" => Bdepend(Some(Contains(r))),
        "idepend" => Idepend(Some(Contains(r))),
        "pdepend" => Pdepend(Some(Contains(r))),
        "rdepend" => Rdepend(Some(Contains(r))),
        _ => panic!("unknown depset dep attribute: {attr}"),
    }
}

fn depset_str_restrict(kind: &str, attr: &str, r: StrRestrict) -> EbuildRestrict {
    use DepSetRestrict::*;
    use EbuildRestrict::*;

    let depset_restrict = match kind {
        "any" => Any,
        "contains" => Contains,
        _ => panic!("unknown depset restriction type: {kind}"),
    };

    match attr {
        "license" => License(Some(depset_restrict(r))),
        "properties" => Properties(Some(depset_restrict(r))),
        "required_use" => RequiredUse(Some(depset_restrict(r))),
        "restrict" => Restrict(Some(depset_restrict(r))),
        _ => panic!("unknown depset string attribute: {attr}"),
    }
}

peg::parser!(grammar restrict() for str {
    rule optional_attr() -> &'input str
        = attr:$((
            "subslot"
            / "dependencies"
            / "depend"
            / "bdepend"
            / "idepend"
            / "pdepend"
            / "rdepend"
            / "license"
            / "properties"
            / "required_use"
            / "restrict"
            / "src_uri"
            / "homepage"
            / "iuse"
            / "inherited"
            / "inherit"
            / "keywords"
            / "long_description"
            / "maintainers"
            / "upstream"
        )) { attr }

    rule attr_optional() -> BaseRestrict
        = attr:optional_attr() is_op() ("None" / "none")
        {
            missing_restrict(attr).into()
        } / vals:(op:['&' | '|'] attr:optional_attr() { (op, attr) }) **<2,> ""
            is_op() ("None" / "none")
        {
            let mut and_restricts = vec![];
            let mut or_restricts = vec![];

            for (op, attr) in vals {
                match op {
                    '&' => and_restricts.push(missing_restrict(attr)),
                    '|' => or_restricts.push(missing_restrict(attr)),
                    _ => panic!("unknown operator: {op}"),
                }
            }

            match (&and_restricts[..], &or_restricts[..]) {
                ([..], []) => BaseRestrict::and(and_restricts),
                ([], [..]) => BaseRestrict::or(or_restricts),
                ([..], [..]) => BaseRestrict::and(
                    [BaseRestrict::and(and_restricts), BaseRestrict::or(or_restricts)]),
                _ => panic!("missing optional attr restrictions"),
            }
        }

    rule quoted_string() -> &'input str
        = "\"" s:$([^'\"']+) "\"" { s }
        / "\'" s:$([^'\'']+) "\'" { s }

    rule string_ops() -> &'input str
        = _* op:$("==" / ">=" / "!=" / "=~" / "!~") _* { op }

    rule set_ops() -> &'input str
        = _* op:$((['<' | '>'] "="?) / "==" / "%") _* { op }

    rule quoted_string_set() -> Vec<&'input str>
        = _* "{" vals:(quoted_string() ** (_* "," _*)) "}" _* { vals }

    rule number_ops() -> Vec<Ordering>
        = _* op:$((['<' | '>'] "="?) / (['!' | '='] "=")) _* {?
            let cmps = match op {
                "<" => vec![Ordering::Less],
                "<=" => vec![Ordering::Less, Ordering::Equal],
                "==" => vec![Ordering::Equal],
                "!=" => vec![Ordering::Less, Ordering::Greater],
                ">=" => vec![Ordering::Greater, Ordering::Equal],
                ">" => vec![Ordering::Greater],
                _ => panic!("unknown count operator: {op}"),
            };

            Ok(cmps)
        }

    rule dep_str_restrict() -> BaseRestrict
        = attr:$((
                "category"
                / "package"
            )) op:string_ops() s:quoted_string()
        {?
            use DepRestrict::*;
            let r = str_restrict(op, s)?;
            match attr {
                "category" => Ok(Category(r).into()),
                "package" => Ok(Package(r).into()),
                _ => panic!("unknown dep attribute: {attr}"),
            }
        }

    rule pkg_restrict() -> BaseRestrict
        = attr:$(("eapi" / "repo")) op:string_ops() s:quoted_string()
        {?
            use crate::eapi::Restrict::*;
            use crate::pkg::Restrict::*;
            let r = str_restrict(op, s)?;
            match attr {
                "eapi" => Ok(Eapi(Id(r)).into()),
                "repo" => Ok(Repo(r).into()),
                _ => panic!("unknown package attribute: {attr}"),
            }
        }

    rule attr_str_restrict() -> BaseRestrict
        = attr:$((
                "ebuild"
                / "description"
                / "slot"
                / "subslot"
                / "long_description"
            )) op:string_ops() s:quoted_string()
        {?
            use EbuildRestrict::*;
            let r = str_restrict(op, s)?;
            let ebuild_r = match attr {
                "ebuild" => Ebuild(r),
                "description" => Description(r),
                "slot" => Slot(r),
                "subslot" => Subslot(r),
                "long_description" => LongDescription(Some(r)),
                _ => panic!("unknown package attribute: {attr}"),
            };
            Ok(ebuild_r.into())
        }

    rule depset_restrict() -> &'input str
        = kind:$(("any" / "contains")) { kind }

    rule depset_dep_attr() -> &'input str
        = attr:$((
            "dependencies"
            / "depend"
            / "bdepend"
            / "idepend"
            / "pdepend"
            / "rdepend"
        )) { attr }

    rule attr_depset_dep_restrict() -> BaseRestrict
        = attr:depset_dep_attr() _ "any" _ s:quoted_string()
        {?
            let restricts = dep::restricts(s)
                .map_err(|_| "invalid dep restriction")?
                .into_iter()
                .map(|r| depset_dep_any(attr, r));
            Ok(BaseRestrict::and(restricts))
        } / attr:depset_dep_attr() _ "contains" _ op:string_ops() s:quoted_string()
        {?
            let r = str_restrict(op, s)?;
            Ok(depset_dep_contains(attr, r).into())
        } / vals:(op:['&' | '|'] attr:depset_dep_attr() { (op, attr) }) **<2,> ""
            _ "any" _ s:quoted_string()
        {?
            let mut and_restricts = vec![];
            let mut or_restricts = vec![];

            let restricts = dep::restricts(s)
                .map_err(|_| "invalid dep restriction")?;

            for (op, attr) in vals {
                let restricts = restricts.iter().cloned().map(|r| depset_dep_any(attr, r));
                match op {
                    '&' => and_restricts.extend(restricts),
                    '|' => or_restricts.extend(restricts),
                    _ => panic!("unknown operator: {op}"),
                }
            }

            let r = match (&and_restricts[..], &or_restricts[..]) {
                ([..], []) => BaseRestrict::and(and_restricts),
                ([], [..]) => BaseRestrict::or(or_restricts),
                ([..], [..]) => BaseRestrict::and(
                    [BaseRestrict::and(and_restricts), BaseRestrict::or(or_restricts)]),
                _ => panic!("missing optional attr restrictions"),
            };

            Ok(r)
        }

    rule depset_str_attr() -> &'input str
        = attr:$((
            "license"
            / "properties"
            / "required_use"
            / "restrict"
        )) { attr }

    rule attr_depset_str_restrict() -> BaseRestrict
        = attr:depset_str_attr() _ kind:depset_restrict() _ op:string_ops() s:quoted_string()
        {?
            let r = str_restrict(op, s)?;
            Ok(depset_str_restrict(kind, attr, r).into())
        }

    rule attr_orderedset_str() -> BaseRestrict
        = attr:$((
                "homepage"
                / "iuse"
                / "inherited"
                / "inherit"
                / "keywords"
            )) op:set_ops() vals:quoted_string_set()
        {
            use EbuildRestrict::*;
            let func = match attr {
                "homepage" => Homepage,
                "iuse" => Iuse,
                "inherit" => Inherit,
                "inherited" => Inherited,
                "keywords" => Keywords,
                _ => panic!("unknown package attribute: {attr}"),
            };
            let r = orderedset_restrict(op, &vals);
            func(Some(r)).into()
        }

    rule count<T>() -> OrderedRestrict<T>
        = cmps:number_ops() s:$(['0'..='9']+)
        {?
            let size = s.parse().map_err(|_| "invalid count size")?;
            Ok(OrderedRestrict::Count(cmps, size))
        }

    rule ordered_ops<T>(exprs: rule<T>) -> OrderedRestrict<T>
        = _ op:$(("any" / "all" / "first" / "last")) _ r:(exprs())
        {
            use OrderedRestrict::*;
            match op {
                "any" => Any(r),
                "all" => All(r),
                "first" => First(r),
                "last" => Last(r),
                _ => panic!("unknown ordered operation: {op}"),
            }
        }

    rule maintainers() -> BaseRestrict
        = "maintainers" r:(ordered_ops(<maintainer_exprs()>) / count())
        { r.into() }

    rule maintainer_exprs() -> MaintainerRestrict
        = r:(maintainer_attr_optional()
             / maintainer_restrict()
             / parens(<maintainer_and()>)
        ) { r }

    rule maintainer_attr_optional() -> MaintainerRestrict
        = attr:$(("name" / "description")) is_op() ("None" / "none")
        {
            use MaintainerRestrict::*;
            match attr {
                "name" => Name(None),
                "description" => Description(None),
                _ => panic!("unknown optional maintainer attribute: {attr}"),
            }
        }

    rule maintainer_restrict() -> MaintainerRestrict
        = attr:$(("email" / "name" / "description" / "type" / "proxied"))
            op:string_ops() s:quoted_string()
        {?
            use MaintainerRestrict::*;
            let r = str_restrict(op, s)?;
            match attr {
                "email" => Ok(Email(r)),
                "name" => Ok(Name(Some(r))),
                "description" => Ok(Description(Some(r))),
                "type" => Ok(Type(r)),
                "proxied" => Ok(Proxied(r)),
                _ => panic!("unknown maintainer attribute: {attr}"),
            }
        }

    rule maintainer_and() -> MaintainerRestrict
        = exprs:(maintainer_attr_optional() / maintainer_restrict()) ++ (_* "&&" _*)
        { MaintainerRestrict::and(exprs) }

    rule _ = quiet!{[' ' | '\n' | '\t']+}

    rule parens<T>(expr: rule<T>) -> T = _* "(" _* v:expr() _* ")" _* { v }
    rule is_op() = _ "is" _

    rule expression() -> BaseRestrict
        = r:(attr_optional()
           / dep_str_restrict()
           / attr_str_restrict()
           / attr_depset_dep_restrict()
           / attr_depset_str_restrict()
           / attr_orderedset_str()
           / maintainers()
           / pkg_restrict()
        ) { r }

    pub(super) rule query() -> BaseRestrict = precedence!{
        x:(@) _* "||" _* y:@ { x | y }
        --
        x:(@) _* "^^" _* y:@ { x ^ y }
        --
        x:(@) _* "&&" _* y:@ { x & y }
        --
        "!" x:(@) { !x }
        --
        v:parens(<query()>) { v }
        e:expression() { e }
    }
});

/// Convert a package query string into a Restriction.
pub fn pkg(s: &str) -> crate::Result<BaseRestrict> {
    restrict::query(s).map_err(|e| peg_error("invalid package query", s, e))
}
