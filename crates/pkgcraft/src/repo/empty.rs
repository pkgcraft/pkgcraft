use std::{fmt, iter};

use camino::Utf8Path;

use crate::config::RepoConfig;
use crate::pkg::Pkg;
use crate::restrict::Restrict;
use crate::Error;

use super::{make_repo_traits, PkgRepository, RepoFormat, Repository};

#[derive(Debug, Default)]
pub struct Repo {
    id: String,
    repo_config: RepoConfig,
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

    pub(super) fn from_path<P: AsRef<Utf8Path>>(
        id: &str,
        priority: i32,
        path: P,
    ) -> crate::Result<Self> {
        let path = path.as_ref();
        if path.exists() {
            Err(Error::RepoInit("not an empty repo".to_string()))
        } else {
            Ok(Self::new(id, priority))
        }
    }

    pub(super) fn repo_config(&self) -> &RepoConfig {
        &self.repo_config
    }
}

impl fmt::Display for Repo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: empty repo", self.id)
    }
}

impl PkgRepository for Repo {
    type Pkg<'a> = Pkg<'a> where Self: 'a;
    type Iterator<'a> = iter::Empty<Self::Pkg<'a>> where Self: 'a;
    type RestrictIterator<'a> = iter::Empty<Self::Pkg<'a>> where Self: 'a;

    fn categories(&self) -> Vec<String> {
        vec![]
    }

    fn packages(&self, _cat: &str) -> Vec<String> {
        vec![]
    }

    fn versions(&self, _cat: &str, _pkg: &str) -> Vec<String> {
        vec![]
    }

    fn len(&self) -> usize {
        0
    }

    fn iter(&self) -> Self::Iterator<'_> {
        iter::empty::<Self::Pkg<'_>>()
    }

    fn iter_restrict<R: Into<Restrict>>(&self, _val: R) -> Self::RestrictIterator<'_> {
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
    use std::str::FromStr;

    use crate::atom::Atom;
    use crate::repo::Contains;

    use super::*;

    #[test]
    fn test_contains() {
        let repo = Repo::new("empty", 0);

        // path containment
        assert!(!repo.contains("cat/pkg"));

        // cpv containment
        let cpv = Atom::new_cpv("cat/pkg-0").unwrap();
        assert!(!repo.contains(&cpv));

        // atom containment
        let a = Atom::from_str("cat/pkg").unwrap();
        assert!(!repo.contains(&a));
    }

    #[test]
    fn test_iter() {
        let repo = Repo::new("empty", 0);
        assert!(repo.iter().next().is_none());
    }
}
