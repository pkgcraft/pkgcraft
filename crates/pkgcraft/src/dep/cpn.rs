use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use serde_with::{DeserializeFromStr, SerializeDisplay};

use crate::macros::{equivalent, partial_cmp_not_equal_opt};
use crate::traits::{IntoOwned, ToRef};
use crate::Error;

use super::{parse, Stringable};

/// Unversioned package.
#[derive(SerializeDisplay, DeserializeFromStr, Debug, Eq, Ord, Clone, Hash)]
pub struct Cpn<S: Stringable> {
    pub(crate) category: S,
    pub(crate) package: S,
}

impl<'a> IntoOwned for Cpn<&'a str> {
    type Owned = Cpn<String>;

    fn into_owned(self) -> Self::Owned {
        Cpn {
            category: self.category.to_string(),
            package: self.package.to_string(),
        }
    }
}

impl<'a> ToRef<'a> for Cpn<String> {
    type Ref = Cpn<&'a str>;

    fn to_ref(&'a self) -> Self::Ref {
        Cpn {
            category: self.category.as_ref(),
            package: self.package.as_ref(),
        }
    }
}

impl FromStr for Cpn<String> {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Self::try_new(s)
    }
}

impl<S1: Stringable, S2: Stringable> PartialEq<Cpn<S1>> for Cpn<S2> {
    fn eq(&self, other: &Cpn<S1>) -> bool {
        self.category() == other.category() && self.package() == other.package()
    }
}

impl<S1: Stringable, S2: Stringable> PartialOrd<Cpn<S1>> for Cpn<S2> {
    fn partial_cmp(&self, other: &Cpn<S1>) -> Option<Ordering> {
        partial_cmp_not_equal_opt!(self.category(), other.category());
        self.package().partial_cmp(other.package())
    }
}

impl<S: Stringable> fmt::Display for Cpn<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.category, self.package)
    }
}

impl Cpn<String> {
    /// Create an owned [`Cpn`] from a given string.
    pub fn try_new<S: AsRef<str>>(s: S) -> crate::Result<Self> {
        Cpn::parse(s.as_ref()).into_owned()
    }
}

impl<'a> Cpn<&'a str> {
    /// Create a borrowed [`Cpn`] from a given string.
    pub fn parse(s: &'a str) -> crate::Result<Self> {
        parse::cpn(s)
    }
}

impl<S: Stringable> Cpn<S> {
    /// Return a Cpn's category.
    pub fn category(&self) -> &str {
        self.category.as_ref()
    }

    /// Return a Cpn's package.
    pub fn package(&self) -> &str {
        self.package.as_ref()
    }
}

equivalent!(Cpn);
