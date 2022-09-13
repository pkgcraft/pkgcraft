use std::cmp::Ordering;

use regex::Regex;

use crate::metadata::ebuild::{MaintainerRestrict, UpstreamRestrict};
use crate::peg::peg_error;
use crate::restrict::*;

fn set_restrict<S: FromIterator<String>>(
    op: &str,
    vals: &[&str],
) -> Result<SetRestrict<S, String>, &'static str> {
    let vals = vals.iter().map(|x| x.to_string()).collect();
    match op {
        "<" => Ok(SetRestrict::ProperSubset(vals)),
        "<=" => Ok(SetRestrict::Subset(vals)),
        "==" => Ok(SetRestrict::Equal(vals)),
        ">=" => Ok(SetRestrict::Superset(vals)),
        ">" => Ok(SetRestrict::ProperSuperset(vals)),
        "%" => Ok(SetRestrict::Disjoint(vals)),
        _ => Err("invalid set operator"),
    }
}

fn str_restrict(op: &str, s: &str) -> Result<Str, &'static str> {
    match op {
        "==" => Ok(Str::equal(s)),
        "!=" => Ok(Str::not(Str::equal(s))),
        "=~" => {
            let re = Regex::new(s).map_err(|_| "invalid regex")?;
            Ok(Str::Regex(re))
        }
        "!~" => {
            let re = Regex::new(s).map_err(|_| "invalid regex")?;
            Ok(Str::not(Str::Regex(re)))
        }
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

peg::parser!(grammar restrict() for str {
    rule attr_optional() -> Restrict
        = attr:$((
                "subslot"
                / "depend"
                / "bdepend"
                / "idepend"
                / "pdepend"
                / "rdepend"
                / "license"
                / "required_use"
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
            )) is_op() ("None" / "none")
        {?
            use crate::pkg::ebuild::Restrict::*;
            let r = match attr {
                "subslot" => RawSubslot(None),
                "depend" => Depend(None),
                "bdepend" => Bdepend(None),
                "idepend" => Idepend(None),
                "pdepend" => Pdepend(None),
                "rdepend" => Rdepend(None),
                "license" => License(None),
                "required_use" => RequiredUse(None),
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
                _ => return Err("unknown optional package attribute"),
            };
            Ok(r.into())
        }

    rule quoted_string() -> &'input str
        = "\"" s:$([^ '\"']+) "\"" { s }
        / "\'" s:$([^ '\'']+) "\'" { s }

    rule string_ops() -> &'input str
        = _ op:$("==" / "!=" / "=~" / "!~") _ { op }

    rule set_ops() -> &'input str
        = _ op:$((['<' | '>'] "="?) / "==" / "%") _ { op }

    rule quoted_string_set() -> Vec<&'input str>
        = _ "{" e:(quoted_string() ** (_ "," _)) "}" _
        { e }

    rule number_ops() -> &'input str
        = _ op:$((['<' | '>'] "="?) / "==") _ { op }

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

    rule attr_orderedset_str() -> Restrict
        = attr:$((
                "homepage"
                / "keywords"
                / "iuse"
                / "inherited"
                / "inherit"
            )) op:set_ops() vals:quoted_string_set()
        {?
            use crate::pkg::ebuild::Restrict::*;
            let r = IndexSetRestrict::Set(set_restrict(op, &vals)?);
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

    rule attr_hashset_str() -> Restrict
        = "defined_phases" op:set_ops() vals:quoted_string_set() {?
            use crate::pkg::ebuild::Restrict::*;
            let r: HashSetRestrict<_> = set_restrict(op, &vals)?;
            Ok(DefinedPhases(Some(r)).into())
        }

    rule count<T>() -> OrderedRestrict<T>
        = op:number_ops() count:$(['0'..='9']+)
        {?
            let (cmps, size) = len_restrict(op, count)?;
            Ok(OrderedRestrict::Count(cmps, size))
        }

    rule ordered_ops<T>(exprs: rule<T>) -> OrderedRestrict<T>
        = __ op:$(("any" / "all" / "first" / "last")) __ r:(exprs())
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
        = exprs:(maintainer_attr_optional() / maintainer_restrict()) ++ (_ "&&" _)
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
        = exprs:upstream_restrict() ++ (_ "&&" _)
        {
            use crate::metadata::ebuild::UpstreamRestrict::And;
            And(exprs.into_iter().map(Box::new).collect())
        }

    rule _ = quiet!{[' ' | '\n' | '\t']*}
    rule __ = quiet!{[' ' | '\n' | '\t']+}

    rule parens<T>(expr: rule<T>) -> T = _ "(" _ v:expr() _ ")" _ { v }
    rule is_op() = __ "is" __

    rule expr() -> Restrict
        = r:(attr_optional()
           / atom_str_restrict()
           / attr_str_restrict()
           / attr_orderedset_str()
           / attr_hashset_str()
           / maintainers()
           / upstreams()
           / pkg_restrict()
        ) { r }

    pub(super) rule query() -> Restrict = precedence!{
        x:(@) _ "||" _ y:@ { x | y }
        --
        x:(@) _ "^^" _ y:@ { x ^ y }
        --
        x:(@) _ "&&" _ y:@ { x & y }
        --
        "!" x:(@) { !x }
        --
        v:parens(<query()>) { v }
        e:expr() { e }
    }
});

/// Convert a package query string into a Restriction.
pub fn pkg(s: &str) -> crate::Result<Restrict> {
    restrict::query(s).map_err(|e| peg_error(format!("invalid package query: {s:?}"), s, e))
}
