use peg;

use super::DepSpec;
use crate::atom;
use crate::eapi::Eapi;

peg::parser! {
    pub grammar depspec() for str {
        rule _ = [' ']

        rule dep(eapi: &'static Eapi) -> atom::Atom
            = s:$(!['(' | ')'] [^' ']+) {?
                let atom = match atom::parse::dep(s, eapi) {
                    Ok(x) => x,
                    Err(e) => return Err("failed parsing atom"),
                };
                Ok(atom)
            }

        rule useflag() -> &'input str
            = s:$(quiet!{
                ['a'..='z' | 'A'..='Z' | '0'..='9']
                ['a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '_' | '@' | '-']*
            } / expected!("useflag name")
            ) { s }

        rule deps(eapi: &'static Eapi) -> DepSpec
            = deps:dep(eapi) ++ " " { DepSpec::Atoms(deps) }

        rule all_of(eapi: &'static Eapi) -> DepSpec
            = "(" _ e:expr(eapi) _ ")" {
                DepSpec::AllOf(Box::new(e))
            }

        rule any_of(eapi: &'static Eapi) -> DepSpec
            = "||" _ "(" _ e:expr(eapi) _ ")" {
                DepSpec::AnyOf(Box::new(e))
            }

        rule conditional(eapi: &'static Eapi) -> DepSpec
            = negate:"!"? u:useflag() "?" _ "(" _ e:expr(eapi) _ ")" {
                DepSpec::ConditionalUse(u.to_string(), negate.is_some(), Box::new(e))
            }

        pub rule expr(eapi: &'static Eapi) -> DepSpec
            = conditional(eapi) / any_of(eapi) / all_of(eapi) / deps(eapi)
    }
}

// export depspec parser
pub use depspec::expr as parse;

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::atom::Atom;
    use crate::depspec::DepSpec;
    use crate::eapi;
    use crate::peg::PegError;

    use super::parse;

    #[test]
    fn test_parse_deps() {
        // invalid data
        for s in ["", "(", ")", "( )", "( a/b)", "| ( a/b )", "use ( a/b )", "!use ( a/b )"] {
            assert!(parse(&s, &eapi::EAPI_LATEST).is_err(), "{s:?} didn't fail");
        }

        let atom = |s| Atom::from_str(s).unwrap();

        // good data
        let mut deps;
        let mut result: Result<DepSpec, PegError>;
        for (s, expected) in [
            ("a/b", DepSpec::Atoms(vec![atom("a/b")])),
            ("a/b c/d", DepSpec::Atoms(vec![atom("a/b"), atom("c/d")])),
        ] {
            result = parse(&s, &eapi::EAPI_LATEST);
            assert!(result.is_ok(), "{s} failed: {}", result.err().unwrap());
            deps = result.unwrap();
            assert_eq!(deps, expected);
        }
    }
}
