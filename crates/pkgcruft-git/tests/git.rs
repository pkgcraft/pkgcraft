use std::ops::{Deref, DerefMut};
use std::path::Path;

/// Git repository wrapper.
pub(crate) struct GitRepo(git2::Repository);

impl GitRepo {
    /// Initialize a git repo at a path, adding all files to an initial commit.
    pub(crate) fn init<P: AsRef<Path>>(path: P) -> pkgcruft_git::Result<Self> {
        let repo = git2::Repository::init(path)?;

        // create initial commit inside block so the tree ref is dropped
        {
            let mut index = repo.index().unwrap();
            index
                .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
                .unwrap();
            index.write().unwrap();
            let oid = index.write_tree().unwrap();
            let tree = repo.find_tree(oid).unwrap();
            let sig = git2::Signature::new("test", "test@test.test", &git2::Time::new(0, 0))
                .unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "initial import", &tree, &[])
                .unwrap();
        }

        Ok(Self(repo))
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
