use std::borrow::Borrow;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

use indexmap::IndexSet;
use strum::{AsRefStr, Display, EnumIter, EnumString};

use super::scope::{Scope, Scopes};

#[derive(AsRefStr, Display, EnumIter, EnumString, Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[strum(serialize_all = "UPPERCASE")]
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
pub(crate) enum VariableKind {
    // package specific
    CATEGORY,
    P,
    PF,
    PN,
    PR,
    PV,
    PVR,

    // environment specific
    A,
    AA,
    FILESDIR,
    DISTDIR,
    WORKDIR,
    S,
    PORTDIR,
    ECLASSDIR,
    ROOT,
    EROOT,
    SYSROOT,
    ESYSROOT,
    BROOT,
    T,
    TMPDIR,
    HOME,
    EPREFIX,
    D,
    ED,
    DESTTREE,
    INSDESTTREE,
    USE,
    EBUILD_PHASE,
    EBUILD_PHASE_FUNC,
    KV,
    MERGE_TYPE,
    REPLACING_VERSIONS,
    REPLACED_BY_VERSION,
}

impl Ord for VariableKind {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_ref().cmp(other.as_ref())
    }
}

impl PartialOrd for VariableKind {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl VariableKind {
    pub(crate) fn scopes<I: IntoIterator<Item = Scopes>>(self, scopes: I) -> Variable {
        let mut scopes: IndexSet<_> = scopes.into_iter().flatten().collect();
        scopes.sort();
        Variable { kind: self, scopes }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Variable {
    kind: VariableKind,
    scopes: IndexSet<Scope>,
}

impl Ord for Variable {
    fn cmp(&self, other: &Self) -> Ordering {
        self.kind.cmp(&other.kind)
    }
}

impl PartialOrd for Variable {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Variable {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}

impl Eq for Variable {}

impl Hash for Variable {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.kind.hash(state);
    }
}

impl fmt::Display for Variable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.kind)
    }
}

impl Borrow<VariableKind> for Variable {
    fn borrow(&self) -> &VariableKind {
        &self.kind
    }
}

impl AsRef<str> for Variable {
    fn as_ref(&self) -> &str {
        self.kind.as_ref()
    }
}

impl From<&Variable> for VariableKind {
    fn from(value: &Variable) -> Self {
        value.kind
    }
}

impl Variable {
    pub(crate) fn scopes(&self) -> &IndexSet<Scope> {
        &self.scopes
    }
}
