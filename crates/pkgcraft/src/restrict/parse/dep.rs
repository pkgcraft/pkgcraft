use crate::dep::version::{ParsedVersion, Suffix};
use crate::dep::Blocker;
use crate::peg::peg_error;
use crate::restrict::dep::Restrict as DepRestrict;
use crate::restrict::str::Restrict as StrRestrict;
use crate::restrict::Restrict as BaseRestrict;

// Convert globbed string to regex restriction, escaping all meta characters except '*'.
fn str_to_regex_restrict(s: &str) -> Result<StrRestrict, &'static str> {
    let re_s = regex::escape(s).replace("\\*", ".*");
    StrRestrict::regex(format!(r"^{re_s}$")).map_err(|_| "invalid regex")
}

peg::parser!(grammar restrict() for str {
    rule category() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '*']
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '.' | '-' | '*']*})
        { s }

    rule package() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '*']
            (['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '*'] / ("-" !version()))*})
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

    rule version() -> ParsedVersion<'input>
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

    rule revision() -> &'input str
        = "-r" s:$(quiet!{['0'..='9']+} / expected!("revision"))
        { s }

    rule cp_restricts() -> Vec<DepRestrict>
        = cat:category() pkg:(quiet!{"/"} s:package() { s }) {?
            let mut restricts = vec![];
            match cat.matches('*').count() {
                0 => restricts.push(DepRestrict::category(cat)),
                _ => {
                    let r = str_to_regex_restrict(cat)?;
                    restricts.push(DepRestrict::Category(r))
                }
            }

            match pkg.matches('*').count() {
                0 => restricts.push(DepRestrict::package(pkg)),
                1 if pkg == "*" && restricts.is_empty() => (),
                _ => {
                    let r = str_to_regex_restrict(pkg)?;
                    restricts.push(DepRestrict::Package(r))
                }
            }

            Ok(restricts)
        } / s:package() {?
            match s.matches('*').count() {
                0 => Ok(vec![DepRestrict::package(s)]),
                1 if s == "*" => Ok(vec![]),
                _ => {
                    let r = str_to_regex_restrict(s)?;
                    Ok(vec![DepRestrict::Package(r)])
                }
            }
        }

    rule pkg_restricts() -> (Vec<DepRestrict>, Option<ParsedVersion<'input>>)
        = restricts:cp_restricts() { (restricts, None) }
            / op:$(("<" "="?) / "=" / "~" / (">" "="?))
            restricts:cp_restricts() "-" ver:version() glob:$("*")?
        {? Ok((restricts, Some(ver.with_op(op, glob)?))) }

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

    rule repo_glob() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '*']
            (['a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '*'] / ("-" !version()))*})
        { s }

    rule useflag() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9']
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '@' | '-']*
        } / expected!("useflag name")
        ) { s }

    rule use_dep() -> &'input str
        = s:$(quiet!{
            (useflag() use_dep_default()? ['=' | '?']?) /
            ("-" useflag() use_dep_default()?) /
            ("!" useflag() use_dep_default()? ['=' | '?'])
        } / expected!("use dep")
        ) { s }

    rule use_dep_default() -> &'input str
        = s:$("(+)" / "(-)") { s }

    rule use_restricts() -> DepRestrict
        = "[" use_deps:use_dep() ++ "," "]"
        { DepRestrict::use_deps(Some(use_deps)) }

    rule repo_restrict() -> DepRestrict
        = "::" s:repo_glob() {?
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

    pub(super) rule dep() -> (Vec<DepRestrict>, Option<ParsedVersion<'input>>)
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
        restrict::dep(s).map_err(|e| peg_error(format!("invalid dep restriction: {s:?}"), s, e))?;

    if let Some(v) = ver {
        let v = v.into_owned(s)?;
        restricts.push(DepRestrict::Version(Some(v)));
    }

    Ok(restricts)
}

/// Convert a globbed dep string into a restriction.
pub fn dep(s: &str) -> crate::Result<BaseRestrict> {
    let restricts = restricts(s)?;
    if restricts.is_empty() {
        Ok(BaseRestrict::True)
    } else {
        Ok(BaseRestrict::and(restricts))
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::dep::Dep;
    use crate::restrict::Restriction;

    use super::*;

    #[test]
    fn test_filtering() {
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
            // repo
            "cat/pkg::repo",
            "cat/pkg::repo-ed",
        ];
        let deps: Vec<_> = dep_strs.iter().map(|s| Dep::from_str(s).unwrap()).collect();

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
