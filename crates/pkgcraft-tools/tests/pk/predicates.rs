use std::collections::HashSet;

use predicates::function::FnPredicate;
use predicates::prelude::*;

type FnPredStr = dyn Fn(&str) -> bool;

/// Verify a given iterable of lines completely contains a set of values.
pub(crate) fn lines_contain<I>(vals: I) -> FnPredicate<Box<FnPredStr>, str>
where
    I: IntoIterator,
    I::Item: std::fmt::Display,
{
    let vals: HashSet<_> = vals.into_iter().map(|s| s.to_string()).collect();
    let func = move |s: &str| -> bool {
        let mut seen = HashSet::new();
        for line in s.lines() {
            for val in vals.iter() {
                if line.contains(val) {
                    seen.insert(val.clone());
                    break;
                }
            }
        }
        seen == vals
    };

    predicate::function(Box::new(func))
}
