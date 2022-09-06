use std::cmp::Ordering;

use regex::Regex;

use crate::metadata::ebuild::{MaintainerRestrict, SliceMaintainers};
use crate::peg::peg_error;
use crate::restrict::{Restrict, Str};

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

peg::parser! {
    grammar restrict() for str {
        rule attr_optional() -> Restrict
            = attr:$((
                    "raw_subslot"
                    / "homepage"
                    / "defined_phases"
                    / "keywords"
                    / "iuse"
                    / "inherit"
                    / "inherited"
                    / "long_description"
                    / "maintainers"
                    / "upstreams"
                )) " is " ("None" / "none")
            {?
                use crate::pkg::ebuild::Restrict::*;
                let r = match attr {
                    "raw_subslot" => RawSubslot(None),
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
            = quiet!{" "*} op:$("==" / "!=" / "=~" / "!~") quiet!{" "*} { op }

        rule number_ops() -> &'input str
            = quiet!{" "*} op:$((['<' | '>'] "="?) / "==") quiet!{" "*} { op }

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
                    / "raw_subslot"
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
                    "raw_subslot" => RawSubslot(Some(r)),
                    "long_description" => LongDescription(Some(r)),
                    _ => return Err("unknown package attribute"),
                };
                Ok(ebuild_r.into())
            }

        rule maintainers() -> Restrict
            = "maintainers" r:(maintainers_ops() / maintainers_count())
            { r.into() }

        rule maintainer_attr_optional() -> MaintainerRestrict
            = attr:$(("name" / "description" / "type" / "proxied"))
                    " is " ("None" / "none") {?
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

        rule maintainers_ops() -> SliceMaintainers
            = quiet!{" "+} op:$(("contains" / "first" / "last")) quiet!{" "+}
                    r:(maintainer_attr_optional() / maintainer_restrict())
            {?
                use crate::metadata::ebuild::SliceMaintainers::*;
                let r = match op {
                    "contains" => Contains(r),
                    "first" => First(r),
                    "last" => Last(r),
                    _ => return Err("unknown maintainers operation"),
                };
                Ok(r)
            }

        rule maintainers_count() -> SliceMaintainers
            = quiet!{" "+} op:number_ops() count:$(['0'..='9']+) {?
                use crate::metadata::ebuild::SliceMaintainers::Count;
                let (cmps, size) = len_restrict(op, count)?;
                Ok(Count(cmps, size))
            }

        rule expr() -> Restrict
            = quiet!{" "*} invert:quiet!{"!"}?
                    r:(attr_optional()
                       / pkg_restrict()
                       / attr_str_restrict()
                       / maintainers()
                    ) quiet!{" "*} {
                match invert {
                    Some(_) => Restrict::not(r),
                    None => r,
                }
            }

        rule and() -> Restrict
            = quiet!{"("} exprs:query() ++ "&&" quiet!{")"} {
                Restrict::and(exprs)
            }

        rule or() -> Restrict
            = quiet!{"("} exprs:query() ++ "||" quiet!{")"} {
                Restrict::or(exprs)
            }

        rule xor() -> Restrict
            = quiet!{"("} exprs:query() ++ "^^" quiet!{")"} {
                Restrict::xor(exprs)
            }

        pub(super) rule query() -> Restrict
            = r:(expr() / and() / or() / xor()) { r }
    }
}

/// Convert a package query string into a Restriction.
pub fn pkg(s: &str) -> crate::Result<Restrict> {
    restrict::query(s).map_err(|e| peg_error(format!("invalid package query: {s:?}"), s, e))
}
