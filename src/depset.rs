use std::collections::VecDeque;

use crate::atom::{Atom, Restrict as AtomRestrict};
use crate::eapi::{Eapi, Feature};
use crate::macros::vec_str as vs;
use crate::restrict::{self, Restriction, Str};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Uri {
    uri: String,
    rename: Option<String>,
}

impl Uri {
    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub fn rename(&self) -> Option<&str> {
        self.rename.as_deref()
    }
}

impl AsRef<str> for Uri {
    fn as_ref(&self) -> &str {
        &self.uri
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DepSet<T> {
    Values(Vec<T>),
    // logic conditionals
    AllOf(Vec<Box<DepSet<T>>>),
    AnyOf(Vec<Box<DepSet<T>>>),
    ExactlyOneOf(Vec<Box<DepSet<T>>>), // REQUIRED_USE only
    AtMostOneOf(Vec<Box<DepSet<T>>>),  // REQUIRED_USE only
    UseEnabled(String, Vec<Box<DepSet<T>>>),
    UseDisabled(String, Vec<Box<DepSet<T>>>),
}

impl<T> DepSet<T> {
    pub fn flatten(&self) -> DepSetFlatten<T> {
        DepSetFlatten {
            depsets: VecDeque::from([self]),
            buffer: VecDeque::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Restrict<T> {
    Matches(T),
}

impl Restriction<&DepSet<Atom>> for Restrict<AtomRestrict> {
    fn matches(&self, val: &DepSet<Atom>) -> bool {
        match self {
            Self::Matches(r) => val.flatten().any(|v| r.matches(v)),
        }
    }
}

impl Restriction<&DepSet<String>> for Restrict<Str> {
    fn matches(&self, val: &DepSet<String>) -> bool {
        match self {
            Self::Matches(r) => val.flatten().any(|v| r.matches(v)),
        }
    }
}

impl Restriction<&DepSet<Uri>> for Restrict<Str> {
    fn matches(&self, val: &DepSet<Uri>) -> bool {
        match self {
            Self::Matches(r) => val.flatten().any(|v| r.matches(v.as_ref())),
        }
    }
}

pub struct DepSetFlatten<'a, T> {
    depsets: VecDeque<&'a DepSet<T>>,
    buffer: VecDeque<&'a T>,
}

impl<'a, T> Iterator for DepSetFlatten<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        while self.buffer.front().is_none() && !self.depsets.is_empty() {
            if let Some(d) = self.depsets.pop_front() {
                match d {
                    DepSet::Values(vals) => self.buffer.extend(vals),
                    DepSet::AllOf(vals) => self.depsets.extend(vals.iter().map(AsRef::as_ref)),
                    DepSet::AnyOf(vals) => self.depsets.extend(vals.iter().map(AsRef::as_ref)),
                    DepSet::ExactlyOneOf(vals) => {
                        self.depsets.extend(vals.iter().map(AsRef::as_ref))
                    }
                    DepSet::AtMostOneOf(vals) => {
                        self.depsets.extend(vals.iter().map(AsRef::as_ref))
                    }
                    DepSet::UseEnabled(_, vals) => {
                        self.depsets.extend(vals.iter().map(AsRef::as_ref))
                    }
                    DepSet::UseDisabled(_, vals) => {
                        self.depsets.extend(vals.iter().map(AsRef::as_ref))
                    }
                }
            }
        }
        self.buffer.pop_front()
    }
}

impl Restriction<&DepSet<Atom>> for restrict::Restrict {
    fn matches(&self, val: &DepSet<Atom>) -> bool {
        restrict::restrict_match! {
            self, val,
            Self::Atom(r) => val.flatten().any(|v| r.matches(v))
        }
    }
}

peg::parser!(grammar depset() for str {
    rule _ = [' ']

    // licenses must not begin with a hyphen, dot, or plus sign.
    rule license_name() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '.' | '-']*
        } / expected!("license name")
        ) { s }

    rule useflag() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9']
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '@' | '-']*
        } / expected!("useflag name")
        ) { s }

    rule dep_char() -> &'input str
        = s:$(quiet!{
            ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '@' | '-' | '/']
        }) { s }

    rule dep(eapi: &'static Eapi) -> Atom
        = s:$((dep_char()+ &(" " / ![_]))) {?
            let atom = match Atom::new(s, eapi) {
                Ok(x) => x,
                Err(e) => return Err("failed parsing atom"),
            };
            Ok(atom)
        }

    rule deps(eapi: &'static Eapi) -> DepSet<Atom>
        = vals:dep(eapi) ++ " " { DepSet::Values(vals) }

    rule licenses() -> DepSet<String>
        = vals:license_name() ++ " " { DepSet::Values(vs!(vals)) }

    rule useflags() -> DepSet<String>
        = vals:useflag() ++ " " { DepSet::Values(vs!(vals)) }

    rule uri() -> &'input str
        = s:$(quiet!{!['(' | ')'] [^' ']+}) { s }

    rule uris(eapi: &'static Eapi) -> DepSet<Uri>
        = uris:uri() ++ " " {
            let mut uri_objs: Vec<Uri> = Vec::new();

            if eapi.has(Feature::SrcUriRenames) {
                let mut uris = uris.iter().peekable();
                while let Some(x) = uris.next() {
                    let rename = match uris.peek() {
                        Some(&&"->") => {
                            uris.next();
                            uris.next().map(|s| s.to_string())
                        },
                        _ => None,
                    };
                    uri_objs.push(Uri { uri: x.to_string(), rename });
                }
            } else {
                for x in uris {
                    uri_objs.push(Uri { uri: x.to_string(), rename: None });
                }
            }

            DepSet::Values(uri_objs)
        }

    rule parens<T>(expr: rule<T>) -> Vec<Box<T>>
        = "(" _ v:expr() ++ " " _ ")"
        { v.into_iter().map(Box::new).collect() }

    rule all_of<T>(expr: rule<DepSet<T>>) -> DepSet<T>
        = vals:parens(<expr()>) { DepSet::AllOf(vals) }

    rule any_of<T>(expr: rule<DepSet<T>>) -> DepSet<T>
        = "||" _ vals:parens(<expr()>) { DepSet::AnyOf(vals) }

    rule use_cond<T>(expr: rule<DepSet<T>>) -> DepSet<T>
        = negate:"!"? u:useflag() "?" _ vals:parens(<expr()>) {
            let f = match negate {
                None => DepSet::UseEnabled,
                Some(_) => DepSet::UseDisabled,
            };
            f(u.to_string(), vals)
        }

    rule exactly_one_of<T>(expr: rule<DepSet<T>>) -> DepSet<T>
        = "^^" _ vals:parens(<expr()>) { DepSet::ExactlyOneOf(vals) }

    rule at_most_one_of<T>(eapi: &'static Eapi, expr: rule<DepSet<T>>) -> DepSet<T>
        = "??" _ vals:parens(<expr()>) {?
            if !eapi.has(Feature::RequiredUseOneOf) {
                return Err("?? groups are supported in >= EAPI 5");
            }
            Ok(DepSet::AtMostOneOf(vals))
        }

    pub(super) rule license() -> DepSet<String>
        = use_cond(<license()>) / any_of(<license()>) / all_of(<license()>) / licenses()

    pub(super) rule src_uri(eapi: &'static Eapi) -> DepSet<Uri>
        = use_cond(<src_uri(eapi)>) / all_of(<src_uri(eapi)>) / uris(eapi)

    pub(super) rule required_use(eapi: &'static Eapi) -> DepSet<String>
        = use_cond(<required_use(eapi)>)
            / any_of(<required_use(eapi)>)
            / all_of(<required_use(eapi)>)
            / exactly_one_of(<required_use(eapi)>)
            / at_most_one_of(eapi, <required_use(eapi)>)
            / useflags()

    pub(super) rule pkgdep(eapi: &'static Eapi) -> DepSet<Atom>
        = use_cond(<pkgdep(eapi)>)
            / any_of(<pkgdep(eapi)>)
            / all_of(<pkgdep(eapi)>)
            / deps(eapi)
});

// provide public parsing functionality while converting error types
pub mod parse {
    use crate::peg::peg_error;

    use super::*;

    pub fn license(s: &str) -> crate::Result<Option<DepSet<String>>> {
        match s.is_empty() {
            true => Ok(None),
            false => depset::license(s)
                .map(Some)
                .map_err(|e| peg_error(format!("invalid LICENSE: {s:?}"), s, e)),
        }
    }

    pub fn src_uri(s: &str, eapi: &'static Eapi) -> crate::Result<Option<DepSet<Uri>>> {
        match s.is_empty() {
            true => Ok(None),
            false => depset::src_uri(s, eapi)
                .map(Some)
                .map_err(|e| peg_error(format!("invalid SRC_URI: {s:?}"), s, e)),
        }
    }

    pub fn required_use(s: &str, eapi: &'static Eapi) -> crate::Result<Option<DepSet<String>>> {
        match s.is_empty() {
            true => Ok(None),
            false => depset::required_use(s, eapi)
                .map(Some)
                .map_err(|e| peg_error(format!("invalid REQUIRED_USE: {s:?}"), s, e)),
        }
    }

    pub fn pkgdep(s: &str, eapi: &'static Eapi) -> crate::Result<Option<DepSet<Atom>>> {
        match s.is_empty() {
            true => Ok(None),
            false => depset::pkgdep(s, eapi)
                .map(Some)
                .map_err(|e| peg_error(format!("invalid dependency: {s:?}"), s, e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::eapi::{EAPIS, EAPI_LATEST};

    use super::*;

    #[test]
    fn test_license() {
        // invalid data
        for s in ["(", ")", "( )", "( l1)", "| ( l1 )", "foo ( l1 )", "!use ( l1 )"] {
            assert!(parse::license(&s).is_err(), "{s:?} didn't fail");
        }

        // good data
        for (s, expected) in [
            ("", None),
            ("l1", Some(DepSet::Values(vs!(["l1"])))),
            ("l1 l2", Some(DepSet::Values(vs!(["l1", "l2"])))),
            ("( l1 )", Some(DepSet::AllOf(vec![Box::new(DepSet::Values(vs!(["l1"])))]))),
            ("( l1 l2 )", Some(DepSet::AllOf(vec![Box::new(DepSet::Values(vs!(["l1", "l2"])))]))),
            ("|| ( l1 )", Some(DepSet::AnyOf(vec![Box::new(DepSet::Values(vs!(["l1"])))]))),
            (
                "|| ( l1 l2 )",
                Some(DepSet::AnyOf(vec![Box::new(DepSet::Values(vs!(["l1", "l2"])))])),
            ),
            (
                "use? ( l1 )",
                Some(DepSet::UseEnabled(
                    "use".to_string(),
                    vec![Box::new(DepSet::Values(vs!(["l1"])))],
                )),
            ),
            (
                "use? ( l1 l2 )",
                Some(DepSet::UseEnabled(
                    "use".to_string(),
                    vec![Box::new(DepSet::Values(vs!(["l1", "l2"])))],
                )),
            ),
            (
                "use? ( || ( l1 l2 ) )",
                Some(DepSet::UseEnabled(
                    "use".to_string(),
                    vec![Box::new(DepSet::AnyOf(vec![Box::new(DepSet::Values(vs!([
                        "l1", "l2"
                    ])))]))],
                )),
            ),
        ] {
            let result = parse::license(&s);
            assert!(result.is_ok(), "{s} failed: {}", result.err().unwrap());
            assert_eq!(result.unwrap(), expected);
        }
    }

    #[test]
    fn test_src_uri() {
        // invalid data
        for s in ["(", ")", "( )", "( uri)", "| ( uri )", "use ( uri )", "!use ( uri )"] {
            for eapi in EAPIS.values() {
                assert!(parse::src_uri(&s, eapi).is_err(), "{s:?} didn't fail");
            }
        }

        let uri = |u1: &str, u2: Option<&str>| Uri {
            uri: u1.to_string(),
            rename: u2.and_then(|s| Some(s.to_string())),
        };

        // good data
        for (s, expected) in [
            ("", None),
            ("uri1", Some(DepSet::Values(vec![uri("uri1", None)]))),
            ("uri1 uri2", Some(DepSet::Values(vec![uri("uri1", None), uri("uri2", None)]))),
            (
                "( uri1 uri2 )",
                Some(DepSet::AllOf(vec![Box::new(DepSet::Values(vec![
                    uri("uri1", None),
                    uri("uri2", None),
                ]))])),
            ),
            (
                "use? ( uri1 )",
                Some(DepSet::UseEnabled(
                    "use".to_string(),
                    vec![Box::new(DepSet::Values(vec![uri("uri1", None)]))],
                )),
            ),
        ] {
            for eapi in EAPIS.values() {
                let result = parse::src_uri(&s, eapi);
                assert!(result.is_ok(), "{s} failed: {}", result.err().unwrap());
                assert_eq!(result.unwrap(), expected);
            }
        }

        // SRC_URI renames
        for (s, expected) in
            [("uri1 -> file", Some(DepSet::Values(vec![uri("uri1", Some("file"))])))]
        {
            for eapi in EAPIS.values() {
                if eapi.has(Feature::SrcUriRenames) {
                    let result = parse::src_uri(&s, eapi);
                    assert!(result.is_ok(), "{s} failed: {}", result.err().unwrap());
                    assert_eq!(result.unwrap(), expected);
                }
            }
        }
    }

    #[test]
    fn test_required_use() {
        // invalid data
        for s in ["(", ")", "( )", "( u)", "| ( u )", "u1 ( u2 )", "!u1 ( u2 )"] {
            assert!(parse::required_use(&s, &EAPI_LATEST).is_err(), "{s:?} didn't fail");
        }

        // good data
        for (s, expected) in [
            ("", None),
            ("u", Some(DepSet::Values(vs!(["u"])))),
            ("u1 u2", Some(DepSet::Values(vs!(["u1", "u2"])))),
            ("( u )", Some(DepSet::AllOf(vec![Box::new(DepSet::Values(vs!(["u"])))]))),
            ("( u1 u2 )", Some(DepSet::AllOf(vec![Box::new(DepSet::Values(vs!(["u1", "u2"])))]))),
            ("|| ( u )", Some(DepSet::AnyOf(vec![Box::new(DepSet::Values(vs!(["u"])))]))),
            (
                "|| ( u1 u2 )",
                Some(DepSet::AnyOf(vec![Box::new(DepSet::Values(vs!(["u1", "u2"])))])),
            ),
            (
                "^^ ( u1 u2 )",
                Some(DepSet::ExactlyOneOf(vec![Box::new(DepSet::Values(vs!(["u1", "u2"])))])),
            ),
            (
                "u1? ( u2 )",
                Some(DepSet::UseEnabled(
                    "u1".to_string(),
                    vec![Box::new(DepSet::Values(vs!(["u2"])))],
                )),
            ),
            (
                "u1? ( u2 u3 )",
                Some(DepSet::UseEnabled(
                    "u1".to_string(),
                    vec![Box::new(DepSet::Values(vs!(["u2", "u3"])))],
                )),
            ),
            (
                "u1? ( || ( u2 u3 ) )",
                Some(DepSet::UseEnabled(
                    "u1".to_string(),
                    vec![Box::new(DepSet::AnyOf(vec![Box::new(DepSet::Values(vs!([
                        "u2", "u3"
                    ])))]))],
                )),
            ),
        ] {
            let result = parse::required_use(&s, &EAPI_LATEST);
            assert!(result.is_ok(), "{s} failed: {}", result.err().unwrap());
            assert_eq!(result.unwrap(), expected);
        }

        // ?? operator
        for (s, expected) in [(
            "?? ( u1 u2 )",
            Some(DepSet::AtMostOneOf(vec![Box::new(DepSet::Values(vs!(["u1", "u2"])))])),
        )] {
            for eapi in EAPIS.values() {
                if eapi.has(Feature::RequiredUseOneOf) {
                    let result = parse::required_use(&s, eapi);
                    assert!(result.is_ok(), "{s} failed: {}", result.err().unwrap());
                    assert_eq!(result.unwrap(), expected);
                }
            }
        }
    }

    #[test]
    fn test_pkgdep() {
        // invalid data
        for s in ["(", ")", "( )", "( a/b)", "| ( a/b )", "use ( a/b )", "!use ( a/b )"] {
            assert!(parse::pkgdep(&s, &EAPI_LATEST).is_err(), "{s:?} didn't fail");
        }

        let atom = |s| Atom::from_str(s).unwrap();

        // good data
        for (s, expected) in [
            ("", None),
            ("a/b", Some(DepSet::Values(vec![atom("a/b")]))),
            ("a/b c/d", Some(DepSet::Values(vec![atom("a/b"), atom("c/d")]))),
            (
                "( a/b c/d )",
                Some(DepSet::AllOf(vec![Box::new(DepSet::Values(vec![atom("a/b"), atom("c/d")]))])),
            ),
            (
                "u? ( a/b c/d )",
                Some(DepSet::UseEnabled(
                    "u".to_string(),
                    vec![Box::new(DepSet::Values(vec![atom("a/b"), atom("c/d")]))],
                )),
            ),
            (
                "!u? ( a/b c/d )",
                Some(DepSet::UseDisabled(
                    "u".to_string(),
                    vec![Box::new(DepSet::Values(vec![atom("a/b"), atom("c/d")]))],
                )),
            ),
            (
                "u1? ( a/b !u2? ( c/d ) )",
                Some(DepSet::UseEnabled(
                    "u1".to_string(),
                    vec![
                        Box::new(DepSet::Values(vec![atom("a/b")])),
                        Box::new(DepSet::UseDisabled(
                            "u2".to_string(),
                            vec![Box::new(DepSet::Values(vec![atom("c/d")]))],
                        )),
                    ],
                )),
            ),
        ] {
            let result = parse::pkgdep(&s, &EAPI_LATEST);
            assert!(result.is_ok(), "{s} failed: {}", result.err().unwrap());
            assert_eq!(result.unwrap(), expected);
        }
    }
}
