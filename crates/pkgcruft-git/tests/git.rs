use std::ops::{Deref, DerefMut};
use std::path::Path;

/// Git repository wrapper.
pub(crate) struct GitRepo(git2::Repository);

impl GitRepo {
    /// Initialize a git repo at a path, adding all files to an initial commit.
    pub(crate) fn init<P: AsRef<Path>>(path: P) -> pkgcruft_git::Result<Self> {
        let repo = Self(git2::Repository::init(path)?);
        let oid = repo.stage(&["*"])?;
        repo.commit(oid, "initial import")?;
        Ok(repo)
    }

    /// Stage the given file paths, updating the index, and returning the index tree's Oid.
    pub(crate) fn stage(&self, paths: &[&str]) -> pkgcruft_git::Result<git2::Oid> {
        let mut index = self.0.index().unwrap();
        index.add_all(paths, git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;
        let oid = index.write_tree()?;
        Ok(oid)
    }

    /// Create a commit for a tree Oid using the given commit message.
    pub(crate) fn commit(&self, oid: git2::Oid, msg: &str) -> pkgcruft_git::Result<()> {
        let tree = self.0.find_tree(oid)?;
        let sig = git2::Signature::new("test", "test@test.test", &git2::Time::new(0, 0))?;
        self.0.commit(Some("HEAD"), &sig, &sig, msg, &tree, &[])?;
        Ok(())
    }
}

impl Deref for GitRepo {
    type Target = git2::Repository;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for GitRepo {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
