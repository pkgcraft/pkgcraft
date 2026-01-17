use crate::types::{Deque, Ordered};

use super::*;

#[derive(Debug)]
pub struct Iter<'a, T: Ordered>(Deque<&'a Dependency<T>>);

impl<'a, T: Ordered> FromIterator<&'a Dependency<T>> for Iter<'a, T> {
    fn from_iter<I: IntoIterator<Item = &'a Dependency<T>>>(iterable: I) -> Self {
        Self(iterable.into_iter().collect())
    }
}

impl<'a, T: Ordered> Iterator for Iter<'a, T> {
    type Item = &'a Dependency<T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.pop_front()
    }
}

impl<T: Ordered> DoubleEndedIterator for Iter<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.pop_back()
    }
}

#[derive(Debug)]
pub struct IntoIter<T: Ordered>(Deque<Dependency<T>>);

impl<T: Ordered> FromIterator<Dependency<T>> for IntoIter<T> {
    fn from_iter<I: IntoIterator<Item = Dependency<T>>>(iterable: I) -> Self {
        Self(iterable.into_iter().collect())
    }
}

impl<T: Ordered> Iterator for IntoIter<T> {
    type Item = Dependency<T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.pop_front()
    }
}

impl<T: Ordered> DoubleEndedIterator for IntoIter<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.pop_back()
    }
}

#[derive(Debug)]
pub struct IterFlatten<'a, T: Ordered>(Deque<&'a Dependency<T>>);

impl<'a, T: Ordered> FromIterator<&'a Dependency<T>> for IterFlatten<'a, T> {
    fn from_iter<I: IntoIterator<Item = &'a Dependency<T>>>(iterable: I) -> Self {
        Self(iterable.into_iter().collect())
    }
}

impl<'a, T: Ordered> Iterator for IterFlatten<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        while let Some(dep) = self.0.pop_front() {
            match dep {
                Enabled(val) | Disabled(val) => return Some(val),
                AllOf(vals) => self.0.extend_left(vals),
                AnyOf(vals) => self.0.extend_left(vals),
                ExactlyOneOf(vals) => self.0.extend_left(vals),
                AtMostOneOf(vals) => self.0.extend_left(vals),
                Conditional(_, vals) => self.0.extend_left(vals),
            }
        }
        None
    }
}

impl<T: Ordered> DoubleEndedIterator for IterFlatten<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        while let Some(dep) = self.0.pop_back() {
            match dep {
                Enabled(val) | Disabled(val) => return Some(val),
                AllOf(vals) => self.0.extend(vals),
                AnyOf(vals) => self.0.extend(vals),
                ExactlyOneOf(vals) => self.0.extend(vals),
                AtMostOneOf(vals) => self.0.extend(vals),
                Conditional(_, vals) => self.0.extend(vals),
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct IntoIterFlatten<T: Ordered>(Deque<Dependency<T>>);

impl<T: Ordered> FromIterator<Dependency<T>> for IntoIterFlatten<T> {
    fn from_iter<I: IntoIterator<Item = Dependency<T>>>(iterable: I) -> Self {
        Self(iterable.into_iter().collect())
    }
}

impl<T: Ordered> Iterator for IntoIterFlatten<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        while let Some(dep) = self.0.pop_front() {
            match dep {
                Enabled(val) | Disabled(val) => return Some(val),
                AllOf(vals) => self.0.extend_left(vals),
                AnyOf(vals) => self.0.extend_left(vals),
                ExactlyOneOf(vals) => self.0.extend_left(vals),
                AtMostOneOf(vals) => self.0.extend_left(vals),
                Conditional(_, vals) => self.0.extend_left(vals),
            }
        }
        None
    }
}

