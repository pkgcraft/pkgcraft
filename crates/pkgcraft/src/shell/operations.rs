use std::borrow::Borrow;
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

use indexmap::IndexSet;
use strum::{AsRefStr, Display, EnumString};

use super::phase::Phase;

pub(crate) mod ebuild;

#[derive(AsRefStr, Display, EnumString, Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[strum(serialize_all = "snake_case")]
pub(crate) enum OperationKind {
    Pretend,
    Build,
    Install,
    Uninstall,
    Replace,
    Config,
    Info,
    NoFetch,
}

impl Ord for OperationKind {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_ref().cmp(other.as_ref())
    }
}

impl PartialOrd for OperationKind {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl OperationKind {
    /// Create an operation.
    pub(crate) fn op<P: IntoIterator<Item = Phase>>(self, phases: P) -> Operation {
        Operation {
            kind: self,
            phases: phases.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Operation {
    kind: OperationKind,
    pub(crate) phases: IndexSet<Phase>,
}

impl Ord for Operation {
    fn cmp(&self, other: &Self) -> Ordering {
        self.kind.cmp(&other.kind)
    }
}

impl PartialOrd for Operation {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Operation {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}

impl Eq for Operation {}

impl Hash for Operation {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.kind.hash(state);
    }
}

impl Borrow<OperationKind> for Operation {
    fn borrow(&self) -> &OperationKind {
        &self.kind
    }
}

pub(crate) struct Iter<'a>(indexmap::set::Iter<'a, Phase>);

impl<'a> IntoIterator for &'a Operation {
    type Item = &'a Phase;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter(self.phases.iter())
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a Phase;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}
