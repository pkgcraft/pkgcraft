use std::fmt;
use std::str::FromStr;

use serde_with::{DeserializeFromStr, SerializeDisplay};

use crate::Error;

use super::parse;

/// Unversioned package.
#[derive(
    SerializeDisplay, DeserializeFromStr, PartialEq, Eq, PartialOrd, Ord, Clone, Hash,
)]
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

/// Try converting a (category, package) string tuple into a Cpn.
impl<T1, T2> TryFrom<(T1, T2)> for Cpn
where
    T1: AsRef<str>,
    T2: AsRef<str>,
{
    type Error = Error;

    fn try_from((category, package): (T1, T2)) -> Result<Self, Self::Error> {
        let category = parse::category(category.as_ref()).map(|s| s.to_string())?;
        let package = parse::package(package.as_ref()).map(|s| s.to_string())?;
        Ok(Self { category, package })
    }
}

impl From<&Cpn> for Cpn {
    fn from(value: &Cpn) -> Self {
        value.clone()
    }
}

impl fmt::Debug for Cpn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Cpn {{ {self} }}")
    }
}

impl fmt::Display for Cpn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.category, self.package)
    }
}

impl PartialEq<str> for Cpn {
    fn eq(&self, other: &str) -> bool {
        other
            .split_once('/')
            .map(|(cat, pkg)| self.category == cat && self.package == pkg)
            .unwrap_or_default()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        // invalid
        for s in ["", "a", "a/", "/b", "a/+b", "a/b-1"] {
            assert!(Cpn::try_new(s).is_err(), "{s} is valid");
        }

        // valid
        for s in ["_/_", "a/b"] {
            let cpn = Cpn::try_new(s);
            assert!(cpn.is_ok(), "{s} isn't valid");
            assert!(format!("{cpn:?}").contains(s));
        }
    }
}
