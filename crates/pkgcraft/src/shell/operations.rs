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
    /// Create an operation.
    pub(crate) fn op<I, P>(self, phases: I) -> Operation
    where
        I: IntoIterator<Item = P>,
        P: Into<Phase>,
    {
        Operation {
            kind: self,
            phases: phases.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Operation {
    kind: OperationKind,
    pub(crate) phases: IndexSet<Phase>,
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
