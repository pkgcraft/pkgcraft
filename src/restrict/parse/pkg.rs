use std::cmp::Ordering;

use regex::Regex;

use crate::metadata::ebuild::{MaintainerRestrict, SliceMaintainers};
use crate::peg::peg_error;
use crate::pkg::ebuild::Restrict::{self as PkgRestrict, *};
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
                let restrict_fn = match attr {
                    "eapi" => Eapi,
                    "repo" => Repo,
                    _ => return Err("unknown package attribute"),
                };

                let r = str_restrict(op, s)?;
                Ok(restrict_fn(r).into())
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
                let restrict_fn = match attr {
                    "ebuild" => Ebuild,
                    "category" => Category,
                    "description" => Description,
                    "slot" => Slot,
                    "subslot" => Subslot,
                    "raw_subslot" => |r: Str| -> PkgRestrict { RawSubslot(Some(r)) },
                    "long_description" => |r: Str| -> PkgRestrict { LongDescription(Some(r)) },
                    _ => return Err("unknown package attribute"),
                };

                let r = str_restrict(op, s)?;
                Ok(restrict_fn(r).into())
            }

        rule maintainers() -> Restrict
            = "maintainers" r:(maintainers_str_ops() / maintainers_count()) { r.into() }

        rule maintainers_attr_optional() -> MaintainerRestrict
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

        rule maintainers_str_restrict() -> MaintainerRestrict
            = attr:$(("email" / "name" / "description" / "type" / "proxied"))
                op:string_ops() s:quoted_string()
            {?
                use crate::metadata::ebuild::MaintainerRestrict::*;
                let restrict_fn = match attr {
                    "email" => Email,
                    "name" => |r: Str| -> MaintainerRestrict { Name(Some(r)) },
                    "description" => |r: Str| -> MaintainerRestrict { Description(Some(r)) },
                    "type" => |r: Str| -> MaintainerRestrict { Type(Some(r)) },
                    "proxied" => |r: Str| -> MaintainerRestrict { Proxied(Some(r)) },
                    _ => return Err("unknown maintainer attribute"),
                };

                let r = str_restrict(op, s)?;
                Ok(restrict_fn(r))
            }

        rule maintainers_str_ops() -> SliceMaintainers
            = quiet!{" "+} op:$(("contains" / "first" / "last")) quiet!{" "+}
                    r:(maintainers_attr_optional()
                       / maintainers_str_restrict()
                    )
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
                let cmps = match op {
                    "<" => vec![Ordering::Less],
                    "<=" => vec![Ordering::Less, Ordering::Equal],
                    "==" => vec![Ordering::Equal],
                    ">=" => vec![Ordering::Greater, Ordering::Equal],
                    ">" => vec![Ordering::Greater],
                    _ => return Err("unknown count operator"),
                };

                let size: usize = match count.parse() {
                    Ok(v) => v,
                    Err(_) => return Err("invalid count size"),
                };

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
