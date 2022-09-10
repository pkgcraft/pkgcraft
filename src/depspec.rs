use crate::atom::Atom;
use crate::eapi::{Eapi, Feature};
use crate::macros::vec_str as vs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Uri {
    pub uri: String,
    pub rename: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DepSpec {
    Values(Vec<String>),
    Atoms(Vec<Atom>),
    Uris(Vec<Uri>),
    AllOf(Box<DepSpec>),
    AnyOf(Box<DepSpec>),
    ExactlyOneOf(Box<DepSpec>), // REQUIRED_USE only
    AtMostOneOf(Box<DepSpec>),  // REQUIRED_USE only
    UseEnabled(String, Box<DepSpec>),
    UseDisabled(String, Box<DepSpec>),
}

peg::parser!(grammar depspec() for str {
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

    rule dep(eapi: &'static Eapi) -> Atom
        = s:$(!['(' | ')'] [^' ']+) {?
            let atom = match Atom::new(s, eapi) {
                Ok(x) => x,
                Err(e) => return Err("failed parsing atom"),
            };
            Ok(atom)
        }

    rule deps(eapi: &'static Eapi) -> DepSpec
        = vals:dep(eapi) ++ " " { DepSpec::Atoms(vals) }

    rule licenses() -> DepSpec
        = vals:license_name() ++ " " { DepSpec::Values(vs!(vals)) }

    rule useflags() -> DepSpec
        = vals:useflag() ++ " " { DepSpec::Values(vs!(vals)) }

    rule uri() -> &'input str
        = s:$(quiet!{!['(' | ')'] [^' ']+}) { s }

    rule uris(eapi: &'static Eapi) -> DepSpec
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

            DepSpec::Uris(uri_objs)
        }

    rule parens<T>(expr: rule<T>) -> T = "(" _ v:expr() _ ")" { v }

    rule all_of(expr: rule<DepSpec>) -> DepSpec
        = e:parens(<expr()>) {
            DepSpec::AllOf(Box::new(e))
        }

    rule any_of(expr: rule<DepSpec>) -> DepSpec
        = "||" _ e:parens(<expr()>) {
            DepSpec::AnyOf(Box::new(e))
        }

    rule conditional(expr: rule<DepSpec>) -> DepSpec
        = negate:"!"? u:useflag() "?" _ e:parens(<expr()>) {
            let f = match negate {
                None => DepSpec::UseEnabled,
                Some(_) => DepSpec::UseDisabled,
            };
            f(u.to_string(), Box::new(e))
        }

    rule exactly_one_of(expr: rule<DepSpec>) -> DepSpec
        = "^^" _ e:parens(<expr()>) {
            DepSpec::ExactlyOneOf(Box::new(e))
        }

    rule at_most_one_of(eapi: &'static Eapi, expr: rule<DepSpec>) -> DepSpec
        = "??" _ e:parens(<expr()>) {?
            if !eapi.has(Feature::RequiredUseOneOf) {
                return Err("?? groups are supported in >= EAPI 5");
            }
            Ok(DepSpec::AtMostOneOf(Box::new(e)))
        }

    pub(super) rule license() -> DepSpec
        = conditional(<license()>) / any_of(<license()>) / all_of(<license()>) / licenses()

    pub(super) rule src_uri(eapi: &'static Eapi) -> DepSpec
        = conditional(<src_uri(eapi)>) / all_of(<src_uri(eapi)>) / uris(eapi)

    pub(super) rule required_use(eapi: &'static Eapi) -> DepSpec
        = conditional(<required_use(eapi)>)
            / any_of(<required_use(eapi)>)
            / all_of(<required_use(eapi)>)
            / exactly_one_of(<required_use(eapi)>)
            / at_most_one_of(eapi, <required_use(eapi)>)
            / useflags()

    pub(super) rule pkgdep(eapi: &'static Eapi) -> DepSpec
        = conditional(<pkgdep(eapi)>)
            / any_of(<pkgdep(eapi)>)
            / all_of(<pkgdep(eapi)>)
            / deps(eapi)
});

// provide public parsing functionality while converting error types
pub mod parse {
    use crate::peg::peg_error;

    use super::*;

    pub fn license(s: &str) -> crate::Result<Option<DepSpec>> {
        match s.is_empty() {
            true => Ok(None),
            false => depspec::license(s)
                .map(Some)
                .map_err(|e| peg_error(format!("invalid license: {s:?}"), s, e)),
        }
    }

    pub fn src_uri(s: &str, eapi: &'static Eapi) -> crate::Result<Option<DepSpec>> {
        match s.is_empty() {
            true => Ok(None),
            false => depspec::src_uri(s, eapi)
                .map(Some)
                .map_err(|e| peg_error(format!("invalid SRC_URI: {s:?}"), s, e)),
        }
    }

    pub fn required_use(s: &str, eapi: &'static Eapi) -> crate::Result<Option<DepSpec>> {
        match s.is_empty() {
            true => Ok(None),
            false => depspec::required_use(s, eapi)
                .map(Some)
                .map_err(|e| peg_error(format!("invalid REQUIRED_USE: {s:?}"), s, e)),
        }
    }

    pub fn pkgdep(s: &str, eapi: &'static Eapi) -> crate::Result<Option<DepSpec>> {
        match s.is_empty() {
            true => Ok(None),
            false => depspec::pkgdep(s, eapi)
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
            ("l1", Some(DepSpec::Values(vs!(["l1"])))),
            ("l1 l2", Some(DepSpec::Values(vs!(["l1", "l2"])))),
            ("( l1 )", Some(DepSpec::AllOf(Box::new(DepSpec::Values(vs!(["l1"])))))),
            ("( l1 l2 )", Some(DepSpec::AllOf(Box::new(DepSpec::Values(vs!(["l1", "l2"])))))),
            ("|| ( l1 )", Some(DepSpec::AnyOf(Box::new(DepSpec::Values(vs!(["l1"])))))),
            ("|| ( l1 l2 )", Some(DepSpec::AnyOf(Box::new(DepSpec::Values(vs!(["l1", "l2"])))))),
            (
                "use? ( l1 )",
                Some(DepSpec::UseEnabled(
                    "use".to_string(),
                    false,
                    Box::new(DepSpec::Values(vs!(["l1"]))),
                )),
            ),
            (
                "use? ( l1 l2 )",
                Some(DepSpec::UseEnabled(
                    "use".to_string(),
                    false,
                    Box::new(DepSpec::Values(vs!(["l1", "l2"]))),
                )),
            ),
            (
                "use? ( || ( l1 l2 ) )",
                Some(DepSpec::UseEnabled(
                    "use".to_string(),
                    false,
                    Box::new(DepSpec::AnyOf(Box::new(DepSpec::Values(vs!(["l1", "l2"]))))),
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
            ("uri1", Some(DepSpec::Uris(vec![uri("uri1", None)]))),
            ("uri1 uri2", Some(DepSpec::Uris(vec![uri("uri1", None), uri("uri2", None)]))),
            (
                "( uri1 uri2 )",
                Some(DepSpec::AllOf(Box::new(DepSpec::Uris(vec![
                    uri("uri1", None),
                    uri("uri2", None),
                ])))),
            ),
            (
                "use? ( uri1 )",
                Some(DepSpec::UseEnabled(
                    "use".to_string(),
                    false,
                    Box::new(DepSpec::Uris(vec![uri("uri1", None)])),
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
            [("uri1 -> file", Some(DepSpec::Uris(vec![uri("uri1", Some("file"))])))]
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
            ("u", Some(DepSpec::Values(vs!(["u"])))),
            ("u1 u2", Some(DepSpec::Values(vs!(["u1", "u2"])))),
            ("( u )", Some(DepSpec::AllOf(Box::new(DepSpec::Values(vs!(["u"])))))),
            ("( u1 u2 )", Some(DepSpec::AllOf(Box::new(DepSpec::Values(vs!(["u1", "u2"])))))),
            ("|| ( u )", Some(DepSpec::AnyOf(Box::new(DepSpec::Values(vs!(["u"])))))),
            ("|| ( u1 u2 )", Some(DepSpec::AnyOf(Box::new(DepSpec::Values(vs!(["u1", "u2"])))))),
            (
                "^^ ( u1 u2 )",
                Some(DepSpec::ExactlyOneOf(Box::new(DepSpec::Values(vs!(["u1", "u2"]))))),
            ),
            (
                "u1? ( u2 )",
                Some(DepSpec::UseEnabled(
                    "u1".to_string(),
                    false,
                    Box::new(DepSpec::Values(vs!(["u2"]))),
                )),
            ),
            (
                "u1? ( u2 u3 )",
                Some(DepSpec::UseEnabled(
                    "u1".to_string(),
                    false,
                    Box::new(DepSpec::Values(vs!(["u2", "u3"]))),
                )),
            ),
            (
                "u1? ( || ( u2 u3 ) )",
                Some(DepSpec::UseEnabled(
                    "u1".to_string(),
                    false,
                    Box::new(DepSpec::AnyOf(Box::new(DepSpec::Values(vs!(["u2", "u3"]))))),
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
            Some(DepSpec::AtMostOneOf(Box::new(DepSpec::Values(vs!(["u1", "u2"]))))),
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
            ("a/b", Some(DepSpec::Atoms(vec![atom("a/b")]))),
            ("a/b c/d", Some(DepSpec::Atoms(vec![atom("a/b"), atom("c/d")]))),
        ] {
            let result = parse::pkgdep(&s, &EAPI_LATEST);
            assert!(result.is_ok(), "{s} failed: {}", result.err().unwrap());
            assert_eq!(result.unwrap(), expected);
        }
    }
}
