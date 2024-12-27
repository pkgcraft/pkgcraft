use crate::dep::version::{Number, Operator, Revision, Suffix, SuffixKind, Version, WithOp};
use crate::dep::{Blocker, UseDep, UseDepKind};
use crate::error::peg_error;
use crate::restrict::dep::Restrict as DepRestrict;
use crate::restrict::str::Restrict as StrRestrict;
use crate::restrict::Restrict as BaseRestrict;

// Convert globbed string to regex restriction, escaping all meta characters except '*'.
fn str_to_regex_restrict(s: &str) -> Result<StrRestrict, &'static str> {
    let re_s = regex::escape(s).replace("\\*", ".*");
    StrRestrict::regex(format!(r"^{re_s}$")).map_err(|_| "invalid regex")
}

peg::parser!(grammar restrict() for str {
    rule _ = quiet!{[^ ' ' | '\n' | '\t']+}
    rule __ = quiet!{[' ' | '\n' | '\t']+}

    rule category() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '*']
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '.' | '-' | '*']*})
        { s }

    rule package() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '*']
            (['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '*'] /
                ("-" !(version() ("-" version())? (__ / "*" / ":" / "[" / ![_]))))*})
        { s }

    rule number() -> Number
        = s:$(['0'..='9']+) {?
            let value = s.parse().map_err(|_| "integer overflow")?;
            Ok(Number { raw: s.to_string(), value })
        }

    rule suffix() -> SuffixKind
        = "alpha" { SuffixKind::Alpha }
        / "beta" { SuffixKind::Beta }
        / "pre" { SuffixKind::Pre }
        / "rc" { SuffixKind::Rc }
        / "p" { SuffixKind::P }

    rule version_suffix() -> Suffix
        = "_" kind:suffix() version:number()? { Suffix { kind, version } }

    pub(super) rule version() -> Version
        = numbers:number() ++ "." letter:['a'..='z']?
                suffixes:version_suffix()* revision:revision()? {
            Version {
                op: None,
                numbers,
                letter,
                suffixes,
                revision: revision.unwrap_or_default(),
            }
        }

    rule revision() -> Revision
        = "-r" rev:number() { Revision(rev) }

    rule cp_restricts() -> Vec<DepRestrict>
        = cat:category() pkg:(quiet!{"/"} s:package() { s }) {?
            let mut restricts = vec![];

            match cat.matches('*').count() {
                0 => restricts.push(DepRestrict::category(cat)),
                _ => {
                    if !cat.trim_start_matches('*').is_empty() {
                        let r = str_to_regex_restrict(cat)?;
                        restricts.push(DepRestrict::Category(r));
                    }
                }
            }

            match pkg.matches('*').count() {
                0 => restricts.push(DepRestrict::package(pkg)),
                _ => {
                    if !pkg.trim_start_matches('*').is_empty() {
                        let r = str_to_regex_restrict(pkg)?;
                        restricts.push(DepRestrict::Package(r));
                    }
                }
            }

            Ok(restricts)
        } / pkg:package() {?
            let mut restricts = vec![];

            match pkg.matches('*').count() {
                0 => restricts.push(DepRestrict::package(pkg)),
                _ => {
                    if !pkg.trim_start_matches('*').is_empty() {
                        let r = str_to_regex_restrict(pkg)?;
                        restricts.push(DepRestrict::Package(r));
                    }
                }
            }

            Ok(restricts)
        }

    rule pkg_restricts() -> (Vec<DepRestrict>, Option<Version>)
        = r:cp_restricts() ver:("-" v:version() { v })? { (r, ver) }
        / "<=" r:cp_restricts() "-" v:version() {? Ok((r, Some(v.with_op(Operator::LessOrEqual)?))) }
        / "<" r:cp_restricts() "-" v:version() {? Ok((r, Some(v.with_op(Operator::Less)?))) }
        / ">=" r:cp_restricts() "-" v:version() {? Ok((r, Some(v.with_op(Operator::GreaterOrEqual)?))) }
        / ">" r:cp_restricts() "-" v:version() {? Ok((r, Some(v.with_op(Operator::Greater)?))) }
        / "=" r:cp_restricts() "-" v:version() glob:$("*")? {?
            if glob.is_none() {
                Ok((r, Some(v.with_op(Operator::Equal)?)))
            } else {
                Ok((r, Some(v.with_op(Operator::EqualGlob)?)))
            }
        } / "~" r:cp_restricts() "-" v:version() {?
            Ok((r, Some(v.with_op(Operator::Approximate)?)))
        }

    rule slot_glob() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '*']
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '.' | '-' | '*']*})
        { s }

    rule slot_restrict() -> DepRestrict
        = s:slot_glob() {?
            match s.matches('*').count() {
                0 => Ok(DepRestrict::slot(Some(s))),
                _ => {
                    let r = str_to_regex_restrict(s)?;
                    Ok(DepRestrict::Slot(Some(r)))
                }
            }
        }

    rule subslot_restrict() -> DepRestrict
        = "/" s:slot_glob() {?
            match s.matches('*').count() {
                0 => Ok(DepRestrict::subslot(Some(s))),
                _ => {
                    let r = str_to_regex_restrict(s)?;
                    Ok(DepRestrict::Subslot(Some(r)))
                }
            }
        }

    rule slot_restricts() -> Vec<DepRestrict>
        = ":" slot_r:slot_restrict() subslot_r:subslot_restrict()? {
            let mut restricts = vec![slot_r];
            if let Some(r) = subslot_r {
                restricts.push(r);
            }
            restricts
        }

    rule use_flag() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9']
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '@' | '-']*
        } / expected!("USE flag name")
        ) { s }

    rule use_dep_default() -> bool
        = "(+)" { true }
        / "(-)" { false }

    rule use_dep() -> UseDep
        = disabled:"!"? flag:use_flag() default:use_dep_default()? kind:$(['=' | '?']) {
            UseDep {
                flag: flag.to_string(),
                kind: match kind {
                    "=" => UseDepKind::Equal,
                    "?" => UseDepKind::Conditional,
                    _ => unreachable!("invalid use dep kind"),
                },
                enabled: disabled.is_none(),
                default,
            }
        } / disabled:"-"? flag:use_flag() default:use_dep_default()? {
            UseDep {
                flag: flag.to_string(),
                kind: UseDepKind::Enabled,
                enabled: disabled.is_none(),
                default,
            }
        } / expected!("use dep")

    rule use_restricts() -> DepRestrict
        = "[" u:use_dep() ++ "," "]" { DepRestrict::UseDeps(Some(u.into_iter().collect())) }

    rule repo_restrict() -> DepRestrict
        = "::" s:$(_+) {?
            match s.matches('*').count() {
                0 => Ok(DepRestrict::repo(Some(s))),
                _ => {
                    let r = str_to_regex_restrict(s)?;
                    Ok(DepRestrict::Repo(Some(r)))
                }
            }
        }

    rule blocker_restrict() -> DepRestrict
        = blocker:("!"*<1,2>) {?
            match blocker.len() {
                1 => Ok(DepRestrict::Blocker(Some(Blocker::Weak))),
                2 => Ok(DepRestrict::Blocker(Some(Blocker::Strong))),
                _ => Err("invalid blocker"),
            }
        }

    pub(super) rule dep() -> (Vec<DepRestrict>, Option<Version>)
        = blocker_r:blocker_restrict()? pkg_r:pkg_restricts()
            slot_r:slot_restricts()? use_r:use_restricts()? repo_r:repo_restrict()?
        {
            let (mut restricts, ver) = pkg_r;

            if let Some(r) = blocker_r {
                restricts.push(r);
            }

            if let Some(r) = slot_r {
                restricts.extend(r);
            }

            if let Some(r) = use_r {
                restricts.push(r);
            }

            if let Some(r) = repo_r {
                restricts.push(r);
            }

            (restricts, ver)
        }
});

