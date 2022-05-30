use std::path::Path;
use std::{fmt, iter};

use super::{make_repo_traits, Repository};
use crate::config::RepoConfig;
use crate::{atom, pkg, repo, Result};

#[derive(Debug, Default)]
pub struct Repo {
    id: String,
    config: RepoConfig,
}

make_repo_traits!(Repo);

impl Repo {
    pub(super) const FORMAT: &'static str = "empty";

    pub(crate) fn new(id: &str) -> Result<Repo> {
        Ok(Repo {
            id: id.to_string(),
            config: Default::default(),
        })
    }

    pub fn iter(&self) -> iter::Empty<pkg::Pkg<'_>> {
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

impl<T: AsRef<Path>> repo::Contains<T> for Repo {
    fn contains(&self, _path: T) -> bool {
        false
    }
}

impl repo::Contains<&atom::Atom> for Repo {
    fn contains(&self, _atom: &atom::Atom) -> bool {
        false
    }
}

impl repo::Contains<atom::Atom> for Repo {
    fn contains(&self, _atom: atom::Atom) -> bool {
        false
    }
}
