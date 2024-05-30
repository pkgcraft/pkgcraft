use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use crate::dep::parse;

/// Package keyword type.
#[repr(C)]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
pub enum KeywordStatus {
    Disabled, // -arch
    Unstable, // ~arch
    Stable,   // arch
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Keyword {
    pub(crate) status: KeywordStatus,
    pub(crate) arch: String,
}

impl Keyword {
    /// Create a [`Keyword`] from a given string.
    pub fn try_new(s: &str) -> crate::Result<Self> {
        parse::keyword(s)
    }

    /// Return the architecture for a keyword without its status.
    pub fn arch(&self) -> &str {
        &self.arch
    }

    /// Return the keyword status.
    pub fn status(&self) -> KeywordStatus {
        self.status
    }
}

/// Compare two arches, making unprefixed arches less than prefixed arches.
pub fn cmp_arches<S1, S2>(arch1: S1, arch2: S2) -> Ordering
where
    S1: AsRef<str>,
    S2: AsRef<str>,
{
    let (arch1, arch2) = (arch1.as_ref(), arch2.as_ref());
    match (arch1.split_once('-'), arch2.split_once('-')) {
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some((arch1, platform1)), Some((arch2, platform2))) => {
            platform1.cmp(platform2).then_with(|| arch1.cmp(arch2))
        }
        (None, None) => arch1.cmp(arch2),
    }
}

impl Ord for Keyword {
    fn cmp(&self, other: &Self) -> Ordering {
        let cmp = cmp_arches(self.arch(), other.arch());
        if cmp != Ordering::Equal {
            return cmp;
        }
        self.status.cmp(&other.status)
    }
}

impl PartialOrd for Keyword {
    fn partial_cmp(&self, other: &Keyword) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl FromStr for Keyword {
    type Err = crate::Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Self::try_new(s)
    }
}

impl fmt::Display for Keyword {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let arch = &self.arch;
        match &self.status {
            KeywordStatus::Stable => write!(f, "{arch}"),
            KeywordStatus::Unstable => write!(f, "~{arch}"),
            KeywordStatus::Disabled => write!(f, "-{arch}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use itertools::Itertools;

    use crate::utils::hash;

    use super::*;

    #[test]
    fn try_new() {
        // invalid
        for s in ["", "-", "-@", "--arch", "-~arch", "~-arch"] {
            assert!(Keyword::try_new(s).is_err(), "{s} didn't fail");
        }

        // valid
        for s in ["arch", "-arch", "~arch", "-*", "_", "-_", "~_"] {
            let kw = Keyword::try_new(s);
            assert!(kw.is_ok(), "{s} failed");
            let kw = kw.unwrap();
            assert_eq!(kw, s.parse().unwrap());
            assert_eq!(kw.to_string(), s);
        }
    }

    #[test]
    fn arch_and_status() {
        use KeywordStatus::*;
        for (s, arch, status) in [
            ("arch", "arch", Stable),
            ("-arch", "arch", Disabled),
            ("~arch", "arch", Unstable),
            ("-*", "*", Disabled),
            ("~_", "_", Unstable),
            ("~arch-linux", "arch-linux", Unstable),
        ] {
            let kw = Keyword::try_new(s).unwrap();
            assert_eq!(kw.arch(), arch);
            assert_eq!(kw.status(), status);
        }
    }

    #[test]
    fn cmp() {
        let exprs = [
            "arch == arch",
            "arch1 != arch2",
            // lexical arch order
            "arch1 < arch2",
            "arch-plat1 < arch-plat2",
            "-* < -arch",
            // platform higher priority than arch
            "arch1-plat1 < arch2-plat2",
            "arch2-plat1 < arch1-plat2",
            // status order
            "-arch < ~arch",
            "~arch < arch",
            "~arch < arch",
            // unprefixed vs prefixed
            "zarch < arch-linux",
        ];

        let op_map: HashMap<_, _> =
            [("<", Ordering::Less), ("==", Ordering::Equal), (">", Ordering::Greater)]
                .into_iter()
                .collect();

        for expr in exprs {
            let (s1, op, s2) = expr.split_whitespace().collect_tuple().unwrap();
            let kw1 = Keyword::try_new(s1).unwrap();
            let kw2 = Keyword::try_new(s2).unwrap();
            if op != "==" {
                assert_ne!(kw1, kw2, "failed comparing: {expr}");
                assert_ne!(kw2, kw1, "failed comparing: {expr}");
            }

            if op != "!=" {
                let op = op_map[op];
                assert_eq!(kw1.cmp(&kw2), op, "failed comparing: {expr}");
                assert_eq!(kw2.cmp(&kw1), op.reverse(), "failed comparing inverted: {expr}");

                // verify the following property holds since both Hash and Eq are implemented:
                // k1 == k2 -> hash(k1) == hash(k2)
                if op == Ordering::Equal {
                    assert_eq!(hash(&kw1), hash(&kw2), "failed hash: {expr}");
                }
            }
        }
    }
}
