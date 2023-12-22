use std::cmp::Ordering;

use crate::dep::{self, Stringable};
use crate::macros::{cmp_not_equal, equivalent};
use crate::traits::IntoOwned;

/// Package IUSE.
#[derive(Debug, Eq, Hash, Clone)]
pub struct Iuse<S: Stringable> {
    pub(crate) default: Option<bool>,
    pub(crate) flag: S,
}

impl IntoOwned for Iuse<&str> {
    type Owned = Iuse<String>;

    fn into_owned(self) -> Self::Owned {
        Iuse {
            flag: self.flag.to_string(),
            default: self.default,
        }
    }
}

impl Iuse<String> {
    /// Create an owned [`Iuse`] from a given string.
    pub fn try_new(s: &str) -> crate::Result<Self> {
        Iuse::parse(s).into_owned()
    }
}

impl<'a> Iuse<&'a str> {
    /// Create a borrowed [`Iuse`] from a given string.
    pub fn parse(s: &'a str) -> crate::Result<Self> {
        dep::parse::iuse(s)
    }
}

impl<S: Stringable> Iuse<S> {
    /// Return the USE flag.
    pub fn flag(&self) -> &str {
        self.flag.as_ref()
    }

    /// Return the default status, if it exists.
    pub fn default(&self) -> Option<bool> {
        self.default
    }
}

impl<S1: Stringable, S2: Stringable> PartialEq<Iuse<S1>> for Iuse<S2> {
    fn eq(&self, other: &Iuse<S1>) -> bool {
        self.default == other.default && self.flag() == other.flag()
    }
}

/// Compare two [`Iuse`] where flag name priority comes before defaults.
fn cmp<S1, S2>(u1: &Iuse<S1>, u2: &Iuse<S2>) -> Ordering
where
    S1: Stringable,
    S2: Stringable,
{
    cmp_not_equal!(u1.flag(), u2.flag());
    u1.default.cmp(&u2.default)
}

impl<S: Stringable> Ord for Iuse<S> {
    fn cmp(&self, other: &Self) -> Ordering {
        cmp(self, other)
    }
}

impl<S1: Stringable, S2: Stringable> PartialOrd<Iuse<S1>> for Iuse<S2> {
    fn partial_cmp(&self, other: &Iuse<S1>) -> Option<Ordering> {
        Some(cmp(self, other))
    }
}

equivalent!(Iuse);

impl std::str::FromStr for Iuse<String> {
    type Err = crate::Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Self::try_new(s)
    }
}

