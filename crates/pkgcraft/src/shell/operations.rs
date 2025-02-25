use std::borrow::Borrow;
use std::hash::{Hash, Hasher};

use indexmap::IndexSet;
use strum::{AsRefStr, Display, EnumString};

use super::phase::PhaseKind;

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

impl OperationKind {
    /// Create an operation from an iterator of phases.
    pub(crate) fn phases<I>(self, phases: I) -> Operation
    where
        I: IntoIterator<Item = PhaseKind>,
    {
        Operation {
            kind: self,
            phases: phases.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Operation {
    kind: OperationKind,
    phases: IndexSet<PhaseKind>,
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

impl<'a> IntoIterator for &'a Operation {
    type Item = &'a PhaseKind;
    type IntoIter = indexmap::set::Iter<'a, PhaseKind>;

    fn into_iter(self) -> Self::IntoIter {
        self.phases.iter()
    }
}
