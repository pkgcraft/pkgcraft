use std::cmp::Ordering;

use scallop::builtins::ExecStatus;

use crate::macros::cmp_not_equal;
use crate::shell::BuildData;

use super::BuildFn;

pub(crate) mod eapi4;
pub(crate) mod eapi7;

#[derive(Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Copy, Clone)]
pub(crate) enum HookKind {
    Pre,
    Post,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub(crate) struct Hook {
    name: String,
    func: BuildFn,
    priority: usize,
    parallel: bool,
}

impl Ord for Hook {
    fn cmp(&self, other: &Self) -> Ordering {
        cmp_not_equal!(&self.priority, &other.priority);
        cmp_not_equal!(&self.name, &other.name);
        Ordering::Equal
    }
}

impl PartialOrd for Hook {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Hook {
    /// Create a new hook.
    pub(crate) fn new(name: &str, func: BuildFn, priority: usize, parallel: bool) -> Self {
        Self {
            name: name.to_string(),
            func,
            priority,
            parallel,
        }
    }

    pub(crate) fn run(&self, build: &mut BuildData) -> scallop::Result<ExecStatus> {
        (self.func)(build)
    }
}
