use std::cmp::Ordering;

use crate::atom;
use crate::metadata::ebuild::{MaintainerRestrict, UpstreamRestrict};
use crate::peg::peg_error;
use crate::pkg::ebuild::Restrict as EbuildRestrict;
use crate::restrict::*;

fn orderedset_restrict(
    op: &str,
    vals: &[&str],
) -> Result<OrderedSetRestrict<String, Str>, &'static str> {
    let vals = vals.iter().map(|x| x.to_string()).collect();
    match op {
        "<" => Ok(OrderedSetRestrict::ProperSubset(vals)),
        "<=" => Ok(OrderedSetRestrict::Subset(vals)),
        "==" => Ok(OrderedSetRestrict::Equal(vals)),
        ">=" => Ok(OrderedSetRestrict::Superset(vals)),
        ">" => Ok(OrderedSetRestrict::ProperSuperset(vals)),
        "%" => Ok(OrderedSetRestrict::Disjoint(vals)),
        _ => Err("invalid set operator"),
    }
}

fn str_restrict(op: &str, s: &str) -> Result<Str, &'static str> {
    match op {
        "==" => Ok(Str::equal(s)),
        "!=" => Ok(Str::not(Str::equal(s))),
        "=~" => Str::regex(s).map_err(|_| "invalid regex"),
        "!~" => Str::regex(s).map_err(|_| "invalid regex"),
        _ => Err("invalid string operator"),
    }
}

fn len_restrict(op: &str, s: &str) -> Result<(Vec<Ordering>, usize), &'static str> {
    let cmps = match op {
        "<" => vec![Ordering::Less],
        "<=" => vec![Ordering::Less, Ordering::Equal],
        "==" => vec![Ordering::Equal],
        ">=" => vec![Ordering::Greater, Ordering::Equal],
        ">" => vec![Ordering::Greater],
        _ => return Err("unknown count operator"),
    };

    let size: usize = match s.parse() {
        Ok(v) => v,
        Err(_) => return Err("invalid count size"),
    };

    Ok((cmps, size))
}

fn missing_restrict(attr: &str) -> EbuildRestrict {
    use crate::pkg::ebuild::Restrict::*;
    match attr {
        "subslot" => RawSubslot(None),
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
        "defined_phases" => DefinedPhases(None),
        "keywords" => Keywords(None),
        "iuse" => Iuse(None),
        "inherited" => Inherited(None),
        "inherit" => Inherit(None),
        "long_description" => LongDescription(None),
        "maintainers" => Maintainers(None),
        "upstreams" => Upstreams(None),
        _ => panic!("unknown optional package attribute: {attr}"),
    }
}

fn dep_restrict(attr: &str, r: atom::Restrict) -> EbuildRestrict {
    use crate::depset::Restrict::*;
    use crate::pkg::ebuild::Restrict::*;

    match attr {
        "depend" => Depend(Some(Any(r))),
        "bdepend" => Bdepend(Some(Any(r))),
        "idepend" => Idepend(Some(Any(r))),
        "pdepend" => Pdepend(Some(Any(r))),
        "rdepend" => Rdepend(Some(Any(r))),
        _ => panic!("unknown dep attribute: {attr}"),
    }
}

type LogicRestrict = fn(Vec<Box<EbuildRestrict>>) -> EbuildRestrict;

fn logic_r(func: LogicRestrict, restricts: Vec<EbuildRestrict>) -> EbuildRestrict {
    func(restricts.into_iter().map(Box::new).collect())
}

