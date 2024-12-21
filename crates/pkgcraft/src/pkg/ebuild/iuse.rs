use std::fmt;
use std::str::FromStr;

use crate::dep::parse;

/// Package IUSE.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct Iuse {
    pub(crate) flag: String,
    pub(crate) default: Option<bool>,
}

impl Iuse {
    /// Create an [`Iuse`] from a given string.
    pub fn try_new(s: &str) -> crate::Result<Self> {
        parse::iuse(s)
    }

    /// Return the USE flag.
    pub fn flag(&self) -> &str {
        &self.flag
    }

    /// Return true if the USE flag is enabled by default.
    pub fn is_enabled(&self) -> bool {
        self.default.unwrap_or(false)
    }

    /// Return true if the USE flag is disabled by default.
    pub fn is_disabled(&self) -> bool {
        !self.default.unwrap_or(true)
    }

    /// Return the default status, if it exists.
    pub fn default(&self) -> Option<bool> {
        self.default
    }
}

impl FromStr for Iuse {
    type Err = crate::Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Self::try_new(s)
    }
}

impl fmt::Display for Iuse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let flag = &self.flag;
        match &self.default {
            Some(true) => write!(f, "+{flag}"),
            Some(false) => write!(f, "-{flag}"),
            None => write!(f, "{flag}"),
        }
    }
}

impl fmt::Debug for Iuse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Iuse {{ {self} }}")
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;
    use std::collections::HashMap;

    use itertools::Itertools;

    use crate::utils::hash;

    use super::*;

    #[test]
    fn new_and_parse() {
        // invalid
        for s in ["", "-", "+", "--u", "++u", "-+u", "+-u", "_u", "@u"] {
            assert!(Iuse::try_new(s).is_err(), "{s} didn't fail");
        }

        // valid
        for s in ["u", "-u", "+u", "+u+", "u-", "0", "0+1-2", "u@u"] {
            let iuse = Iuse::try_new(s);
            assert!(iuse.is_ok(), "{s} failed");
            let iuse = iuse.unwrap();
            assert_eq!(iuse, s.parse().unwrap());
            assert_eq!(iuse.to_string(), s);
            assert!(format!("{iuse:?}").contains(s));
        }
    }

    #[test]
    fn flag_and_default() {
        for (s, flag, default, disabled, enabled) in [
            ("u", "u", None, false, false),
            ("-u", "u", Some(false), true, false),
            ("+u", "u", Some(true), false, true),
            ("+u+", "u+", Some(true), false, true),
        ] {
            let iuse = Iuse::try_new(s).unwrap();
            assert_eq!(iuse.flag(), flag);
            assert_eq!(iuse.default(), default);
            assert_eq!(iuse.is_disabled(), disabled);
            assert_eq!(iuse.is_enabled(), enabled);
        }
    }

    #[test]
    fn cmp() {
        #[rustfmt::skip]
        let exprs = [
            // equality
            "u == u",
            "u != -u",
            "u != +u",
            "-u != +u",
            "u1 != u2",
            // lexical flag order
            "u1 < u2",
            "z > a",
            "-a < -b",
            "+a < +b",
            "-1 > +0",
            // default order
            "u < -u",
            "-u < +u",
        ];

        let op_map: HashMap<_, _> =
            [("<", Ordering::Less), ("==", Ordering::Equal), (">", Ordering::Greater)]
                .into_iter()
                .collect();

        for expr in exprs {
            let (s1, op, s2) = expr.split_whitespace().collect_tuple().unwrap();
            let iuse1 = Iuse::try_new(s1).unwrap();
            let iuse2 = Iuse::try_new(s2).unwrap();
            if op != "==" {
                assert_ne!(iuse1, iuse2, "failed comparing: {expr}");
                assert_ne!(iuse2, iuse1, "failed comparing: {expr}");
            }

            if op != "!=" {
                let op = op_map[op];
                assert_eq!(iuse1.cmp(&iuse2), op, "failed comparing: {expr}");
                assert_eq!(iuse2.cmp(&iuse1), op.reverse(), "failed comparing inverted: {expr}");

                // verify the following property holds since both Hash and Eq are implemented:
                // k1 == k2 -> hash(k1) == hash(k2)
                if op == Ordering::Equal {
                    assert_eq!(hash(&iuse1), hash(&iuse2), "failed hash: {expr}");
                }
            }
        }
    }
}
