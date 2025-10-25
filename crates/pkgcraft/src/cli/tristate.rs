use std::fmt::Display;
use std::hash::Hash;
use std::str::FromStr;

use indexmap::IndexSet;
use itertools::Itertools;

/// Tri-state value support for command-line arguments.
///
/// This supports arguments of the form: `set`, `+add`, and `-remove` that relate to their
/// matching variants.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub enum TriState<T> {
    Set(T),
    Add(T),
    Remove(T),
}

impl<T: Ord + Clone + Hash> TriState<T> {
    /// Modify the given, enabled set given an iterator of TriState values.
    pub fn enabled<I>(enabled: &mut IndexSet<T>, selected: I)
    where
        I: IntoIterator<Item = TriState<T>>,
    {
        // sort by variant
        let selected: Vec<_> = selected.into_iter().sorted().collect();

        // don't use default if neutral options exist
        if let Some(TriState::Set(_)) = selected.first() {
            std::mem::take(enabled);
        }

        for x in selected {
            match x {
                TriState::Set(value) => enabled.insert(value),
                TriState::Add(value) => enabled.insert(value),
                TriState::Remove(value) => enabled.swap_remove(&value),
            };
        }

        enabled.sort_unstable();
    }
}

impl<T: FromStr> FromStr for TriState<T> {
    type Err = T::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(value) = s.strip_prefix('+') {
            value.parse().map(Self::Add)
        } else if let Some(value) = s.strip_prefix('-') {
            value.parse().map(Self::Remove)
        } else {
            s.parse().map(Self::Set)
        }
    }
}

impl<T: Display> Display for TriState<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Add(v) => write!(f, "+{v}"),
            Self::Remove(v) => write!(f, "-{v}"),
            Self::Set(v) => write!(f, "{v}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test::*;

    use super::*;

    #[test]
    fn tri_state() {
        let empty: [i32; 0] = [];

        // empty
        let mut enabled = IndexSet::<i32>::new();
        let selected = IndexSet::new();
        TriState::enabled(&mut enabled, selected);
        assert_ordered_eq!(enabled, empty);

        // no selections
        let mut enabled: IndexSet<i32> = [1].into_iter().collect();
        let selected = IndexSet::new();
        TriState::enabled(&mut enabled, selected);
        assert_ordered_eq!(enabled, [1]);

        // override defaults
        let mut enabled: IndexSet<i32> = [1].into_iter().collect();
        let selected: IndexSet<_> = ["2"].iter().map(|s| s.parse()).try_collect().unwrap();
        TriState::enabled(&mut enabled, selected);
        assert_ordered_eq!(enabled, [2]);

        // negated selection
        let mut enabled: IndexSet<i32> = [1].into_iter().collect();
        let selected: IndexSet<_> =
            ["2", "-2"].iter().map(|s| s.parse()).try_collect().unwrap();
        TriState::enabled(&mut enabled, selected);
        assert_ordered_eq!(enabled, empty);

        // add to defaults
        let mut enabled: IndexSet<_> = [1].into_iter().collect();
        let selected: IndexSet<_> = ["+2"].iter().map(|s| s.parse()).try_collect().unwrap();
        TriState::enabled(&mut enabled, selected);
        assert_ordered_eq!(enabled, [1, 2]);
    }
}
