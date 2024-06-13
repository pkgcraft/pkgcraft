use std::borrow::Borrow;
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

impl OperationKind {
    /// Create an operation from an iterator of phases.
    pub(crate) fn phases<I>(self, phases: I) -> Operation
    where
        I: IntoIterator,
        I::Item: Into<Phase>,
    {
        Operation {
            kind: self,
            phases: phases.into_iter().map(Into::into).collect(),
        }
    }

    /// Create an operation from a single phase.
    pub(crate) fn phase<P: Into<Phase>>(self, phase: P) -> Operation {
        Operation {
            kind: self,
            phases: IndexSet::from([phase.into()]),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Operation {
    kind: OperationKind,
    pub(crate) phases: IndexSet<Phase>,
}

impl Operation {
    /// Append a phase to an operation.
    pub(crate) fn phase<P: Into<Phase>>(mut self, phase: P) -> Self {
        self.phases.insert(phase.into());
        self
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

impl<'a> IntoIterator for &'a Operation {
    type Item = &'a Phase;
    type IntoIter = indexmap::set::Iter<'a, Phase>;

    fn into_iter(self) -> Self::IntoIter {
        self.phases.iter()
    }
}
