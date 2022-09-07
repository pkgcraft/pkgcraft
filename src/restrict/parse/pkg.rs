use std::cmp::Ordering;

use regex::Regex;

use crate::metadata::ebuild::{MaintainerRestrict, UpstreamRestrict};
use crate::peg::peg_error;
use crate::restrict::{Restrict, SliceRestrict, Str};

fn str_restrict(op: &str, s: &str) -> Result<Str, &'static str> {
    match op {
        "==" => Ok(Str::matches(s)),
        "!=" => Ok(Str::not(Str::matches(s))),
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
                / "homepage"
                / "defined_phases"
                / "keywords"
                / "iuse"
                / "inherit"
                / "inherited"
                / "long_description"
                / "maintainers"
                / "upstreams"
            )) is_op() ("None" / "none")
        {?
            use crate::pkg::ebuild::Restrict::*;
            let r = match attr {
                "subslot" => RawSubslot(None),
                "homepage" => Homepage(None),
                "defined_phases" => DefinedPhases(None),
                "keywords" => Keywords(None),
                "iuse" => Iuse(None),
                "inherit" => Inherit(None),
                "inherited" => Inherited(None),
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
        = opt_ws() op:$("==" / "!=" / "=~" / "!~") opt_ws() { op }

    rule number_ops() -> &'input str
        = opt_ws() op:$((['<' | '>'] "="?) / "==") opt_ws() { op }

    rule pkg_restrict() -> Restrict
        = attr:$(("eapi" / "repo")) op:string_ops() s:quoted_string() {?
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
                / "category"
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
                "category" => Category(r),
                "description" => Description(r),
                "slot" => Slot(r),
                "subslot" => Subslot(r),
                "long_description" => LongDescription(Some(r)),
                _ => return Err("unknown package attribute"),
            };
            Ok(ebuild_r.into())
        }

    rule slice_count<T>() -> SliceRestrict<T>
        = op:number_ops() count:$(['0'..='9']+) {?
            let (cmps, size) = len_restrict(op, count)?;
            Ok(SliceRestrict::Count(cmps, size))
        }

    rule slice_ops<T>(x: rule<T>) -> SliceRestrict<T>
        = ws() op:$(("contains" / "first" / "last")) ws()
            r:(x())
        {?
            use crate::restrict::SliceRestrict::*;
            let r = match op {
                "contains" => Contains(r),
                "first" => First(r),
                "last" => Last(r),
                _ => return Err("unknown upstreams operation"),
            };
            Ok(r)
        }

    rule maintainers() -> Restrict
        = "maintainers" r:(slice_ops(<maintainer_exprs()>) / slice_count())
        { r.into() }

    rule maintainer_exprs() -> MaintainerRestrict
        = r:(maintainer_attr_optional() / maintainer_restrict() / maintainer_and()) { r }

    rule maintainer_attr_optional() -> MaintainerRestrict
        = attr:$(("name" / "description" / "type" / "proxied"))
                is_op() ("None" / "none") {?
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
        = lparen() exprs:(
                maintainer_attr_optional()
                / maintainer_restrict()
            ) ++ (ws() "&&" ws()) rparen()
        {
            use crate::metadata::ebuild::MaintainerRestrict::And;
            And(exprs.into_iter().map(Box::new).collect())
        }

    rule upstreams() -> Restrict
        = "upstreams" r:(slice_ops(<upstream_exprs()>) / slice_count())
        { r.into() }

    rule upstream_exprs() -> UpstreamRestrict
        = r:(upstream_restrict() / upstream_and()) { r }

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
        = lparen() exprs:upstream_restrict() ++ (ws() "&&" ws()) rparen()
        {
            use crate::metadata::ebuild::UpstreamRestrict::And;
            And(exprs.into_iter().map(Box::new).collect())
        }

    rule ws() = quiet!{[' ' | '\n' | '\t']+}
    rule opt_ws() = quiet!{[' ' | '\n' | '\t']*}

    rule lparen() = opt_ws() "(" opt_ws()
    rule rparen() = opt_ws() ")" opt_ws()
    rule is_op() = ws() "is" ws()

    rule expr() -> Restrict
        = r:(attr_optional()
           / pkg_restrict()
           / attr_str_restrict()
           / maintainers()
           / upstreams()
        ) { r }

    pub(crate) rule query() -> Restrict = precedence!{
        x:(@) opt_ws() "||" opt_ws() y:@ { Restrict::or([x, y]) }
        --
        x:(@) opt_ws() "^^" opt_ws() y:@ { Restrict::xor([x, y]) }
        --
        x:(@) opt_ws() "&&" opt_ws() y:@ { Restrict::and([x, y]) }
        --
        "!" x:(@) { Restrict::not(x) }
        --
        lparen() v:query() rparen() { v }
        e:expr() { e }
    }
});

/// Convert a package query string into a Restriction.
pub fn pkg(s: &str) -> crate::Result<Restrict> {
    restrict::query(s).map_err(|e| peg_error(format!("invalid package query: {s:?}"), s, e))
}
