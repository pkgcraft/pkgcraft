use std::fmt;
use std::str::FromStr;

use serde_with::{DeserializeFromStr, SerializeDisplay};

use crate::Error;

use super::parse;

/// Unversioned package.
#[derive(SerializeDisplay, DeserializeFromStr, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct Cpn {
    pub(crate) category: String,
    pub(crate) package: String,
}

impl FromStr for Cpn {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Self::try_new(s)
    }
}

impl fmt::Debug for Cpn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Cpn {{ {self} }}")
    }
}

impl fmt::Display for Cpn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.category, self.package)
    }
}

impl Cpn {
    /// Create a [`Cpn`] from a given string.
    pub fn try_new<S: AsRef<str>>(s: S) -> crate::Result<Self> {
        parse::cpn(s.as_ref())
    }

    /// Return a Cpn's category.
    pub fn category(&self) -> &str {
        &self.category
    }

    /// Return a Cpn's package.
    pub fn package(&self) -> &str {
        &self.package
    }
}