impl<T: Ordered> DoubleEndedIterator for IntoIterFlatten<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        while let Some(dep) = self.0.pop_back() {
            match dep {
                Enabled(val) | Disabled(val) => return Some(val),
                AllOf(vals) => self.0.extend(vals),
                AnyOf(vals) => self.0.extend(vals),
                ExactlyOneOf(vals) => self.0.extend(vals),
                AtMostOneOf(vals) => self.0.extend(vals),
                Conditional(_, vals) => self.0.extend(vals),
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct IterConditionalFlatten<'a, T: Ordered>(Deque<(Vec<&'a UseDep>, &'a Dependency<T>)>);

impl<'a, T: Ordered> FromIterator<&'a Dependency<T>> for IterConditionalFlatten<'a, T> {
    fn from_iter<I: IntoIterator<Item = &'a Dependency<T>>>(iterable: I) -> Self {
        Self(
            iterable
                .into_iter()
                .map(|d| (Default::default(), d))
                .collect(),
        )
    }
}

impl<'a, T: Ordered> Iterator for IterConditionalFlatten<'a, T> {
    type Item = (Vec<&'a UseDep>, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        while let Some((mut use_deps, dep)) = self.0.pop_front() {
            match dep {
                Enabled(val) | Disabled(val) => return Some((use_deps, val)),
                AllOf(vals) => self
                    .0
                    .extend_left(vals.iter().map(|d| (use_deps.clone(), d))),
                AnyOf(vals) => self
                    .0
                    .extend_left(vals.iter().map(|d| (use_deps.clone(), d))),
                ExactlyOneOf(vals) => self
                    .0
                    .extend_left(vals.iter().map(|d| (use_deps.clone(), d))),
                AtMostOneOf(vals) => self
                    .0
                    .extend_left(vals.iter().map(|d| (use_deps.clone(), d))),
                Conditional(u, vals) => {
                    use_deps.push(u);
                    self.0
                        .extend_left(vals.iter().map(|d| (use_deps.clone(), d)));
                }
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct IntoIterConditionalFlatten<T: Ordered>(Deque<(Vec<UseDep>, Dependency<T>)>);

impl<T: Ordered> FromIterator<Dependency<T>> for IntoIterConditionalFlatten<T> {
    fn from_iter<I: IntoIterator<Item = Dependency<T>>>(iterable: I) -> Self {
        Self(
            iterable
                .into_iter()
                .map(|d| (Default::default(), d))
                .collect(),
        )
    }
}

impl<T: Ordered> Iterator for IntoIterConditionalFlatten<T> {
    type Item = (Vec<UseDep>, T);

    fn next(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        while let Some((mut use_deps, dep)) = self.0.pop_front() {
            match dep {
                Enabled(val) | Disabled(val) => return Some((use_deps, val)),
                AllOf(vals) => self
                    .0
                    .extend_left(vals.into_iter().map(|x| (use_deps.clone(), x))),
                AnyOf(vals) => self
                    .0
                    .extend_left(vals.into_iter().map(|x| (use_deps.clone(), x))),
                ExactlyOneOf(vals) => self
                    .0
                    .extend_left(vals.into_iter().map(|x| (use_deps.clone(), x))),
                AtMostOneOf(vals) => self
                    .0
                    .extend_left(vals.into_iter().map(|x| (use_deps.clone(), x))),
                Conditional(u, vals) => {
                    use_deps.push(u);
                    self.0
                        .extend_left(vals.into_iter().map(|x| (use_deps.clone(), x)));
                }
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct IterRecursive<'a, T: Ordered>(Deque<&'a Dependency<T>>);

impl<'a, T: Ordered> FromIterator<&'a Dependency<T>> for IterRecursive<'a, T> {
    fn from_iter<I: IntoIterator<Item = &'a Dependency<T>>>(iterable: I) -> Self {
        Self(iterable.into_iter().collect())
    }
}

impl<'a, T: Ordered> Iterator for IterRecursive<'a, T> {
    type Item = &'a Dependency<T>;

    fn next(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        let val = self.0.pop_front();
        if let Some(dep) = val {
            match dep {
                Enabled(_) | Disabled(_) => (),
                AllOf(vals) => self.0.extend_left(vals),
                AnyOf(vals) => self.0.extend_left(vals),
                ExactlyOneOf(vals) => self.0.extend_left(vals),
                AtMostOneOf(vals) => self.0.extend_left(vals),
                Conditional(_, vals) => self.0.extend_left(vals),
            }
        }

        val
    }
}

#[derive(Debug)]
pub struct IntoIterRecursive<T: Ordered>(Deque<Dependency<T>>);

impl<T: Ordered> FromIterator<Dependency<T>> for IntoIterRecursive<T> {
    fn from_iter<I: IntoIterator<Item = Dependency<T>>>(iterable: I) -> Self {
        Self(iterable.into_iter().collect())
    }
}

impl<T: Ordered> Iterator for IntoIterRecursive<T> {
    type Item = Dependency<T>;

    fn next(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        let val = self.0.pop_front();
        if let Some(dep) = &val {
            match dep {
                Enabled(_) | Disabled(_) => (),
                AllOf(vals) => self.0.extend_left(vals.clone()),
                AnyOf(vals) => self.0.extend_left(vals.clone()),
                ExactlyOneOf(vals) => self.0.extend_left(vals.clone()),
                AtMostOneOf(vals) => self.0.extend_left(vals.clone()),
                Conditional(_, vals) => self.0.extend_left(vals.clone()),
            }
        }

        val
    }
}

#[derive(Debug)]
pub struct IterConditionals<'a, T: Ordered>(Deque<&'a Dependency<T>>);

impl<'a, T: Ordered> FromIterator<&'a Dependency<T>> for IterConditionals<'a, T> {
    fn from_iter<I: IntoIterator<Item = &'a Dependency<T>>>(iterable: I) -> Self {
        Self(iterable.into_iter().collect())
    }
}

impl<'a, T: Ordered> Iterator for IterConditionals<'a, T> {
    type Item = &'a UseDep;

