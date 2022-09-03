use regex::Regex;

use crate::peg::peg_error;
use crate::pkg::ebuild::Restrict::*;
use crate::restrict::{Restrict, Str};

peg::parser! {
    grammar restrict() for str {
        rule quoted_string() -> &'input str
            = "\"" s:$([^ '\"']+) "\"" { s }
            / "\'" s:$([^ '\'']+) "\'" { s }

        rule string_ops() -> &'input str
            = " "* op:$("==" / "!=" / "=~" / "!~") " "* { op }

        rule category() -> Restrict
            = ("category" / "CATEGORY") op:string_ops() s:quoted_string()
            {?
                let r: Restrict = match op {
                    "==" => Category(Str::matches(s)).into(),
                    "!=" => Restrict::not(Category(Str::matches(s))),
                    "=~" => match Regex::new(s) {
                        Ok(r) => Category(Str::Regex(r)).into(),
                        Err(_) => return Err("invalid regex"),
                    },
                    "!~" => match Regex::new(s) {
                        Ok(r) => Restrict::not(Category(Str::Regex(r))),
                        Err(_) => return Err("invalid regex"),
                    },
                    _ => return Err("invalid string operator"),
                };
                Ok(r)
            }

        rule description() -> Restrict
            = ("description" / "DESCRIPTION") op:string_ops() s:quoted_string()
            {?
                let r: Restrict = match op {
                    "==" => Description(Str::matches(s)).into(),
                    "!=" => Restrict::not(Description(Str::matches(s))),
                    "=~" => match Regex::new(s) {
                        Ok(r) => Description(Str::Regex(r)).into(),
                        Err(_) => return Err("invalid regex"),
                    },
                    "!~" => match Regex::new(s) {
                        Ok(r) => Restrict::not(Description(Str::Regex(r))),
                        Err(_) => return Err("invalid regex"),
                    },
                    _ => return Err("invalid string operator"),
                };
                Ok(r)
            }

        pub rule expr() -> Restrict
            = " "* invert:"!"? r:(category() / description()) " "* {
                let mut restrict = r;
                if invert.is_some() {
                    restrict = Restrict::not(restrict);
                }
                restrict
            }

        pub rule and() -> Restrict
            = "(" exprs:query() ++ "&&" ")" {
                Restrict::and(exprs)
            }

        pub rule or() -> Restrict
            = "(" exprs:query() ++ "||" ")" {
                Restrict::or(exprs)
            }

        pub rule xor() -> Restrict
            = "(" exprs:query() ++ "^^" ")" {
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
