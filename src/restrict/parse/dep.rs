use regex::{escape, Regex};

use crate::atom::{self, version::ParsedVersion};
use crate::peg::peg_error;
use crate::restrict::{Restrict, Str};

// Convert globbed string to regex, escaping all meta characters except '*'.
fn str_to_regex_restrict(s: &str) -> Str {
    let re_s = escape(s).replace("\\*", ".*");
    let re = Regex::new(&format!(r"^{re_s}$")).unwrap();
    Str::Regex(re)
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

    rule version_suffix() -> (&'input str, Option<&'input str>)
        = suffix:$("alpha" / "beta" / "pre" / "rc" / "p") ver:$(['0'..='9']+)? {?
            Ok((suffix, ver))
        }

    rule version() -> ParsedVersion<'input>
        = start:position!() numbers:$(['0'..='9']+) ++ "." letter:['a'..='z']?
                suffixes:("_" s:version_suffix() ++ "_" {s})?
                end_base:position!() revision:revision()? end:position!() {
            ParsedVersion {
                start,
                start_base: start,
                end_base,
                end,
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

    rule cp_restricts() -> Vec<Restrict>
        = cat:category() pkg:(quiet!{"/"} s:package() { s }) {
            let mut restricts = vec![];
            match cat.matches('*').count() {
                0 => restricts.push(Restrict::Atom(atom::Restrict::category(cat))),
                _ => {
                    let r = str_to_regex_restrict(cat);
                    restricts.push(Restrict::Atom(atom::Restrict::Category(r)))
                }
            }

            match pkg.matches('*').count() {
                0 => restricts.push(Restrict::Atom(atom::Restrict::package(pkg))),
                1 if pkg == "*" && restricts.is_empty() => (),
                _ => {
                    let r = str_to_regex_restrict(pkg);
                    restricts.push(Restrict::Atom(atom::Restrict::Package(r)))
                }
            }

            restricts
        } / s:package() {
            match s.matches('*').count() {
                0 => vec![Restrict::Atom(atom::Restrict::package(s))],
                1 if s == "*" => vec![],
                _ => {
                    let r = str_to_regex_restrict(s);
                    vec![Restrict::Atom(atom::Restrict::Package(r))]
                }
            }
        }

    rule pkg_restricts() -> (Vec<Restrict>, Option<ParsedVersion<'input>>)
        = restricts:cp_restricts() { (restricts, None) }
        / op:$(("<" "="?) / "=" / "~" / (">" "="?))
                restricts:cp_restricts() "-" ver:version() glob:$("*")?
        {?
            Ok((restricts, Some(ver.with_op(op, glob)?)))
        }

    rule slot_glob() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '*']
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '.' | '-' | '*']*})
        { s }

    rule slot_restrict() -> Restrict
        = s:slot_glob() {
            match s.matches('*').count() {
                0 => Restrict::Atom(atom::Restrict::slot(Some(s))),
                _ => {
                    let r = str_to_regex_restrict(s);
                    Restrict::Atom(atom::Restrict::Slot(Some(r)))
                }
            }
        }

    rule subslot_restrict() -> Restrict
        = "/" s:slot_glob() {
            match s.matches('*').count() {
                0 => Restrict::Atom(atom::Restrict::subslot(Some(s))),
                _ => {
                    let r = str_to_regex_restrict(s);
                    Restrict::Atom(atom::Restrict::Subslot(Some(r)))
                }
            }
        }

    rule slot_restricts() -> Vec<Restrict>
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

    rule repo_restrict() -> Restrict
        = "::" s:repo_glob() {
            match s.matches('*').count() {
                0 => Restrict::Atom(atom::Restrict::repo(Some(s))),
                _ => {
                    let r = str_to_regex_restrict(s);
                    Restrict::Atom(atom::Restrict::Repo(Some(r)))
                }
            }
        }

    pub(super) rule dep() -> (Vec<Restrict>, Option<ParsedVersion<'input>>)
        = pkg_r:pkg_restricts() slot_r:slot_restricts()? repo_r:repo_restrict()? {
            let (mut restricts, ver) = pkg_r;
            if let Some(r) = slot_r {
                restricts.extend(r);
            }
            if let Some(r) = repo_r {
                restricts.push(r);
            }
            (restricts, ver)
        }
});

/// Convert a globbed dep string into a Restriction.
pub fn dep(s: &str) -> crate::Result<Restrict> {
    let (mut restricts, ver) =
        restrict::dep(s).map_err(|e| peg_error(format!("invalid dep restriction: {s:?}"), s, e))?;

    if let Some(v) = ver {
        let v = v.to_owned(s)?;
        restricts.push(Restrict::Atom(atom::Restrict::Version(Some(v))));
    }

    match restricts.is_empty() {
        true => Ok(Restrict::True),
        false => Ok(Restrict::and(restricts)),
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::atom::Atom;
    use crate::restrict::Restriction;

    use super::*;

    #[test]
    fn test_filtering() {
        let atom_strs = vec![
            "cat/pkg",
            "cat-abc/pkg2",
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
        let atoms: Vec<_> = atom_strs
            .iter()
            .map(|s| Atom::from_str(s).unwrap())
            .collect();

        let filter = |r: Restrict, atoms: &[Atom]| -> Vec<String> {
            atoms
                .into_iter()
                .filter(|&a| r.matches(a))
                .map(|a| a.to_string())
                .collect()
        };

        // category and package
        for (s, expected) in [
            ("*", &atom_strs[..]),
            ("*/*", &atom_strs[..]),
            ("*cat*/*", &atom_strs[..]),
            ("c*t*/*", &atom_strs[..]),
            ("c*ot/*", &[]),
            ("cat", &[]),
            ("cat-*/*", &["cat-abc/pkg2"]),
            ("*-abc/*", &["cat-abc/pkg2"]),
            ("*-abc/pkg*", &["cat-abc/pkg2"]),
            ("pkg2", &["cat-abc/pkg2"]),
            ("*2", &["cat-abc/pkg2"]),
            ("pkg*", &atom_strs[..]),
        ] {
            let r = dep(s).unwrap();
            assert_eq!(filter(r, &atoms), expected, "{s:?} failed");
        }

        // package and version
        for (s, expected) in [
            (">=pkg-1", vec!["=cat/pkg-1", ">=cat/pkg-2", "<cat/pkg-3"]),
            ("=pkg-2", vec![">=cat/pkg-2"]),
            ("=*-2", vec![">=cat/pkg-2"]),
            ("<pkg-3", vec!["=cat/pkg-0-r0:0/0.+", "=cat/pkg-1", ">=cat/pkg-2"]),
        ] {
            let r = dep(s).unwrap();
            assert_eq!(filter(r, &atoms), expected, "{s:?} failed");
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
            assert_eq!(filter(r, &atoms), expected, "{s:?} failed");
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
            assert_eq!(filter(r, &atoms), expected, "{s:?} failed");
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
            assert_eq!(filter(r, &atoms), expected, "{s:?} failed");
        }
    }
}
