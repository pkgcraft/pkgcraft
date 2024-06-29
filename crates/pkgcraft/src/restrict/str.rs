use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::ops::Deref;

use crate::Error;

use super::boolean::*;
use super::{Restrict as BaseRestrict, Restriction};

#[derive(Clone, Debug)]
pub struct Regex(regex::Regex);

impl PartialEq for Regex {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_str() == other.0.as_str()
    }
}

impl Eq for Regex {}

impl Hash for Regex {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.as_str().hash(state);
    }
}

impl Deref for Regex {
    type Target = regex::Regex;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

restrict_with_boolean! {Restrict,
    Equal(String),
    Prefix(String),
    Regex(Regex),
    Substr(String),
    Suffix(String),
    Length(Vec<Ordering>, usize),
}

impl From<Restrict> for BaseRestrict {
    fn from(r: Restrict) -> Self {
        Self::Str(r)
    }
}

impl Restrict {
    restrict_impl_boolean! {Self}

    pub fn equal<S: Into<String>>(s: S) -> Self {
        Self::Equal(s.into())
    }

    pub fn prefix<S: Into<String>>(s: S) -> Self {
        Self::Prefix(s.into())
    }

    pub fn regex<S: AsRef<str>>(s: S) -> crate::Result<Self> {
        let re = regex::Regex::new(s.as_ref())
            .map_err(|e| Error::InvalidValue(format!("invalid regex: {e}")))?;
        Ok(Self::Regex(Regex(re)))
    }

    pub fn substr<S: Into<String>>(s: S) -> Self {
        Self::Substr(s.into())
    }

    pub fn suffix<S: Into<String>>(s: S) -> Self {
        Self::Suffix(s.into())
    }
}

impl Restriction<&str> for Restrict {
    fn matches(&self, val: &str) -> bool {
        restrict_match_boolean! {self, val,
            Self::Equal(s) => val == s,
            Self::Prefix(s) => val.starts_with(s),
            Self::Regex(re) => re.is_match(val),
            Self::Substr(s) => val.contains(s),
            Self::Suffix(s) => val.ends_with(s),
            Self::Length(ordering, size) => ordering.contains(&val.len().cmp(size)),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::hash;

    use super::*;

    #[test]
    fn variants() {
        // equal
        let r = Restrict::equal("a");
        assert!(r.matches("a"));
        assert!(!r.matches("b"));
        assert_eq!(r, r);
        assert!(hash(&r) != 0);

        // prefix
        let r = Restrict::prefix("ab");
        assert!(r.matches("ab"));
        assert!(r.matches("abc"));
        assert!(!r.matches("a"));
        assert!(!r.matches("cab"));
        assert_eq!(r, r);
        assert!(hash(&r) != 0);

        // regex
        let r = Restrict::regex("^(a|b)$").unwrap();
        assert!(r.matches("a"));
        assert!(r.matches("b"));
        assert!(!r.matches("ab"));
        assert_eq!(r, r);
        assert!(hash(&r) != 0);

        // substr
        let r = Restrict::substr("ab");
        assert!(r.matches("ab"));
        assert!(r.matches("cab"));
        assert!(r.matches("cabo"));
        assert!(!r.matches("acb"));
        assert_eq!(r, r);
        assert!(hash(&r) != 0);

        // suffix
        let r = Restrict::suffix("ab");
        assert!(r.matches("ab"));
        assert!(r.matches("cab"));
        assert!(!r.matches("a"));
        assert!(!r.matches("abc"));
        assert_eq!(r, r);
        assert!(hash(&r) != 0);
    }
}
