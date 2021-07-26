use peg;

use super::DepSpec;
use crate::eapi::Eapi;
use crate::macros::vec_str;

peg::parser! {
    pub grammar depspec() for str {
        rule _ = [' ']

        rule useflag() -> &'input str
            = s:$(quiet!{
                ['a'..='z' | 'A'..='Z' | '0'..='9']
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '@' | '-']*
            } / expected!("useflag name")
            ) { s }

        rule useflags() -> DepSpec
            = useflags:useflag() ++ " " { DepSpec::Strings(vec_str!(useflags)) }

        rule all_of(eapi: &'static Eapi) -> DepSpec
            = "(" _ e:expr(eapi) _ ")" {
                DepSpec::AllOf(Box::new(e))
            }

        rule any_of(eapi: &'static Eapi) -> DepSpec
            = "||" _ "(" _ e:expr(eapi) _ ")" {
                DepSpec::AnyOf(Box::new(e))
            }

        rule exactly_one_of(eapi: &'static Eapi) -> DepSpec
            = "^^" _ "(" _ e:expr(eapi) _ ")" {
                DepSpec::ExactlyOneOf(Box::new(e))
            }

        rule at_most_one_of(eapi: &'static Eapi) -> DepSpec
            = "??" _ "(" _ e:expr(eapi) _ ")" {?
                if !eapi.has("required_use_one_of") {
                    return Err("?? groups are supported in >= EAPI 5");
                }
                Ok(DepSpec::AtMostOneOf(Box::new(e)))
            }

        rule conditional(eapi: &'static Eapi) -> DepSpec
            = negate:"!"? u:useflag() "?" _ "(" _ e:expr(eapi) _ ")" {
                DepSpec::ConditionalUse(u.to_string(), negate.is_some(), Box::new(e))
            }

        pub rule expr(eapi: &'static Eapi) -> DepSpec
            = conditional(eapi) / any_of(eapi) / all_of(eapi) /
                exactly_one_of(eapi) / at_most_one_of(eapi) / useflags()
    }
}

// export depspec parser
pub use depspec::expr as parse;

#[cfg(test)]
mod tests {
    use crate::atom::ParseError;
    use crate::depspec::DepSpec;
    use crate::eapi;
    use crate::macros::vec_str;

    use super::parse;

    #[test]
    fn test_parse_required_use() {
        // invalid data
        for s in [
            "",
            "(",
            ")",
            "( )",
            "( u)",
            "| ( u )",
            "u1 ( u2 )",
            "!u1 ( u2 )",
        ] {
            assert!(parse(&s, eapi::EAPI_LATEST).is_err(), "{} didn't fail", s);
        }

        // good data
        let mut required_use;
        let mut result: Result<DepSpec, ParseError>;
        for (s, expected) in [
            ("u", DepSpec::Strings(vec_str!(["u"]))),
            ("u1 u2", DepSpec::Strings(vec_str!(["u1", "u2"]))),
            (
                "( u )",
                DepSpec::AllOf(Box::new(DepSpec::Strings(vec_str!(["u"])))),
            ),
            (
                "( u1 u2 )",
                DepSpec::AllOf(Box::new(DepSpec::Strings(vec_str!(["u1", "u2"])))),
            ),
            (
                "|| ( u )",
                DepSpec::AnyOf(Box::new(DepSpec::Strings(vec_str!(["u"])))),
            ),
            (
                "|| ( u1 u2 )",
                DepSpec::AnyOf(Box::new(DepSpec::Strings(vec_str!(["u1", "u2"])))),
            ),
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
                    Box::new(DepSpec::AnyOf(Box::new(DepSpec::Strings(vec_str!([
                        "u2", "u3"
                    ]))))),
                ),
            ),
        ] {
            result = parse(&s, eapi::EAPI_LATEST);
            assert!(result.is_ok(), "{} failed: {}", s, result.err().unwrap());
            required_use = result.unwrap();
            assert_eq!(required_use, expected);
        }

        // ?? operator
        for (s, expected) in [(
            "?? ( u1 u2 )",
            DepSpec::AtMostOneOf(Box::new(DepSpec::Strings(vec_str!(["u1", "u2"])))),
        )] {
            for eapi in eapi::KNOWN_EAPIS.values() {
                if eapi.has("required_use_one_of") {
                    result = parse(&s, eapi);
                    assert!(result.is_ok(), "{} failed: {}", s, result.err().unwrap());
                    required_use = result.unwrap();
                    assert_eq!(required_use, expected);
                }
            }
        }
    }
}
