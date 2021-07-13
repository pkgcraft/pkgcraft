use peg;

use super::DepSpec;
use crate::macros::vec_str;

peg::parser!{
    pub grammar parse() for str {
        rule _ = [' ']

        // licenses must not begin with a hyphen, dot, or plus sign.
        rule name() -> &'input str
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

        rule names() -> DepSpec
            = names:name() ++ " " { DepSpec::Names(vec_str!(names)) }

        rule all_of() -> DepSpec
            = "(" _ e:expr() _ ")" {
                DepSpec::AllOf(Box::new(e))
            }

        rule any_of() -> DepSpec
            = "||" _ "(" _ e:expr() _ ")" {
                DepSpec::AnyOf(Box::new(e))
            }

        // TODO: handle negation
        rule conditional() -> DepSpec
            = "!"? u:useflag() "?" _ "(" _ e:expr() _ ")" {
                DepSpec::ConditionalUse(u.to_string(), Box::new(e))
            }

        pub rule expr() -> DepSpec
            = conditional() / any_of() / all_of() / names()
    }
}

#[cfg(test)]
mod tests {
    use crate::depspec::DepSpec;
    use crate::atom::ParseError;
    use crate::macros::vec_str;

    use super::parse::expr as parse;

    #[test]
    fn test_parse_license() {
        // invalid data
        for s in [
                "", "( )", "( l1)", "| ( l1 )", "foo ( l1 )", "!use ( l1 )"
                ] {
            assert!(parse(&s).is_err(), "{} didn't fail", s);
        }

        // good data
        let mut license;
        let mut result: Result<DepSpec, ParseError>;
        for (s, expected) in [
                ("l1", DepSpec::Names(vec_str!(["l1"]))),
                ("l1 l2", DepSpec::Names(vec_str!(["l1", "l2"]))),
                ("( l1 )",
                 DepSpec::AllOf(Box::new(DepSpec::Names(vec_str!(["l1"]))))),
                ("( l1 l2 )",
                 DepSpec::AllOf(Box::new(DepSpec::Names(vec_str!(["l1", "l2"]))))),
                ("|| ( l1 )",
                 DepSpec::AnyOf(Box::new(DepSpec::Names(vec_str!(["l1"]))))),
                ("|| ( l1 l2 )",
                 DepSpec::AnyOf(Box::new(DepSpec::Names(vec_str!(["l1", "l2"]))))),
                ("use? ( l1 )",
                 DepSpec::ConditionalUse(
                    "use".to_string(),
                    Box::new(DepSpec::Names(vec_str!(["l1"]))))),
                ("use? ( l1 l2 )",
                 DepSpec::ConditionalUse(
                    "use".to_string(),
                    Box::new(DepSpec::Names(vec_str!(["l1", "l2"]))))),
                ("use? ( || ( l1 l2 ) )",
                 DepSpec::ConditionalUse(
                    "use".to_string(),
                    Box::new(DepSpec::AnyOf(
                        Box::new(DepSpec::Names(vec_str!(["l1", "l2"]))))))),
                ] {
            result = parse(&s);
            assert!(result.is_ok(), "{} failed: {}", s, result.err().unwrap());
            license = result.unwrap();
            assert_eq!(license, expected);
        }
    }
}
