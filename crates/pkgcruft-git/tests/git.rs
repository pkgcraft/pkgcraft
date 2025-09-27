use std::io::Write;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::sync::LazyLock;

use assert_cmd::Command;
use tempfile::NamedTempFile;

/// Determine if the `git` binary exists in the system path.
pub(crate) static GIT_EXISTS: LazyLock<bool> =
    LazyLock::new(|| Command::new("git").arg("-v").ok().is_ok());

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

/// Git command wrapper.
pub(crate) struct GitCmd {
    cmd: Command,
    _config: NamedTempFile,
}

impl Deref for GitCmd {
    type Target = Command;

    fn deref(&self) -> &Self::Target {
        &self.cmd
    }
}

impl DerefMut for GitCmd {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cmd
    }
}

impl GitCmd {
    /// Construct a git command that uses custom config files.
    pub(crate) fn new<S: AsRef<str>>(cmd: S) -> Self {
        let args: Vec<_> = cmd.as_ref().split_whitespace().collect();
        let mut cmd = Command::new("git");
        cmd.args(&args);

        // create custom git config
        let data = indoc::indoc! {"
            [user]
                name = Pkgcruft Git
                email = pkgcruft-git@pkgcruft.pkgcraft
        "};
        let mut config = NamedTempFile::new().unwrap();
        config.write_all(data.as_bytes()).unwrap();
        let config_path = config.path().to_str().unwrap();

        // disable system config
        cmd.env("GIT_CONFIG_SYSTEM", "/dev/null");
        // use custom user config
        cmd.env("GIT_CONFIG_GLOBAL", config_path);

        Self { cmd, _config: config }
    }
}

/// Run a git command if `git` exists on the system path.
#[macro_export]
macro_rules! git {
    ($cmd:expr) => {
        if *$crate::git::GIT_EXISTS {
            $crate::git::GitCmd::new($cmd)
        } else {
            return;
        }
    };
}
pub(crate) use git;
