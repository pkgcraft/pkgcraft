use std::hash::{Hash, Hasher};
use std::{fmt, iter};

use camino::Utf8Path;
use indexmap::IndexSet;

use crate::config::RepoConfig;
use crate::dep::{Cpv, Version};
use crate::pkg::Pkg;
use crate::restrict::Restrict;
use crate::Error;

use super::{make_repo_traits, PkgRepository, RepoFormat, Repository};

#[derive(Debug, Default)]
pub struct Repo {
    id: String,
    repo_config: RepoConfig,
}

impl PartialEq for Repo {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Repo {}

impl Hash for Repo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

make_repo_traits!(Repo);

impl Repo {
    pub(crate) fn new(id: &str, priority: i32) -> Self {
        let repo_config = RepoConfig { priority, ..Default::default() };

        Self {
            id: id.to_string(),
            repo_config,
        }
    }

    pub(super) fn from_path<P: AsRef<Utf8Path>, S: AsRef<str>>(
        id: S,
        priority: i32,
        path: P,
    ) -> crate::Result<Self> {
        let id = id.as_ref();
        let path = path.as_ref();
        if path.exists() {
            Ok(Self::new(id, priority))
        } else {
            Err(Error::NotARepo {
                kind: RepoFormat::Empty,
                id: id.to_string(),
                err: "repo dir exists".to_string(),
            })
        }
    }

    pub(super) fn repo_config(&self) -> &RepoConfig {
        &self.repo_config
    }
}

impl fmt::Display for Repo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

impl PkgRepository for Repo {
    type Pkg<'a> = Pkg<'a> where Self: 'a;
    type IterCpv<'a> = iter::Empty<Cpv<String>> where Self: 'a;
    type Iter<'a> = iter::Empty<Self::Pkg<'a>> where Self: 'a;
    type IterRestrict<'a> = iter::Empty<Self::Pkg<'a>> where Self: 'a;

    fn categories(&self) -> IndexSet<String> {
        IndexSet::new()
    }

    fn packages(&self, _cat: &str) -> IndexSet<String> {
        IndexSet::new()
    }

    fn versions(&self, _cat: &str, _pkg: &str) -> IndexSet<Version<String>> {
        IndexSet::new()
    }

    fn len(&self) -> usize {
        0
    }

    fn iter_cpv(&self) -> Self::IterCpv<'_> {
        iter::empty::<Cpv<String>>()
    }

    fn iter(&self) -> Self::Iter<'_> {
        iter::empty::<Self::Pkg<'_>>()
    }

    fn iter_restrict<R: Into<Restrict>>(&self, _val: R) -> Self::IterRestrict<'_> {
        iter::empty::<Self::Pkg<'_>>()
    }
}

impl Repository for Repo {
    fn format(&self) -> RepoFormat {
        self.repo_config.format
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn priority(&self) -> i32 {
        self.repo_config.priority
    }

    fn path(&self) -> &Utf8Path {
        &self.repo_config.location
    }

    fn sync(&self) -> crate::Result<()> {
        self.repo_config.sync()
    }
}

#[cfg(test)]
mod tests {
    use crate::dep::Dep;
    use crate::repo::Contains;

    use super::*;

    #[test]
    fn test_contains() {
        let repo = Repo::new("empty", 0);

        // path
        assert!(!repo.contains("cat/pkg"));

        // versioned dep
        let cpv = Cpv::new("cat/pkg-0").unwrap();
        assert!(!repo.contains(&cpv));

        // unversioned dep
        let a = Dep::new("cat/pkg").unwrap();
        assert!(!repo.contains(&a));
    }

    #[test]
    fn test_iter() {
        let repo = Repo::new("empty", 0);
        assert!(repo.iter().next().is_none());
    }
}