impl<S: Stringable> std::fmt::Display for Iuse<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let flag = &self.flag;
        match &self.default {
            Some(true) => write!(f, "+{flag}"),
            Some(false) => write!(f, "-{flag}"),
            None => write!(f, "{flag}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use indexmap::Equivalent;
    use itertools::Itertools;

    use crate::utils::hash;

    use super::*;

    #[test]
    fn new_and_parse() {
        // invalid
        for s in ["", "-", "+", "--u", "++u", "-+u", "+-u", "_u", "@u"] {
            assert!(Iuse::parse(s).is_err(), "{s} didn't fail");
            assert!(Iuse::try_new(s).is_err(), "{s} didn't fail");
        }

        // valid
        for s in ["u", "-u", "+u", "+u+", "u-", "0", "0+1-2", "u@u"] {
            let borrowed = Iuse::parse(s);
            let owned = Iuse::try_new(s);
            assert!(borrowed.is_ok(), "{s} failed");
            assert!(owned.is_ok(), "{s} failed");
            let borrowed = borrowed.unwrap();
            let owned = owned.unwrap();
            assert_eq!(borrowed, owned);
            assert_eq!(owned, borrowed);
            assert_eq!(owned, s.parse().unwrap());
            assert!(owned.equivalent(&borrowed));
            assert!(borrowed.equivalent(&owned));
            assert_eq!(borrowed.to_string(), s);
            assert_eq!(owned.to_string(), s);
        }
    }

    #[test]
    fn flag_and_default() {
        for (s, flag, default) in [
            ("u", "u", None),
            ("-u", "u", Some(false)),
            ("+u", "u", Some(true)),
            ("+u+", "u+", Some(true)),
        ] {
            let borrowed = Iuse::parse(s).unwrap();
            let owned = Iuse::try_new(s).unwrap();
            assert_eq!(owned.flag(), flag);
            assert_eq!(owned.default(), default);
            assert_eq!(borrowed.flag(), flag);
            assert_eq!(borrowed.default(), default);
        }
    }

    #[test]
    fn cmp() {
        let exprs = [
            // lexical flag order
            "u1 < u2",
            "z > a",
            "-a < -b",
            "+a < +b",
            "-1 > +0",
            // default order
            "-u1 < +u1",
        ];

        let op_map: HashMap<_, _> =
            [("<", Ordering::Less), ("==", Ordering::Equal), (">", Ordering::Greater)]
                .into_iter()
                .collect();

        for expr in exprs {
            let (s1, op, s2) = expr.split_whitespace().collect_tuple().unwrap();
            let v1_owned = Iuse::try_new(s1).unwrap();
            let v1_borrowed = Iuse::parse(s1).unwrap();
            let v2_owned = Iuse::try_new(s2).unwrap();
            let v2_borrowed = Iuse::parse(s2).unwrap();
            if op == "!=" {
                assert_ne!(v1_owned, v2_owned, "failed comparing: {expr}");
                assert_ne!(v1_borrowed, v2_borrowed, "failed comparing: {expr}");
                assert_ne!(v1_owned, v2_borrowed, "failed comparing: {expr}");
                assert_ne!(v1_borrowed, v2_owned, "failed comparing: {expr}");
                assert_ne!(v2_owned, v1_owned, "failed comparing: {expr}");
                assert_ne!(v2_borrowed, v1_borrowed, "failed comparing: {expr}");
                assert_ne!(v2_owned, v1_borrowed, "failed comparing: {expr}");
                assert_ne!(v2_borrowed, v1_owned, "failed comparing: {expr}");
            } else {
                let op = op_map[op];
                assert_eq!(v1_owned.cmp(&v2_owned), op, "failed comparing: {expr}");
                assert_eq!(v1_borrowed.cmp(&v2_borrowed), op, "failed comparing: {expr}");
                assert_eq!(
                    v1_owned.partial_cmp(&v2_borrowed),
                    Some(op),
                    "failed comparing: {expr}"
                );
                assert_eq!(
                    v1_borrowed.partial_cmp(&v2_owned),
                    Some(op),
                    "failed comparing: {expr}"
                );
                assert_eq!(
                    v2_owned.cmp(&v1_owned),
                    op.reverse(),
                    "failed comparing inverted: {expr}"
                );
                assert_eq!(
                    v2_borrowed.cmp(&v1_borrowed),
                    op.reverse(),
                    "failed comparing inverted: {expr}"
                );
                assert_eq!(
                    v2_owned.partial_cmp(&v1_borrowed),
                    Some(op.reverse()),
                    "failed comparing inverted: {expr}"
                );
                assert_eq!(
                    v2_borrowed.partial_cmp(&v1_owned),
                    Some(op.reverse()),
                    "failed comparing inverted: {expr}"
                );

                // verify the following property holds since both Hash and Eq are implemented:
                // k1 == k2 -> hash(k1) == hash(k2)
                if op == Ordering::Equal {
                    assert_eq!(hash(&v1_owned), hash(&v2_owned), "failed hash: {expr}");
                    assert_eq!(hash(&v1_borrowed), hash(&v2_borrowed), "failed hash: {expr}");
                    assert_eq!(hash(&v1_owned), hash(&v2_borrowed), "failed hash: {expr}");
                    assert_eq!(hash(&v1_borrowed), hash(&v2_owned), "failed hash: {expr}");
                }
            }
        }
    }
}