peg::parser!(grammar restrict() for str {
    rule optional_attr() -> &'input str
        = attr:$((
            "subslot"
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
            / "defined_phases"
            / "keywords"
            / "iuse"
            / "inherited"
            / "inherit"
            / "long_description"
            / "maintainers"
            / "upstreams"
        )) { attr }

    rule attr_optional() -> Restrict
        = attr:optional_attr() is_op() ("None" / "none")
        {
            missing_restrict(attr).into()
        } / vals:(op:['&' | '|'] attr:optional_attr() { (op, attr) }) **<2,> ""
            is_op() ("None" / "none")
        {
            use crate::pkg::ebuild::Restrict::{And, Or};
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
                ([..], []) => logic_r(And, and_restricts).into(),
                ([], [..]) => logic_r(Or, or_restricts).into(),
                ([..], [..]) => Restrict::and(
                    [logic_r(And, and_restricts), logic_r(Or, or_restricts)]),
                _ => panic!("missing optional attr restrictions"),
            }
        }

    rule quoted_string() -> &'input str
        = "\"" s:$([^ '\"']+) "\"" { s }
        / "\'" s:$([^ '\'']+) "\'" { s }

    rule string_ops() -> &'input str
        = _* op:$("==" / "!=" / "=~" / "!~") _* { op }

    rule set_ops() -> &'input str
        = _* op:$((['<' | '>'] "="?) / "==" / "%") _* { op }

    rule quoted_string_set() -> Vec<&'input str>
        = _* "{" vals:(quoted_string() ** (_* "," _*)) "}" _* { vals }

    rule number_ops() -> &'input str
        = _* op:$((['<' | '>'] "="?) / "==") _* { op }

    rule atom_str_restrict() -> Restrict
        = attr:$((
                "category"
                / "package"
                / "version"
            )) op:string_ops() s:quoted_string()
        {?
            use crate::atom::Restrict::*;
            let r = str_restrict(op, s)?;
            match attr {
                "category" => Ok(Category(r).into()),
                "package" => Ok(Package(r).into()),
                "version" => Ok(VersionStr(r).into()),
                _ => Err("unknown atom attribute"),
            }
        }

    rule pkg_restrict() -> Restrict
        = attr:$(("eapi" / "repo")) op:string_ops() s:quoted_string()
        {?
            use crate::pkg::Restrict::*;
            let r = str_restrict(op, s)?;
            match attr {
                "eapi" => Ok(Eapi(r).into()),
                "repo" => Ok(Repo(r).into()),
                _ => Err("unknown package attribute"),
            }
        }

    rule attr_str_restrict() -> Restrict
        = attr:$((
                "ebuild"
                / "description"
                / "slot"
                / "subslot"
                / "long_description"
            )) op:string_ops() s:quoted_string()
        {?
            use crate::pkg::ebuild::Restrict::*;
            let r = str_restrict(op, s)?;
            let ebuild_r = match attr {
                "ebuild" => Ebuild(r),
                "description" => Description(r),
                "slot" => Slot(r),
                "subslot" => Subslot(r),
                "long_description" => LongDescription(Some(r)),
                _ => return Err("unknown package attribute"),
            };
            Ok(ebuild_r.into())
        }

    rule dep_attr() -> &'input str
        = attr:$((
            "depend"
            / "bdepend"
            / "idepend"
            / "pdepend"
            / "rdepend"
        )) { attr }

    rule attr_dep_restrict() -> Restrict
        = attr:dep_attr() _ "any" _ s:quoted_string()
        {?
            let atom_r = match super::parse::dep(s) {
                Ok(Restrict::Atom(r)) => r,
                _ => return Err("invalid dep restriction"),
            };

            Ok(dep_restrict(attr, atom_r).into())
        } / vals:(op:['&' | '|'] attr:dep_attr() { (op, attr) }) **<2,> ""
            _ "any" _ s:quoted_string()
        {?
            use crate::pkg::ebuild::Restrict::{And, Or};
            let mut and_restricts = vec![];
            let mut or_restricts = vec![];

            let atom_r = match super::parse::dep(s) {
                Ok(Restrict::Atom(r)) => r,
                _ => return Err("invalid dep restriction"),
            };

            for (op, attr) in vals {
                match op {
                    '&' => and_restricts.push(dep_restrict(attr, atom_r.clone())),
                    '|' => or_restricts.push(dep_restrict(attr, atom_r.clone())),
                    _ => panic!("unknown operator: {op}"),
                }
            }

            match (&and_restricts[..], &or_restricts[..]) {
                ([..], []) => Ok(logic_r(And, and_restricts).into()),
                ([], [..]) => Ok(logic_r(Or, or_restricts).into()),
                ([..], [..]) => Ok(Restrict::and(
                    [logic_r(And, and_restricts), logic_r(Or, or_restricts)])),
                _ => panic!("missing optional attr restrictions"),
            }
        }

    rule attr_orderedset_str() -> Restrict
        = attr:$((
                "homepage"
                / "defined_phases"
                / "keywords"
                / "iuse"
                / "inherited"
                / "inherit"
            )) op:set_ops() vals:quoted_string_set()
        {?
            use crate::pkg::ebuild::Restrict::*;
            let r = orderedset_restrict(op, &vals)?;
            let ebuild_r = match attr {
                "homepage" => Homepage(Some(r)),
                "keywords" => Keywords(Some(r)),
                "iuse" => Iuse(Some(r)),
                "inherited" => Inherited(Some(r)),
                "inherit" => Inherit(Some(r)),
                _ => return Err("unknown package attribute"),
            };
            Ok(ebuild_r.into())
        }

    rule count<T>() -> OrderedRestrict<T>
        = op:number_ops() count:$(['0'..='9']+)
        {?
            let (cmps, size) = len_restrict(op, count)?;
            Ok(OrderedRestrict::Count(cmps, size))
        }

    rule ordered_ops<T>(exprs: rule<T>) -> OrderedRestrict<T>
        = _ op:$(("any" / "all" / "first" / "last")) _ r:(exprs())
        {?
            use crate::restrict::OrderedRestrict::*;
            let r = match op {
                "any" => Any(r),
                "all" => All(r),
                "first" => First(r),
                "last" => Last(r),
                _ => return Err("unknown upstreams operation"),
            };
            Ok(r)
        }

    rule maintainers() -> Restrict
        = "maintainers" r:(ordered_ops(<maintainer_exprs()>) / count())
        { r.into() }

    rule maintainer_exprs() -> MaintainerRestrict
        = r:(maintainer_attr_optional()
             / maintainer_restrict()
             / parens(<maintainer_and()>)
        ) { r }

    rule maintainer_attr_optional() -> MaintainerRestrict
        = attr:$(("name" / "description" / "type" / "proxied"))
                is_op() ("None" / "none")
        {?
            use crate::metadata::ebuild::MaintainerRestrict::*;
            let r = match attr {
                "name" => Name(None),
                "description" => Description(None),
                "type" => Type(None),
                "proxied" => Proxied(None),
                _ => return Err("unknown optional maintainer attribute"),
            };
            Ok(r)
        }

    rule maintainer_restrict() -> MaintainerRestrict
        = attr:$(("email" / "name" / "description" / "type" / "proxied"))
            op:string_ops() s:quoted_string()
        {?
            use crate::metadata::ebuild::MaintainerRestrict::*;
            let r = str_restrict(op, s)?;
            match attr {
                "email" => Ok(Email(r)),
                "name" => Ok(Name(Some(r))),
                "description" => Ok(Description(Some(r))),
                "type" => Ok(Type(Some(r))),
                "proxied" => Ok(Proxied(Some(r))),
                _ => Err("unknown maintainer attribute"),
            }
        }

    rule maintainer_and() -> MaintainerRestrict
        = exprs:(maintainer_attr_optional() / maintainer_restrict()) ++ (_* "&&" _*)
        {
            use crate::metadata::ebuild::MaintainerRestrict::And;
            And(exprs.into_iter().map(Box::new).collect())
        }

    rule upstreams() -> Restrict
        = "upstreams" r:(ordered_ops(<upstream_exprs()>) / count())
        { r.into() }

    rule upstream_exprs() -> UpstreamRestrict
        = r:(upstream_restrict() / parens(<upstream_and()>)) { r }

    rule upstream_restrict() -> UpstreamRestrict
        = attr:$(("site" / "name"))
            op:string_ops() s:quoted_string()
        {?
            use crate::metadata::ebuild::UpstreamRestrict::*;
            let r = str_restrict(op, s)?;
            match attr {
                "site" => Ok(Site(r)),
                "name" => Ok(Name(r)),
                _ => Err("unknown upstream attribute"),
            }
        }

    rule upstream_and() -> UpstreamRestrict
        = exprs:upstream_restrict() ++ (_* "&&" _*)
        {
            use crate::metadata::ebuild::UpstreamRestrict::And;
            And(exprs.into_iter().map(Box::new).collect())
        }

    rule _ = quiet!{[' ' | '\n' | '\t']+}

    rule parens<T>(expr: rule<T>) -> T = _* "(" _* v:expr() _* ")" _* { v }
    rule is_op() = _ "is" _

    rule expression() -> Restrict
        = r:(attr_optional()
           / atom_str_restrict()
           / attr_str_restrict()
           / attr_dep_restrict()
           / attr_orderedset_str()
           / maintainers()
           / upstreams()
           / pkg_restrict()
        ) { r }

    pub(super) rule query() -> Restrict = precedence!{
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
pub fn pkg(s: &str) -> crate::Result<Restrict> {
    restrict::query(s).map_err(|e| peg_error(format!("invalid package query: {s:?}"), s, e))
}
