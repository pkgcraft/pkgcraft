use std::cmp::Ordering;

use crate::dep::{self, Stringable};
use crate::macros::{cmp_not_equal, equivalent};
use crate::traits::IntoOwned;

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
