use std::cell::Cell;
use std::hash::Hash;
use std::process::ExitCode;
use std::rc::Rc;

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::{Equivalent, IndexSet};
use scallop::{ExecStatus, source};
use tracing::error;

/// Return true if a container type contains a given object, otherwise false.
pub trait Contains<T> {
    fn contains(&self, obj: T) -> bool;
}

impl<T, Q> Contains<&Q> for IndexSet<T>
where
    Q: Eq + Hash + Equivalent<T>,
    T: Eq + Hash,
{
    fn contains(&self, value: &Q) -> bool {
        IndexSet::contains(self, value)
    }
}

impl<T, V> Contains<&V> for &T
where
    T: for<'a> Contains<&'a V>,
{
    fn contains(&self, value: &V) -> bool {
        (*self).contains(value)
    }
}

/// Determine if two objects intersect.
pub trait Intersects<Rhs = Self> {
    fn intersects(&self, obj: &Rhs) -> bool;
}

/// Convert a borrowed type into an owned type.
pub trait IntoOwned {
    type Owned;
    fn into_owned(self) -> Self::Owned;
}

impl<T: IntoOwned> IntoOwned for Option<T> {
    type Owned = Option<T::Owned>;

    fn into_owned(self) -> Self::Owned {
        self.map(|x| x.into_owned())
    }
}

impl<T: IntoOwned> IntoOwned for Result<T, &crate::Error> {
    type Owned = Result<T::Owned, crate::Error>;

    fn into_owned(self) -> Self::Owned {
        self.map(|x| x.into_owned()).map_err(|e| e.clone())
    }
}

/// Create a borrowed type from an owned type.
pub trait ToRef<'a> {
    type Ref;
    fn to_ref(&'a self) -> Self::Ref;
}

impl<'a, T: ToRef<'a>> ToRef<'a> for Option<T> {
    type Ref = Option<T::Ref>;

    fn to_ref(&'a self) -> Self::Ref {
        self.as_ref().map(|x| x.to_ref())
    }
}

impl<'a, T: ToRef<'a>> ToRef<'a> for Result<T, crate::Error> {
    type Ref = Result<T::Ref, &'a crate::Error>;

    fn to_ref(&'a self) -> Self::Ref {
        self.as_ref().map(|x| x.to_ref())
    }
}

/// Iterate over an object's lines, filtering comments starting with '#' and empty lines returning
/// an enumerated iterator for the remaining content.
pub trait FilterLines {
    fn filter_lines(&self) -> impl Iterator<Item = (usize, &str)>;
}

impl<T: AsRef<str>> FilterLines for T {
    fn filter_lines(&self) -> impl Iterator<Item = (usize, &str)> {
        self.as_ref()
            .lines()
            .map(|s| s.trim())
            .enumerate()
            .map(|(i, s)| (i + 1, s))
            .filter(|(_, s)| !s.is_empty() && !s.starts_with('#'))
    }
}

/// Filter an iterable of results while conditionally logging errors.
pub trait LogErrors<I, T>
where
    I: IntoIterator<Item = crate::Result<T>>,
{
    fn log_errors(self, ignore: bool) -> LogErrorsIter<I::IntoIter, T>;
}

/// Iterator that filters an iterator of results while logging errors.
pub struct LogErrorsIter<I, T>
where
    I: Iterator<Item = crate::Result<T>>,
{
    iter: I,
    pub failed: Rc<Cell<bool>>,
    ignore: bool,
}

impl<I, T> From<LogErrorsIter<I, T>> for ExitCode
where
    I: Iterator<Item = crate::Result<T>>,
{
    fn from(iter: LogErrorsIter<I, T>) -> Self {
        ExitCode::from(iter.failed() as u8)
    }
}

impl<I, T> LogErrorsIter<I, T>
where
    I: Iterator<Item = crate::Result<T>>,
{
    /// Return true if any errors occurred during iteration, false otherwise.
    pub fn failed(&self) -> bool {
        self.failed.get()
    }
}

impl<I, T> Iterator for LogErrorsIter<I, T>
where
    I: Iterator<Item = crate::Result<T>>,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        for result in &mut self.iter {
            match result {
                Ok(object) => return Some(object),
                Err(e) => {
                    if !self.ignore {
                        error!("{e}");
                        self.failed.set(true);
                    }
                    continue;
                }
            }
        }

        None
    }
}

impl<I, T> LogErrors<I, T> for I
where
    I: IntoIterator<Item = crate::Result<T>>,
{
    fn log_errors(self, ignore: bool) -> LogErrorsIter<I::IntoIter, T> {
        LogErrorsIter {
            iter: self.into_iter(),
            failed: Default::default(),
            ignore,
        }
    }
}

/// Support bash sourcing via file paths or directly from string content.
pub(crate) trait SourceBash {
    fn source_bash(&self) -> scallop::Result<ExecStatus>;
}

macro_rules! make_source_path_trait {
    ($($x:ty),+) => {$(
        impl SourceBash for $x {
            fn source_bash(&self) -> scallop::Result<ExecStatus> {
                if !self.exists() {
                    return Err(scallop::Error::Base(format!("nonexistent file: {self}")));
                }

                source::file(self)
            }
        }
    )+};
}
make_source_path_trait!(&Utf8Path, &Utf8PathBuf);

impl SourceBash for &str {
    fn source_bash(&self) -> scallop::Result<ExecStatus> {
        source::string(self)
    }
}
