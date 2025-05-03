use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

use scallop::ExecStatus;

use crate::shell::BuildData;

use super::BuildFn;
use super::phase::PhaseKind;

pub(crate) mod docompress;
pub(crate) mod dostrip;
pub(crate) mod eapply_user;

#[derive(Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Copy, Clone)]
pub(crate) enum HookKind {
    Pre,
    Post,
}

#[derive(Debug)]
pub(crate) struct HookBuilder {
    pub(crate) phase: PhaseKind,
    pub(crate) kind: HookKind,
    pub(crate) name: String,
    pub(crate) func: BuildFn,
    pub(crate) priority: usize,
}

impl From<HookBuilder> for Hook {
    fn from(value: HookBuilder) -> Self {
        Self {
            name: value.name,
            func: value.func,
            priority: value.priority,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Hook {
    name: String,
    func: BuildFn,
    priority: usize,
}

impl PartialEq for Hook {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Hook {}

impl Hash for Hook {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
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
    #[allow(dead_code)]
    pub(crate) fn run(&self, build: &mut BuildData) -> scallop::Result<ExecStatus> {
        (self.func)(build)
    }
}
