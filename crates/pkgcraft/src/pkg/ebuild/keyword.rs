use std::cmp::Ordering;

use crate::dep::{self, Stringable};
use crate::macros::{cmp_not_equal, equivalent};
use crate::traits::{IntoOwned, ToRef};

/// Package keyword type.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
pub enum Status {
    Disabled, // -arch
    Unstable, // ~arch
    Stable,   // arch
}

#[derive(Debug, Eq, Hash, Clone)]
pub struct Keyword<S: Stringable> {
    pub(crate) status: Status,
    pub(crate) arch: S,
}

impl IntoOwned for Keyword<&str> {
    type Owned = Keyword<String>;

    fn into_owned(self) -> Self::Owned {
        Keyword {
            status: self.status,
            arch: self.arch.to_string(),
        }
    }
}

impl<'a> ToRef<'a> for Keyword<String> {
    type Ref = Keyword<&'a str>;

    fn to_ref(&'a self) -> Self::Ref {
        Keyword {
            status: self.status,
            arch: self.arch.as_ref(),
        }
    }
}

impl Keyword<String> {
    /// Create an owned [`Keyword`] from a given string.
    pub fn try_new(s: &str) -> crate::Result<Self> {
        Keyword::parse(s).into_owned()
    }
}

impl<'a> Keyword<&'a str> {
    /// Create a borrowed [`Keyword`] from a given string.
    pub fn parse(s: &'a str) -> crate::Result<Self> {
        dep::parse::keyword(s)
    }
}

impl<S: Stringable> Keyword<S> {
    /// Return the architecture for a keyword without its status.
    pub fn arch(&self) -> &str {
        self.arch.as_ref()
    }

    /// Return the keyword status.
    pub fn status(&self) -> Status {
        self.status
    }

    /// Disable a keyword, returning its borrowed form.
    pub fn disable(&self) -> Keyword<&str> {
        Keyword {
            status: Status::Disabled,
            arch: self.arch(),
        }
    }

    /// Stabilize a keyword, returning its borrowed form while skipping disabled variants.
    pub fn stable(&self) -> Keyword<&str> {
        let status = if self.status != Status::Disabled {
            Status::Stable
        } else {
            Status::Disabled
        };

        Keyword {
            status,
            arch: self.arch(),
        }
    }

    /// Destabilize a keyword, returning its borrowed form while skipping disabled variants.
    pub fn unstable(&self) -> Keyword<&str> {
        let status = if self.status != Status::Disabled {
            Status::Unstable
        } else {
            Status::Disabled
        };

        Keyword {
            status,
            arch: self.arch(),
        }
    }
}

impl<S1: Stringable, S2: Stringable> PartialEq<Keyword<S1>> for Keyword<S2> {
    fn eq(&self, other: &Keyword<S1>) -> bool {
        self.status == other.status && self.arch() == other.arch()
    }
}

/// Compare two keywords, making unprefixed arches less than prefixed arches.
fn cmp<S1, S2>(k1: &Keyword<S1>, k2: &Keyword<S2>) -> Ordering
where
    S1: Stringable,
    S2: Stringable,
{
    let (arch1, arch2) = (k1.arch(), k2.arch());
    match (arch1.find('-'), arch2.find('-')) {
        (None, Some(_)) => return Ordering::Less,
        (Some(_), None) => return Ordering::Greater,
        _ => cmp_not_equal!(arch1, arch2),
    }

    k1.status.cmp(&k2.status)
}

impl<S: Stringable> Ord for Keyword<S> {
    fn cmp(&self, other: &Self) -> Ordering {
        cmp(self, other)
    }
}

impl<S1: Stringable, S2: Stringable> PartialOrd<Keyword<S1>> for Keyword<S2> {
    fn partial_cmp(&self, other: &Keyword<S1>) -> Option<Ordering> {
        Some(cmp(self, other))
    }
}

equivalent!(Keyword);

impl std::str::FromStr for Keyword<String> {
    type Err = crate::Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Self::try_new(s)
    }
}

impl<S: Stringable> std::fmt::Display for Keyword<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let arch = &self.arch;
        match &self.status {
            Status::Stable => write!(f, "{arch}"),
            Status::Unstable => write!(f, "~{arch}"),
            Status::Disabled => write!(f, "-{arch}"),
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
        for s in ["", "-", "-@", "--arch", "-~arch", "~-arch"] {
            assert!(Keyword::parse(s).is_err(), "{s} didn't fail");
            assert!(Keyword::try_new(s).is_err(), "{s} didn't fail");
        }

        // valid
        for s in ["arch", "-arch", "~arch", "-*", "_", "-_", "~_"] {
            let borrowed = Keyword::parse(s);
            let owned = Keyword::try_new(s);
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
    fn arch_and_status() {
        use Status::*;
        for (s, arch, status) in [
            ("arch", "arch", Stable),
            ("-arch", "arch", Disabled),
            ("~arch", "arch", Unstable),
            ("-*", "*", Disabled),
            ("~_", "_", Unstable),
            ("~arch-linux", "arch-linux", Unstable),
        ] {
            let borrowed = Keyword::parse(s).unwrap();
            let owned = Keyword::try_new(s).unwrap();
            assert_eq!(owned.arch(), arch);
            assert_eq!(owned.status(), status);
            assert_eq!(borrowed.arch(), arch);
            assert_eq!(borrowed.status(), status);
        }
    }

    #[test]
    fn alter_status() {
        let disabled = Keyword::parse("-arch").unwrap();
        let unstable = Keyword::parse("~arch").unwrap();
        let stable = Keyword::parse("arch").unwrap();

        assert_eq!(disabled.disable(), disabled);
        assert_eq!(unstable.disable(), disabled);
        assert_eq!(stable.disable(), disabled);

        // unstable() does not alter keywords with disabled status
        assert_eq!(disabled.unstable(), disabled);
        assert_eq!(unstable.unstable(), unstable);
        assert_eq!(stable.unstable(), unstable);

        // stable() does not alter keywords with disabled status
        assert_eq!(disabled.stable(), disabled);
        assert_eq!(unstable.stable(), stable);
        assert_eq!(stable.stable(), stable);
    }

    #[test]
    fn cmp() {
        let exprs = [
            // lexical arch order
            "arch1 < arch2",
            "arch-plat1 < arch-plat2",
            "-* < -arch",
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
            let v1_owned = Keyword::try_new(s1).unwrap();
            let v1_borrowed = Keyword::parse(s1).unwrap();
            let v2_owned = Keyword::try_new(s2).unwrap();
            let v2_borrowed = Keyword::parse(s2).unwrap();
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