/// Convert a globbed dep string into a Vector of dep restrictions.
pub(crate) fn restricts(s: &str) -> crate::Result<Vec<DepRestrict>> {
    let (mut restricts, ver) =
        restrict::dep(s).map_err(|e| peg_error("invalid dep restriction", s, e))?;

    if let Some(v) = ver {
        restricts.push(DepRestrict::Version(Some(v)));
    }

    Ok(restricts)
}

/// Convert a globbed dep string into a restriction.
pub fn dep<S: AsRef<str>>(s: S) -> crate::Result<BaseRestrict> {
    let restricts = restricts(s.as_ref())?;
    if restricts.is_empty() {
        Ok(BaseRestrict::True)
    } else {
        Ok(BaseRestrict::and(restricts))
    }
}

#[cfg(test)]
mod tests {
    use crate::dep::Dep;
    use crate::restrict::Restriction;

    use super::*;

    #[test]
    fn filtering() {
        let dep_strs = vec![
            "cat/pkg",
            "cat-abc/pkg2",
            // blocked
            "!cat/pkg",
            "!!cat/pkg",
            // slotted
            "cat/pkg:0",
            "cat/pkg:2.1",
            // subslotted
            "cat/pkg:2/1.1",
            // versioned
            "=cat/pkg-0-r0:0/0.+",
            "=cat/pkg-1",
            ">=cat/pkg-2",
            "<cat/pkg-3",
            // use deps
            "cat/pkg[u]",
            // repo
            "cat/pkg::repo",
            "cat/pkg::repo-ed",
        ];
        let deps: Vec<_> = dep_strs.iter().map(|s| Dep::try_new(s).unwrap()).collect();

        let filter = |r: BaseRestrict, deps: &[Dep]| -> Vec<String> {
            deps.iter()
                .filter(|&a| r.matches(a))
                .map(|a| a.to_string())
                .collect()
        };

        // category and package
        for (s, expected) in [
            ("*", &dep_strs[..]),
            ("*/*", &dep_strs[..]),
            ("*cat*/*", &dep_strs[..]),
            ("c*t*/*", &dep_strs[..]),
            ("c*ot/*", &[]),
            ("cat", &[]),
            ("cat-*/*", &["cat-abc/pkg2"]),
            ("*-abc/*", &["cat-abc/pkg2"]),
            ("*-abc/pkg*", &["cat-abc/pkg2"]),
            ("pkg2", &["cat-abc/pkg2"]),
            ("*2", &["cat-abc/pkg2"]),
            ("pkg*", &dep_strs[..]),
        ] {
            let r = dep(s).unwrap();
            assert_eq!(filter(r, &deps), expected, "{s:?} failed");
        }

        // package and version
        for (s, expected) in [
            (">=pkg-1", vec!["=cat/pkg-1", ">=cat/pkg-2", "<cat/pkg-3"]),
            ("=pkg-2", vec![">=cat/pkg-2", "<cat/pkg-3"]),
            ("=*-2", vec![">=cat/pkg-2", "<cat/pkg-3"]),
            ("<pkg-3", vec!["=cat/pkg-0-r0:0/0.+", "=cat/pkg-1", ">=cat/pkg-2", "<cat/pkg-3"]),
        ] {
            let r = dep(s).unwrap();
            assert_eq!(filter(r, &deps), expected, "{s:?} failed");
        }

        // blocker
        for (s, expected) in [("!*", vec!["!cat/pkg"]), ("!!*", vec!["!!cat/pkg"])] {
            let r = dep(s).unwrap();
            assert_eq!(filter(r, &deps), expected, "{s:?} failed");
        }

        // slot
        for (s, expected) in [
            ("*:*", vec!["cat/pkg:0", "cat/pkg:2.1", "cat/pkg:2/1.1", "=cat/pkg-0-r0:0/0.+"]),
            ("*:0", vec!["cat/pkg:0", "=cat/pkg-0-r0:0/0.+"]),
            ("*:2", vec!["cat/pkg:2/1.1"]),
            ("*:2*", vec!["cat/pkg:2.1", "cat/pkg:2/1.1"]),
            ("pkg*:2*", vec!["cat/pkg:2.1", "cat/pkg:2/1.1"]),
            ("<pkg-1:*", vec!["=cat/pkg-0-r0:0/0.+"]),
        ] {
            let r = dep(s).unwrap();
            assert_eq!(filter(r, &deps), expected, "{s:?} failed");
        }

        // subslot
        for (s, expected) in [
            ("*:*/*", vec!["cat/pkg:2/1.1", "=cat/pkg-0-r0:0/0.+"]),
            ("*:2/*", vec!["cat/pkg:2/1.1"]),
            ("*:2/1", vec![]),
            ("*:2/1*", vec!["cat/pkg:2/1.1"]),
            ("*:*/*.+", vec!["=cat/pkg-0-r0:0/0.+"]),
        ] {
            let r = dep(s).unwrap();
            assert_eq!(filter(r, &deps), expected, "{s:?} failed");
        }

        // use deps
        for (s, expected) in [
            ("cat/pkg[u]", vec!["cat/pkg[u]"]),
            ("cat/pkg[u=]", vec![]),
            ("cat/pkg[!u=]", vec![]),
            ("cat/pkg[u?]", vec![]),
            ("cat/pkg[!u?]", vec![]),
            ("cat/pkg[-u]", vec![]),
        ] {
            let r = dep(s).unwrap();
            assert_eq!(filter(r, &deps), expected, "{s:?} failed");
        }

        // repo
        for (s, expected) in [
            ("*::*", vec!["cat/pkg::repo", "cat/pkg::repo-ed"]),
            ("*::r*", vec!["cat/pkg::repo", "cat/pkg::repo-ed"]),
            ("*::re*po", vec!["cat/pkg::repo"]),
            ("*::repo*", vec!["cat/pkg::repo", "cat/pkg::repo-ed"]),
            ("*::repo", vec!["cat/pkg::repo"]),
        ] {
            let r = dep(s).unwrap();
            assert_eq!(filter(r, &deps), expected, "{s:?} failed");
        }
    }
}
