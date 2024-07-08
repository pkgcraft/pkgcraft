use std::fmt;
use std::str::FromStr;

use indexmap::IndexSet;
use serde_with::{DeserializeFromStr, SerializeDisplay};

use crate::types::SortedSet;
use crate::Error;

use super::{parse, Stringable};

/// Package USE dependency type.
#[repr(C)]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
pub enum UseDepKind {
    Enabled,     // cat/pkg[u] and cat/pkg[-u]
    Equal,       // cat/pkg[u=] and cat/pkg[-u=]
    Conditional, // cat/pkg[u?] and cat/pkg[!u?]
}

/// Package USE dependency.
#[derive(
    DeserializeFromStr, SerializeDisplay, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone,
)]
pub struct UseDep {
    pub(crate) flag: String,
    pub(crate) kind: UseDepKind,
    pub(crate) enabled: bool,
    pub(crate) default: Option<bool>,
}

impl fmt::Display for UseDep {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let default = match &self.default {
            Some(true) => "(+)",
            Some(false) => "(-)",
            None => "",
        };

        let flag = &self.flag;
        match (&self.kind, &self.enabled) {
            (UseDepKind::Enabled, true) => write!(f, "{flag}{default}"),
            (UseDepKind::Enabled, false) => write!(f, "-{flag}{default}"),
            (UseDepKind::Equal, true) => write!(f, "{flag}{default}="),
            (UseDepKind::Equal, false) => write!(f, "!{flag}{default}="),
            (UseDepKind::Conditional, true) => write!(f, "{flag}{default}?"),
            (UseDepKind::Conditional, false) => write!(f, "!{flag}{default}?"),
        }
    }
}

impl FromStr for UseDep {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Self::try_new(s)
    }
}

impl FromStr for SortedSet<UseDep> {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        s.split(',').map(UseDep::try_new).collect()
    }
}

impl UseDep {
    /// Create a new UseDep from a given string.
    pub fn try_new(s: &str) -> crate::Result<Self> {
        parse::use_dep(s)
    }

    /// Return the USE dependency type.
    pub fn kind(&self) -> UseDepKind {
        self.kind
    }

    /// Return the flag value for the USE dependency.
    pub fn flag(&self) -> &str {
        &self.flag
    }

    /// Return true if the USE dependency may or must be enabled, otherwise false.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Return the USE dependency default.
    pub fn default(&self) -> Option<bool> {
        self.default
    }

    /// Determine if a USE dependency matches a set of enabled flags.
    pub(crate) fn matches<S: Stringable>(&self, options: &IndexSet<S>) -> bool {
        if self.kind == UseDepKind::Conditional {
            !(self.enabled ^ options.contains(self.flag()))
        } else {
            todo!()
        }
    }
}
