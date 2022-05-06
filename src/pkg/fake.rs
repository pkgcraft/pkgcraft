use std::fmt;

use crate::repo::Repo as RepoTrait;
use crate::{atom, eapi, pkg, repo, Result};

#[derive(Debug, Clone)]
pub struct Pkg<'a> {
    atom: atom::Atom,
    repo: &'a repo::fake::Repo,
}

impl PartialEq for Pkg<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.atom == other.atom && self.repo.id() == other.repo.id()
    }
}

impl Eq for Pkg<'_> {}

impl<'a> Pkg<'a> {
    pub fn new<S: AsRef<str>>(cpv: S, repo: &'a repo::fake::Repo) -> Result<Self> {
        let atom = atom::parse::cpv(cpv.as_ref())?;
        Ok(Pkg { atom, repo })
    }
}

impl fmt::Display for Pkg<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.atom)
    }
}

impl pkg::Pkg for Pkg<'_> {
    type Repo = repo::fake::Repo;

    fn eapi(&self) -> &eapi::Eapi {
        &eapi::EAPI_LATEST
    }

    fn repo(&self) -> &Self::Repo {
        self.repo
    }
}
