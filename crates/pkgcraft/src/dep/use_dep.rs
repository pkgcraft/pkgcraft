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
    Enabled(bool),     // cat/pkg[u] and cat/pkg[-u]
    Equal(bool),       // cat/pkg[u=] and cat/pkg[-u=]
    Conditional(bool), // cat/pkg[u?] and cat/pkg[!u?]
}

/// Package USE dependency default when missing.
#[repr(C)]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
pub enum UseDepDefault {
    Disabled, // cat/pkg[opt(-)]
    Enabled,  // cat/pkg[opt(+)]
}

/// Package USE dependency.
#[derive(
    DeserializeFromStr, SerializeDisplay, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone,
)]
pub struct UseDep {
    pub(crate) flag: String,
    pub(crate) kind: UseDepKind,
    pub(crate) default: Option<UseDepDefault>,
}

impl fmt::Display for UseDep {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let default = match &self.default {
            Some(UseDepDefault::Enabled) => "(+)",
            Some(UseDepDefault::Disabled) => "(-)",
            None => "",
        };

        let flag = &self.flag;
        match &self.kind {
            UseDepKind::Enabled(true) => write!(f, "{flag}{default}"),
            UseDepKind::Enabled(false) => write!(f, "-{flag}{default}"),
            UseDepKind::Equal(true) => write!(f, "{flag}{default}="),
            UseDepKind::Equal(false) => write!(f, "!{flag}{default}="),
            UseDepKind::Conditional(true) => write!(f, "{flag}{default}?"),
            UseDepKind::Conditional(false) => write!(f, "!{flag}{default}?"),
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
    pub fn kind(&self) -> &UseDepKind {
        &self.kind
    }

    /// Return true if the USE dependency may or must be enabled, otherwise false.
    pub fn possible(&self) -> bool {
        match self.kind {
            UseDepKind::Enabled(val) => val,
            UseDepKind::Equal(val) => val,
            UseDepKind::Conditional(val) => val,
        }
    }

    /// Return the flag value for the USE dependency.
    pub fn flag(&self) -> &str {
        &self.flag
    }

    /// Return the USE dependency default.
    pub fn default(&self) -> Option<UseDepDefault> {
        self.default
    }

    /// Determine if a USE dependency matches a set of enabled flags.
    pub(crate) fn matches<S: Stringable>(&self, options: &IndexSet<S>) -> bool {
        let flag = self.flag.as_str();
        match &self.kind {
            UseDepKind::Conditional(val) => !(val ^ options.contains(flag)),
            _ => todo!(),
        }
    }
}
