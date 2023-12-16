use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use indexmap::IndexSet;
use serde_with::{DeserializeFromStr, SerializeDisplay};

use crate::traits::{IntoOwned, ToRef};
use crate::types::SortedSet;
use crate::Error;

use super::{parse, Enabled, Stringable};

/// Package USE dependency type.
#[repr(C)]
#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub enum UseDepKind {
    Enabled,             // cat/pkg[opt]
    Disabled,            // cat/pkg[-opt]
    Equal,               // cat/pkg[opt=]
    NotEqual,            // cat/pkg[!opt=]
    EnabledConditional,  // cat/pkg[opt?]
    DisabledConditional, // cat/pkg[!opt?]
}

/// Package USE dependency default when missing.
#[repr(C)]
#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub enum UseDepDefault {
    Enabled,  // cat/pkg[opt(+)]
    Disabled, // cat/pkg[opt(-)]
}

/// Package USE dependency.
#[derive(DeserializeFromStr, SerializeDisplay, Debug, Eq, Hash, Clone)]
pub struct UseDep<S: Stringable> {
    pub(crate) kind: UseDepKind,
    pub(crate) flag: S,
    pub(crate) default: Option<UseDepDefault>,
}

impl IntoOwned for UseDep<&str> {
    type Owned = UseDep<String>;

    fn into_owned(self) -> Self::Owned {
        UseDep {
            kind: self.kind,
            flag: self.flag.to_string(),
            default: self.default,
        }
    }
}

impl<'a, S: Stringable> ToRef<'a> for UseDep<S> {
    type Ref = UseDep<&'a str>;

    fn to_ref(&'a self) -> Self::Ref {
        UseDep {
            kind: self.kind,
            flag: self.flag.as_ref(),
            default: self.default,
        }
    }
}

impl<S1: Stringable, S2: Stringable> PartialEq<UseDep<S1>> for UseDep<S2> {
    fn eq(&self, other: &UseDep<S1>) -> bool {
        self.kind == other.kind && self.flag() == other.flag() && self.default == other.default
    }
}

impl<S: Stringable> Ord for UseDep<S> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.flag.cmp(&other.flag)
    }
}

impl<S1: Stringable, S2: Stringable> PartialOrd<UseDep<S1>> for UseDep<S2> {
    fn partial_cmp(&self, other: &UseDep<S1>) -> Option<Ordering> {
        Some(self.flag().cmp(other.flag()))
    }
}

impl<S: Stringable> fmt::Display for UseDep<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let default = match &self.default {
            Some(UseDepDefault::Enabled) => "(+)",
            Some(UseDepDefault::Disabled) => "(-)",
            None => "",
        };

        let flag = self.flag();
        match &self.kind {
            UseDepKind::Enabled => write!(f, "{flag}{default}"),
            UseDepKind::Disabled => write!(f, "-{flag}{default}"),
            UseDepKind::Equal => write!(f, "{flag}{default}="),
            UseDepKind::NotEqual => write!(f, "!{flag}{default}="),
            UseDepKind::EnabledConditional => write!(f, "{flag}{default}?"),
            UseDepKind::DisabledConditional => write!(f, "!{flag}{default}?"),
        }
    }
}

impl FromStr for UseDep<String> {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Self::new(s)
    }
}

impl FromStr for SortedSet<UseDep<String>> {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        s.split(',').map(UseDep::new).collect()
    }
}

impl<S: Stringable> UseDep<S> {
    /// Return the USE dependency type.
    pub fn kind(&self) -> UseDepKind {
        self.kind
    }

    /// Return the flag value for the USE dependency.
    pub fn flag(&self) -> &str {
        self.flag.as_ref()
    }

    /// Return the USE dependency default.
    pub fn default(&self) -> Option<UseDepDefault> {
        self.default
    }

    /// Determine if a USE dependency matches a set of enabled flags.
    pub(crate) fn matches<F: Enabled>(&self, options: &IndexSet<F>) -> bool {
        use UseDepKind::*;
        match &self.kind {
            EnabledConditional => options.contains(self.flag()),
            DisabledConditional => !options.contains(self.flag()),
            _ => todo!(),
        }
    }
}

impl UseDep<String> {
    /// Create a new UseDep from a given string.
    pub fn new(s: &str) -> crate::Result<Self> {
        parse::use_dep(s).into_owned()
    }
}
