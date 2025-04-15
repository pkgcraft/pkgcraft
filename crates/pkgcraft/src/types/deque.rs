use std::collections::VecDeque;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone)]
pub struct Deque<T>(VecDeque<T>);

impl<T> Default for Deque<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T> Deque<T> {
    /// Construct a new, empty Deque<T>.
    pub fn new() -> Self {
        Self::default()
    }

    /// Prepends all elements to the deque.
    pub fn extend_left<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = T>,
        I::IntoIter: DoubleEndedIterator,
    {
        for item in iter.into_iter().rev() {
            self.push_front(item);
        }
    }
}

impl<T> Deref for Deque<T> {
    type Target = VecDeque<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Deque<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> FromIterator<T> for Deque<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iterable: I) -> Self {
        Self(iterable.into_iter().collect())
    }
}

impl<'a, T> IntoIterator for &'a Deque<T> {
    type Item = &'a T;
    type IntoIter = std::collections::vec_deque::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<T> IntoIterator for Deque<T> {
    type Item = T;
    type IntoIter = std::collections::vec_deque::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
