use std::hash::Hash;

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::{Equivalent, IndexSet};
use scallop::{source, ExecStatus};

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

impl<T: IntoOwned> IntoOwned for crate::Result<T> {
    type Owned = crate::Result<T::Owned>;

    fn into_owned(self) -> Self::Owned {
        self.map(|x| x.into_owned())
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

/// Shared data cache trait.
pub(crate) trait ArcCacheData: Default {
    const RELPATH: &'static str;
    fn parse(data: &str) -> crate::Result<Self>;
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
