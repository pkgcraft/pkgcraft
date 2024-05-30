use std::cmp::Ordering;

use scallop::ExecStatus;

use crate::shell::BuildData;

use super::phase::PhaseKind;
use super::BuildFn;

pub(crate) mod docompress;
pub(crate) mod dostrip;

#[derive(Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Copy, Clone)]
pub(crate) enum HookKind {
    Pre,
    Post,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub(crate) struct Hook {
    pub(crate) phase: PhaseKind,
    pub(crate) kind: HookKind,
    pub(crate) name: String,
    pub(crate) func: BuildFn,
    pub(crate) priority: usize,
    pub(crate) parallel: bool,
}

impl Ord for Hook {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority
            .cmp(&other.priority)
            .then_with(|| self.name.cmp(&other.name))
    }
}

impl PartialOrd for Hook {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Hook {
    pub(crate) fn run(&self, build: &mut BuildData) -> scallop::Result<ExecStatus> {
        (self.func)(build)
    }
}
