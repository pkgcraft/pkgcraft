use peg;

use crate::atom::ParseError;
use crate::macros::vec_str;
use super::DepSpec;

peg::parser!{
    pub grammar required_use() for str {
        rule _ = [' ']

        rule useflag() -> &'input str
            = s:$(quiet!{
                ['a'..='z' | 'A'..='Z' | '0'..='9']
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '@' | '-']*
            } / expected!("useflag name")
            ) { s }

        rule useflags() -> DepSpec
            = useflags:useflag() ++ " " { DepSpec::Names(vec_str!(useflags)) }

        rule all_of() -> DepSpec
            = "(" _ e:expr() _ ")" {
                DepSpec::AllOf(Box::new(e))
            }

        rule any_of() -> DepSpec
            = "||" _ "(" _ e:expr() _ ")" {
                DepSpec::AnyOf(Box::new(e))
            }

        rule exactly_one_of() -> DepSpec
            = "^^" _ "(" _ e:expr() _ ")" {
                DepSpec::ExactlyOneOf(Box::new(e))
            }

        rule at_most_one_of() -> DepSpec
            = "??" _ "(" _ e:expr() _ ")" {
                DepSpec::AtMostOneOf(Box::new(e))
            }

        // TODO: handle negation
        rule conditional() -> DepSpec
            = "!"? u:useflag() "?" _ "(" _ e:expr() _ ")" {
                DepSpec::ConditionalUse(u.to_string(), Box::new(e))
            }

        pub rule expr() -> DepSpec
            = conditional() / any_of() / all_of() /
                exactly_one_of() / at_most_one_of() / useflags()
    }
}

pub fn parse(s: &str) -> Result<DepSpec, ParseError> {
    required_use::expr(s)
}

#[cfg(test)]
mod tests {
    use crate::depspec::DepSpec;
    use crate::atom::ParseError;
    use crate::macros::vec_str;

    use super::required_use::expr as parse;

    #[test]
    fn test_parse_required_use() {
        // invalid data
        for s in [
                "", "( )", "( u)", "| ( u )", "u1 ( u2 )", "!u1 ( u2 )"
                ] {
            assert!(parse(&s).is_err(), "{} didn't fail", s);
        }

        // good data
        let mut required_use;
        let mut result: Result<DepSpec, ParseError>;
        for (s, expected) in [
                ("u", DepSpec::Names(vec_str!(["u"]))),
                ("u1 u2", DepSpec::Names(vec_str!(["u1", "u2"]))),
                ("( u )",
                 DepSpec::AllOf(Box::new(DepSpec::Names(vec_str!(["u"]))))),
                ("( u1 u2 )",
                 DepSpec::AllOf(Box::new(DepSpec::Names(vec_str!(["u1", "u2"]))))),
                ("|| ( u )",
                 DepSpec::AnyOf(Box::new(DepSpec::Names(vec_str!(["u"]))))),
                ("|| ( u1 u2 )",
                 DepSpec::AnyOf(Box::new(DepSpec::Names(vec_str!(["u1", "u2"]))))),
                ("^^ ( u1 u2 )",
                 DepSpec::ExactlyOneOf(Box::new(DepSpec::Names(vec_str!(["u1", "u2"]))))),
                ("?? ( u1 u2 )",
                 DepSpec::AtMostOneOf(Box::new(DepSpec::Names(vec_str!(["u1", "u2"]))))),
                ("u1? ( u2 )",
                 DepSpec::ConditionalUse(
                    "u1".to_string(),
                    Box::new(DepSpec::Names(vec_str!(["u2"]))))),
                ("u1? ( u2 u3 )",
                 DepSpec::ConditionalUse(
                    "u1".to_string(),
                    Box::new(DepSpec::Names(vec_str!(["u2", "u3"]))))),
                ("u1? ( || ( u2 u3 ) )",
                 DepSpec::ConditionalUse(
                    "u1".to_string(),
                    Box::new(DepSpec::AnyOf(
                        Box::new(DepSpec::Names(vec_str!(["u2", "u3"]))))))),
                ] {
            result = parse(&s);
            assert!(result.is_ok(), "{} failed: {}", s, result.err().unwrap());
            required_use = result.unwrap();
            assert_eq!(required_use, expected);
        }
    }
}
