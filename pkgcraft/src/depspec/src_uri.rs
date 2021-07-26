use peg;

use super::{DepSpec, Uri};
use crate::atom::ParseError;
use crate::eapi::Eapi;

peg::parser! {
    pub grammar depspec() for str {
        rule _ = [' ']

        rule uri() -> &'input str
            = s:$(quiet!{!['(' | ')'] [^' ']+}) { s }

        rule useflag() -> &'input str
            = s:$(quiet!{
                ['a'..='z' | 'A'..='Z' | '0'..='9']
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '@' | '-']*
            } / expected!("useflag name")
            ) { s }

        rule uris(eapi: &'static Eapi) -> DepSpec
            = uris:uri() ++ " " {
                let mut uri_objs: Vec<Uri> = Vec::new();

                if eapi.has("src_uri_renames") {
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

        rule all_of(eapi: &'static Eapi) -> DepSpec
            = "(" _ e:expr(eapi) _ ")" {
                DepSpec::AllOf(Box::new(e))
            }

        rule conditional(eapi: &'static Eapi) -> DepSpec
            = negate:"!"? u:useflag() "?" _ "(" _ e:expr(eapi) _ ")" {
                DepSpec::ConditionalUse(u.to_string(), negate.is_some(), Box::new(e))
            }

        pub rule expr(eapi: &'static Eapi) -> DepSpec
            = conditional(eapi) / all_of(eapi) / uris(eapi)
    }
}

// export depspec parser
pub use depspec::expr as parse;

#[cfg(test)]
mod tests {
    use crate::atom::ParseError;
    use crate::depspec::{DepSpec, Uri};
    use crate::eapi;

    use super::parse;

    #[test]
    fn test_parse_src_uri() {
        // invalid data
        let mut result: Result<DepSpec, ParseError>;
        for s in [
            "",
            "(",
            ")",
            "( )",
            "( uri)",
            "| ( uri )",
            "use ( uri )",
            "!use ( uri )",
        ] {
            for eapi in eapi::KNOWN_EAPIS.values() {
                assert!(parse(&s, eapi).is_err(), "{} didn't fail", s);
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
            (
                "uri1 uri2",
                DepSpec::Uris(vec![uri("uri1", None), uri("uri2", None)]),
            ),
            (
                "( uri1 uri2 )",
                DepSpec::AllOf(Box::new(DepSpec::Uris(vec![
                    uri("uri1", None),
                    uri("uri2", None),
                ]))),
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
            for eapi in eapi::KNOWN_EAPIS.values() {
                result = parse(&s, eapi);
                assert!(result.is_ok(), "{} failed: {}", s, result.err().unwrap());
                src_uri = result.unwrap();
                assert_eq!(src_uri, expected);
            }
        }

        // SRC_URI renames
        for (s, expected) in [(
            "uri1 -> file",
            DepSpec::Uris(vec![uri("uri1", Some("file"))]),
        )] {
            for eapi in eapi::KNOWN_EAPIS.values() {
                if eapi.has("src_uri_renames") {
                    result = parse(&s, eapi);
                    assert!(result.is_ok(), "{} failed: {}", s, result.err().unwrap());
                    src_uri = result.unwrap();
                    assert_eq!(src_uri, expected);
                }
            }
        }
    }
}
