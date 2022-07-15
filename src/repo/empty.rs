use std::{fmt, iter};

use camino::Utf8Path;

use super::{make_repo_traits, Repository};
use crate::config::RepoConfig;
use crate::pkg::Package;
use crate::restrict::{Restrict, Restriction};
use crate::{atom, pkg, repo, Error};

#[derive(Debug, Default)]
pub struct Repo {
    id: String,
    config: RepoConfig,
}

make_repo_traits!(Repo);

impl Repo {
    pub(crate) fn new(id: &str, priority: i32) -> Repo {
        let config = RepoConfig {
            priority,
            ..Default::default()
        };

        Repo {
            id: id.to_string(),
            config,
        }
    }

    pub(super) fn from_path<P: AsRef<Utf8Path>>(
        id: &str,
        priority: i32,
        path: P,
    ) -> crate::Result<Self> {
        let path = path.as_ref();
        match path.exists() {
            false => Err(Error::RepoInit("not an empty repo".to_string())),
            true => Ok(Repo::new(id, priority)),
        }
    }

    pub fn iter(&self) -> iter::Empty<pkg::Pkg<'_>> {
        iter::empty::<pkg::Pkg<'_>>()
    }

    pub fn iter_restrict<T: Into<Restrict>>(&self, _val: T) -> iter::Empty<pkg::Pkg<'_>> {
        iter::empty::<pkg::Pkg<'_>>()
    }
}

impl fmt::Display for Repo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: empty repo", self.id)
    }
}

impl Repository for Repo {
    fn categories(&self) -> Vec<String> {
        vec![]
    }

    fn packages(&self, _cat: &str) -> Vec<String> {
        vec![]
    }

    fn versions(&self, _cat: &str, _pkg: &str) -> Vec<String> {
        vec![]
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn config(&self) -> &RepoConfig {
        &self.config
    }

    fn len(&self) -> usize {
        0
    }

    fn is_empty(&self) -> bool {
        true
    }
}

impl<T: AsRef<Utf8Path>> repo::Contains<T> for Repo {
    fn contains(&self, _path: T) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::atom;
    use crate::repo::Contains;

    use super::*;

    #[test]
    fn test_contains() {
        let repo = Repo::new("empty", 0);

        // path containment
        assert!(!repo.contains("cat/pkg"));

        // cpv containment
        let cpv = atom::cpv("cat/pkg-0").unwrap();
        assert!(!repo.contains(&cpv));
        assert!(!repo.contains(cpv));

        // atom containment
        let a = atom::Atom::from_str("cat/pkg").unwrap();
        assert!(!repo.contains(&a));
        assert!(!repo.contains(a));
    }

    #[test]
    fn test_iter() {
        let repo = Repo::new("empty", 0);
        assert!(repo.iter().next().is_none());
    }
}