    fn next(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        while let Some(dep) = self.0.pop_front() {
            match dep {
                Enabled(_) | Disabled(_) => (),
                AllOf(vals) => self.0.extend_left(vals),
                AnyOf(vals) => self.0.extend_left(vals),
                ExactlyOneOf(vals) => self.0.extend_left(vals),
                AtMostOneOf(vals) => self.0.extend_left(vals),
                Conditional(u, vals) => {
                    self.0.extend_left(vals);
                    return Some(u);
                }
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct IntoIterConditionals<T: Ordered>(Deque<Dependency<T>>);

impl<T: Ordered> FromIterator<Dependency<T>> for IntoIterConditionals<T> {
    fn from_iter<I: IntoIterator<Item = Dependency<T>>>(iterable: I) -> Self {
        Self(iterable.into_iter().collect())
    }
}

impl<T: Ordered> Iterator for IntoIterConditionals<T> {
    type Item = UseDep;

    fn next(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        while let Some(dep) = self.0.pop_front() {
            match dep {
                Enabled(_) | Disabled(_) => (),
                AllOf(vals) => self.0.extend_left(vals),
                AnyOf(vals) => self.0.extend_left(vals),
                ExactlyOneOf(vals) => self.0.extend_left(vals),
                AtMostOneOf(vals) => self.0.extend_left(vals),
                Conditional(u, vals) => {
                    self.0.extend_left(vals);
                    return Some(u);
                }
            }
        }
        None
    }
}

macro_rules! eval {
    ($vals:expr, $options:expr) => {
        $vals
            .into_iter()
            .flat_map(|d| d.into_iter_evaluate($options))
            .collect()
    };
}

macro_rules! return_non_empty {
    ($type:ident, $vals:expr, $options:expr) => {{
        let evaluated = $type(eval!($vals, $options));
        if !evaluated.is_empty() {
            return Some(evaluated);
        }
    }};
}

#[derive(Debug)]
pub struct IterEvaluate<'a, S: Stringable, T: Ordered> {
    pub(super) q: Deque<&'a Dependency<T>>,
    pub(super) options: &'a IndexSet<S>,
}

impl<'a, S: Stringable, T: Ordered> Iterator for IterEvaluate<'a, S, T> {
    type Item = Dependency<&'a T>;

    fn next(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        while let Some(dep) = self.q.pop_front() {
            match dep {
                Enabled(val) => return Some(Enabled(val)),
                Disabled(val) => return Some(Disabled(val)),
                AllOf(vals) => return_non_empty!(AllOf, vals, self.options),
                AnyOf(vals) => return_non_empty!(AnyOf, vals, self.options),
                ExactlyOneOf(vals) => return_non_empty!(ExactlyOneOf, vals, self.options),
                AtMostOneOf(vals) => return_non_empty!(AtMostOneOf, vals, self.options),
                Conditional(u, vals) => {
                    if u.matches(self.options) {
                        self.q.extend_left(vals);
                    }
                }
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct IntoIterEvaluate<'a, S: Stringable, T: Ordered> {
    pub(super) q: Deque<Dependency<&'a T>>,
    pub(super) options: &'a IndexSet<S>,
}

impl<'a, S: Stringable, T: Ordered> Iterator for IntoIterEvaluate<'a, S, T> {
    type Item = Dependency<&'a T>;

    fn next(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        while let Some(dep) = self.q.pop_front() {
            match dep {
                Enabled(val) => return Some(Enabled(val)),
                Disabled(val) => return Some(Disabled(val)),
                AllOf(vals) => return_non_empty!(AllOf, vals, self.options),
                AnyOf(vals) => return_non_empty!(AnyOf, vals, self.options),
                ExactlyOneOf(vals) => return_non_empty!(ExactlyOneOf, vals, self.options),
                AtMostOneOf(vals) => return_non_empty!(AtMostOneOf, vals, self.options),
                Conditional(u, vals) => {
                    if u.matches(self.options) {
                        self.q.extend_left(vals);
                    }
                }
            }
        }
        None
    }
}

macro_rules! iter_eval_force {
    ($variant:expr, $vals:expr, $force:expr) => {{
        let dep = $variant(
            $vals
                .into_iter()
                .flat_map(|d| d.into_iter_evaluate_force($force))
                .collect(),
        );

        if !dep.is_empty() {
            return Some(dep);
        }
    }};
}

#[derive(Debug)]
pub struct IterEvaluateForce<'a, T: Ordered> {
    pub(super) q: Deque<&'a Dependency<T>>,
    pub(super) force: bool,
}

impl<'a, T: Ordered> Iterator for IterEvaluateForce<'a, T> {
    type Item = Dependency<&'a T>;

    fn next(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        while let Some(dep) = self.q.pop_front() {
            match dep {
                Enabled(val) => return Some(Enabled(val)),
                Disabled(val) => return Some(Disabled(val)),
                AllOf(vals) => iter_eval_force!(AllOf, vals, self.force),
                AnyOf(vals) => iter_eval_force!(AnyOf, vals, self.force),
                ExactlyOneOf(vals) => iter_eval_force!(ExactlyOneOf, vals, self.force),
                AtMostOneOf(vals) => iter_eval_force!(AtMostOneOf, vals, self.force),
                Conditional(_, vals) => {
                    if self.force {
                        self.q.extend_left(vals);
                    }
                }
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct IntoIterEvaluateForce<'a, T: Ordered> {
    pub(super) q: Deque<Dependency<&'a T>>,
    pub(super) force: bool,
}

impl<'a, T: Ordered> Iterator for IntoIterEvaluateForce<'a, T> {
    type Item = Dependency<&'a T>;

    fn next(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        while let Some(dep) = self.q.pop_front() {
            match dep {
                Enabled(val) => return Some(Enabled(val)),
                Disabled(val) => return Some(Disabled(val)),
                AllOf(vals) => iter_eval_force!(AllOf, vals, self.force),
                AnyOf(vals) => iter_eval_force!(AnyOf, vals, self.force),
                ExactlyOneOf(vals) => iter_eval_force!(ExactlyOneOf, vals, self.force),
                AtMostOneOf(vals) => iter_eval_force!(AtMostOneOf, vals, self.force),
                Conditional(_, vals) => {
                    if self.force {
                        self.q.extend_left(vals);
                    }
                }
            }
        }
        None
    }
}
