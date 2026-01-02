use std::io::Write;
use std::ops::{Deref, DerefMut};
use std::path::Path;

use tempfile::NamedTempFile;

/// Create a custom git config.
fn git_config() -> NamedTempFile {
    let data = indoc::indoc! {"
        [user]
            name = Pkgcruft Git
            email = pkgcruft-git@pkgcruft.pkgcraft
    "};
    let mut config = NamedTempFile::new().unwrap();
    config.write_all(data.as_bytes()).unwrap();
    config
}

/// Git repository wrapper.
pub(crate) struct GitRepo(git2::Repository);

impl GitRepo {
    /// Initialize a git repo at a path.
    pub(crate) fn init<P: AsRef<Path>>(path: P) -> pkgcruft_git::Result<Self> {
        let mut opts = git2::RepositoryInitOptions::new();
        opts.bare(false)
            .external_template(false)
            .initial_head("main");
        Ok(Self(git2::Repository::init_opts(path, &opts)?))
    }

    /// Initialize a bare git repo at a path.
    pub(crate) fn init_bare<P: AsRef<Path>>(path: P) -> pkgcruft_git::Result<Self> {
        let mut opts = git2::RepositoryInitOptions::new();
        opts.bare(true)
            .external_template(false)
            .initial_head("main");
        Ok(Self(git2::Repository::init_opts(path, &opts)?))
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

/// Synchronous git command wrapper.
pub(crate) struct GitCmd {
    cmd: assert_cmd::Command,
    _config: NamedTempFile,
}

impl Deref for GitCmd {
    type Target = assert_cmd::Command;

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
        let mut cmd = assert_cmd::Command::new("git");
        cmd.args(&args);

        // disable system config
        cmd.env("GIT_CONFIG_SYSTEM", "/dev/null");
        // use custom user config
        let config = git_config();
        cmd.env("GIT_CONFIG_GLOBAL", config.path());

        Self { cmd, _config: config }
    }
}

/// Run a git command ignoring system and user config settings.
#[macro_export]
macro_rules! git {
    ($cmd:expr) => {
        $crate::git::GitCmd::new($cmd)
    };
}
pub(crate) use git;

/// Asynchronous git command wrapper.
pub(crate) struct GitCmdAsync {
    cmd: tokio::process::Command,
    _config: NamedTempFile,
}

impl Deref for GitCmdAsync {
    type Target = tokio::process::Command;

    fn deref(&self) -> &Self::Target {
        &self.cmd
    }
}

impl DerefMut for GitCmdAsync {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cmd
    }
}

impl GitCmdAsync {
    /// Construct a git command that uses custom config files.
    pub(crate) fn new<S: AsRef<str>>(cmd: S) -> Self {
        let args: Vec<_> = cmd.as_ref().split_whitespace().collect();
        let mut cmd = tokio::process::Command::new("git");
        cmd.args(&args);

        // disable system config
        cmd.env("GIT_CONFIG_SYSTEM", "/dev/null");
        // use custom user config
        let config = git_config();
        cmd.env("GIT_CONFIG_GLOBAL", config.path());

        Self { cmd, _config: config }
    }
}

/// Run an async git command ignoring system and user config settings.
#[macro_export]
macro_rules! git_async {
    ($cmd:expr) => {
        $crate::git::GitCmdAsync::new($cmd)
    };
}
pub(crate) use git_async;
