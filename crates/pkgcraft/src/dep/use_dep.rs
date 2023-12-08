use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use indexmap::IndexSet;
use serde_with::{DeserializeFromStr, SerializeDisplay};

use crate::traits::IntoOwned;
use crate::types::SortedSet;
use crate::Error;

use super::{parse, Enabled, UseFlag};

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
#[derive(DeserializeFromStr, SerializeDisplay, Debug, PartialEq, Eq, Hash, Clone)]
pub struct UseDep<S: UseFlag> {
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

impl<S: UseFlag> Ord for UseDep<S> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.flag.cmp(&other.flag)
    }
}

impl<S: UseFlag> PartialOrd for UseDep<S> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<S: UseFlag> fmt::Display for UseDep<S> {
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
        UseDep::new(s)
    }
}

impl FromStr for SortedSet<UseDep<String>> {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        s.split(',').map(UseDep::new).collect()
    }
}

impl<S: UseFlag> UseDep<S> {
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

    /// Return the UseDep using internal references.
    pub(crate) fn as_ref(&self) -> UseDep<&str> {
        UseDep {
            kind: self.kind,
            flag: &self.flag,
            default: self.default,
        }
    }
}
