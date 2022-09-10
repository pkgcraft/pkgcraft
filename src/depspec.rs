use crate::atom::Atom;
use crate::eapi::{Eapi, Feature};
use crate::macros::vec_str;

#[derive(Debug, PartialEq, Eq)]
pub struct Uri {
    pub uri: String,
    pub rename: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DepSpec {
    Strings(Vec<String>),
    Atoms(Vec<Atom>),
    Uris(Vec<Uri>),
    AllOf(Box<DepSpec>),
    AnyOf(Box<DepSpec>),
    ExactlyOneOf(Box<DepSpec>), // REQUIRED_USE only
    AtMostOneOf(Box<DepSpec>),  // REQUIRED_USE only
    ConditionalUse(String, bool, Box<DepSpec>),
}

peg::parser!(pub grammar depspec() for str {
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
        = vals:license_name() ++ " " { DepSpec::Strings(vec_str!(vals)) }

    rule useflags() -> DepSpec
        = vals:useflag() ++ " " { DepSpec::Strings(vec_str!(vals)) }

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
            DepSpec::ConditionalUse(u.to_string(), negate.is_some(), Box::new(e))
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

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::eapi::{EAPIS, EAPI_LATEST};
    use crate::peg::PegError;

    use super::*;

    #[test]
    fn test_license() {
        // invalid data
        for s in ["", "(", ")", "( )", "( l1)", "| ( l1 )", "foo ( l1 )", "!use ( l1 )"] {
            assert!(depspec::license(&s).is_err(), "{s:?} didn't fail");
        }

        // good data
        let mut result: Result<DepSpec, PegError>;
        for (s, expected) in [
            ("l1", DepSpec::Strings(vec_str!(["l1"]))),
            ("l1 l2", DepSpec::Strings(vec_str!(["l1", "l2"]))),
            ("( l1 )", DepSpec::AllOf(Box::new(DepSpec::Strings(vec_str!(["l1"]))))),
            ("( l1 l2 )", DepSpec::AllOf(Box::new(DepSpec::Strings(vec_str!(["l1", "l2"]))))),
            ("|| ( l1 )", DepSpec::AnyOf(Box::new(DepSpec::Strings(vec_str!(["l1"]))))),
            ("|| ( l1 l2 )", DepSpec::AnyOf(Box::new(DepSpec::Strings(vec_str!(["l1", "l2"]))))),
            (
                "use? ( l1 )",
                DepSpec::ConditionalUse(
                    "use".to_string(),
                    false,
                    Box::new(DepSpec::Strings(vec_str!(["l1"]))),
                ),
            ),
            (
                "use? ( l1 l2 )",
                DepSpec::ConditionalUse(
                    "use".to_string(),
                    false,
                    Box::new(DepSpec::Strings(vec_str!(["l1", "l2"]))),
                ),
            ),
            (
                "use? ( || ( l1 l2 ) )",
                DepSpec::ConditionalUse(
                    "use".to_string(),
                    false,
                    Box::new(DepSpec::AnyOf(Box::new(DepSpec::Strings(vec_str!(["l1", "l2"]))))),
                ),
            ),
        ] {
            result = depspec::license(&s);
            assert!(result.is_ok(), "{s} failed: {}", result.err().unwrap());
            assert_eq!(result.unwrap(), expected);
        }
    }

    #[test]
    fn test_src_uri() {
        // invalid data
        let mut result: Result<DepSpec, PegError>;
        for s in ["", "(", ")", "( )", "( uri)", "| ( uri )", "use ( uri )", "!use ( uri )"] {
            for eapi in EAPIS.values() {
                assert!(depspec::src_uri(&s, eapi).is_err(), "{s:?} didn't fail");
            }
        }

        let uri = |u1: &str, u2: Option<&str>| Uri {
            uri: u1.to_string(),
            rename: u2.and_then(|s| Some(s.to_string())),
        };

        // good data
        let mut src_uri;
        for (s, expected) in [
            ("uri1", DepSpec::Uris(vec![uri("uri1", None)])),
            ("uri1 uri2", DepSpec::Uris(vec![uri("uri1", None), uri("uri2", None)])),
            (
                "( uri1 uri2 )",
                DepSpec::AllOf(Box::new(DepSpec::Uris(vec![uri("uri1", None), uri("uri2", None)]))),
            ),
            (
                "use? ( uri1 )",
                DepSpec::ConditionalUse(
                    "use".to_string(),
                    false,
                    Box::new(DepSpec::Uris(vec![uri("uri1", None)])),
                ),
            ),
        ] {
            for eapi in EAPIS.values() {
                result = depspec::src_uri(&s, eapi);
                assert!(result.is_ok(), "{s} failed: {}", result.err().unwrap());
                src_uri = result.unwrap();
                assert_eq!(src_uri, expected);
            }
        }

        // SRC_URI renames
        for (s, expected) in [("uri1 -> file", DepSpec::Uris(vec![uri("uri1", Some("file"))]))] {
            for eapi in EAPIS.values() {
                if eapi.has(Feature::SrcUriRenames) {
                    result = depspec::src_uri(&s, eapi);
                    assert!(result.is_ok(), "{s} failed: {}", result.err().unwrap());
                    src_uri = result.unwrap();
                    assert_eq!(src_uri, expected);
                }
            }
        }
    }

    #[test]
    fn test_required_use() {
        // invalid data
        for s in ["", "(", ")", "( )", "( u)", "| ( u )", "u1 ( u2 )", "!u1 ( u2 )"] {
            assert!(depspec::required_use(&s, &EAPI_LATEST).is_err(), "{s:?} didn't fail");
        }

        // good data
        let mut required_use;
        let mut result: Result<DepSpec, PegError>;
        for (s, expected) in [
            ("u", DepSpec::Strings(vec_str!(["u"]))),
            ("u1 u2", DepSpec::Strings(vec_str!(["u1", "u2"]))),
            ("( u )", DepSpec::AllOf(Box::new(DepSpec::Strings(vec_str!(["u"]))))),
            ("( u1 u2 )", DepSpec::AllOf(Box::new(DepSpec::Strings(vec_str!(["u1", "u2"]))))),
            ("|| ( u )", DepSpec::AnyOf(Box::new(DepSpec::Strings(vec_str!(["u"]))))),
            ("|| ( u1 u2 )", DepSpec::AnyOf(Box::new(DepSpec::Strings(vec_str!(["u1", "u2"]))))),
            (
                "^^ ( u1 u2 )",
                DepSpec::ExactlyOneOf(Box::new(DepSpec::Strings(vec_str!(["u1", "u2"])))),
            ),
            (
                "u1? ( u2 )",
                DepSpec::ConditionalUse(
                    "u1".to_string(),
                    false,
                    Box::new(DepSpec::Strings(vec_str!(["u2"]))),
                ),
            ),
            (
                "u1? ( u2 u3 )",
                DepSpec::ConditionalUse(
                    "u1".to_string(),
                    false,
                    Box::new(DepSpec::Strings(vec_str!(["u2", "u3"]))),
                ),
            ),
            (
                "u1? ( || ( u2 u3 ) )",
                DepSpec::ConditionalUse(
                    "u1".to_string(),
                    false,
                    Box::new(DepSpec::AnyOf(Box::new(DepSpec::Strings(vec_str!(["u2", "u3"]))))),
                ),
            ),
        ] {
            result = depspec::required_use(&s, &EAPI_LATEST);
            assert!(result.is_ok(), "{s} failed: {}", result.err().unwrap());
            required_use = result.unwrap();
            assert_eq!(required_use, expected);
        }

        // ?? operator
        for (s, expected) in [(
            "?? ( u1 u2 )",
            DepSpec::AtMostOneOf(Box::new(DepSpec::Strings(vec_str!(["u1", "u2"])))),
        )] {
            for eapi in EAPIS.values() {
                if eapi.has(Feature::RequiredUseOneOf) {
                    result = depspec::required_use(&s, eapi);
                    assert!(result.is_ok(), "{s} failed: {}", result.err().unwrap());
                    required_use = result.unwrap();
                    assert_eq!(required_use, expected);
                }
            }
        }
    }

    #[test]
    fn test_pkgdep() {
        // invalid data
        for s in ["", "(", ")", "( )", "( a/b)", "| ( a/b )", "use ( a/b )", "!use ( a/b )"] {
            assert!(depspec::pkgdep(&s, &EAPI_LATEST).is_err(), "{s:?} didn't fail");
        }

        let atom = |s| Atom::from_str(s).unwrap();

        // good data
        let mut deps;
        let mut result: Result<DepSpec, PegError>;
        for (s, expected) in [
            ("a/b", DepSpec::Atoms(vec![atom("a/b")])),
            ("a/b c/d", DepSpec::Atoms(vec![atom("a/b"), atom("c/d")])),
        ] {
            result = depspec::pkgdep(&s, &EAPI_LATEST);
            assert!(result.is_ok(), "{s} failed: {}", result.err().unwrap());
            deps = result.unwrap();
            assert_eq!(deps, expected);
        }
    }
}
