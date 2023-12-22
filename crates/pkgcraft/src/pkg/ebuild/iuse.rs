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
