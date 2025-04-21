use std::fs::File;
use std::io::{stderr, stdout};
use std::os::fd::{AsRawFd, RawFd};
use std::sync::OnceLock;

use indexmap::IndexMap;
use ipc_channel::ipc::{self, IpcReceiver, IpcSender};
use nix::sys::{prctl, signal::Signal};
use nix::unistd::{dup2, fork, ForkResult};
use scallop::pool::SharedSemaphore;
use scallop::variables::{self, ShellVariable};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::dep::Cpv;
use crate::error::Error;
use crate::pkg::ebuild::metadata::Metadata;
use crate::pkg::{ebuild::EbuildRawPkg, PkgPretend, Source};
use crate::repo::ebuild::cache::{Cache, CacheEntry, MetadataCache};
use crate::repo::ebuild::EbuildRepo;
use crate::repo::Repository;

/// Get an ebuild repo from a config matching a given ID.
fn get_ebuild_repo<'a>(config: &'a Config, repo: &str) -> crate::Result<&'a EbuildRepo> {
    config
        .repos
        .get(repo)?
        .as_ebuild()
        .ok_or_else(|| Error::InvalidValue(format!("unknown ebuild repo: {repo}")))
}

/// Update an ebuild repo's package metadata cache for a given [`Cpv`].
#[derive(Debug, Serialize, Deserialize)]
struct MetadataTask {
    repo: String,
    cpv: Cpv,
    cache: MetadataCache,
    verify: bool,
}

impl MetadataTask {
    fn new(repo: &EbuildRepo, cpv: Cpv, cache: MetadataCache, verify: bool) -> Self {
        Self {
            repo: repo.id().to_string(),
            cpv,
            cache,
            verify,
        }
    }

    fn run(self, config: &Config) -> crate::Result<()> {
        let repo = get_ebuild_repo(config, &self.repo)?;
        let pkg = EbuildRawPkg::try_new(self.cpv, repo)?;
        let meta = Metadata::try_from(&pkg).map_err(|e| e.into_invalid_pkg_err(&pkg))?;
        if !self.verify {
            self.cache.update(&pkg, &meta)?;
        }
        Ok(())
    }
}

/// Task builder for ebuild package metadata cache generation.
#[derive(Debug)]
pub struct MetadataTaskBuilder {
    tx: IpcSender<Command>,
    repo: EbuildRepo,
    cache: Option<MetadataCache>,
    force: bool,
    output: Pipes,
    verify: bool,
}

// needed due to IpcSender lacking Sync
unsafe impl Sync for MetadataTaskBuilder {}

impl MetadataTaskBuilder {
    /// Create a new ebuild package metadata cache task builder.
    fn new(pool: &BuildPool, repo: &EbuildRepo) -> Self {
        Self {
            tx: pool.tx.clone(),
            repo: repo.clone(),
            cache: Default::default(),
            force: Default::default(),
            output: Default::default(),
            verify: Default::default(),
        }
    }

    /// Use a custom metadata cache.
    pub fn cache(mut self, value: &MetadataCache) -> Self {
        self.cache = Some(value.clone());
        self
    }

    /// Force the package's metadata to be regenerated.
    pub fn force(mut self, value: bool) -> Self {
        self.force = value;
        self
    }

    /// Pass through output to stderr and stdout.
    pub fn output(mut self, value: bool) -> Self {
        if value {
            self.output = Pipes::all();
        }
        self
    }

    /// Verify the package's metadata.
    pub fn verify(mut self, value: bool) -> Self {
        self.verify = value;
        self
    }

    /// Run the task for a target [`Cpv`].
    pub fn run<T: Into<Cpv>>(&self, cpv: T) -> crate::Result<()> {
        let cpv = cpv.into();
        let cache = self
            .cache
            .as_ref()
            .unwrap_or_else(|| self.repo.metadata().cache());

        if !self.force {
            let pkg = self.repo.get_pkg_raw(cpv.clone())?;
            if let Some(result) = cache.get(&pkg) {
                if self.verify {
                    // perform deserialization, returning any occurring error
                    return result.and_then(|e| e.to_metadata(&pkg)).map(|_| ());
                } else if result.is_ok() {
                    // skip deserialization, assuming existing cache entry is valid
                    return Ok(());
                }
            }
        }
        let meta = MetadataTask::new(&self.repo, cpv, cache.clone(), self.verify);
        let (tx, rx) = ipc::channel()
            .map_err(|e| Error::InvalidValue(format!("failed creating task channel: {e}")))?;
        let task = Task::Metadata(meta, tx);
        self.tx
            .send(Command::Task(task, self.output))
            .map_err(|e| Error::InvalidValue(format!("failed queuing task: {e}")))?;
        rx.recv()
            .map_err(|e| Error::InvalidValue(format!("failed receiving task status: {e}")))?
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct PkgPretendTask {
    repo: String,
    cpv: Cpv,
}

impl PkgPretendTask {
    fn new<T: Into<Cpv>>(repo: &EbuildRepo, cpv: T) -> Self {
        Self {
            repo: repo.id().to_string(),
            cpv: cpv.into(),
        }
    }

    fn run(self, config: &Config) -> crate::Result<Option<String>> {
        let repo = get_ebuild_repo(config, &self.repo)?;
        let pkg = repo.get_pkg(self.cpv)?;
        Ok(pkg.pkg_pretend()?)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SourceEnvTask {
    repo: String,
    cpv: Cpv,
}

impl SourceEnvTask {
    fn new<T: Into<Cpv>>(repo: &EbuildRepo, cpv: T) -> Self {
        Self {
            repo: repo.id().to_string(),
            cpv: cpv.into(),
        }
    }

    fn run(self, config: &Config) -> crate::Result<IndexMap<String, String>> {
        let repo = get_ebuild_repo(config, &self.repo)?;
        let pkg = repo.get_pkg_raw(self.cpv)?;
        pkg.source()?;
        Ok(variables::visible()
            .into_iter()
            .filter_map(|var| var.to_vec().map(|v| (var.to_string(), v.join(" "))))
            .collect())
    }
}

/// Build pool task.
#[derive(Debug, Serialize, Deserialize)]
enum Task {
    Metadata(MetadataTask, IpcSender<crate::Result<()>>),
    PkgPretend(PkgPretendTask, IpcSender<crate::Result<Option<String>>>),
    SourceEnv(SourceEnvTask, IpcSender<crate::Result<IndexMap<String, String>>>),
}

impl Task {
    fn run(self, config: &Config, pipes: Pipes, nullfd: RawFd) {
        // redirect output
        let result = pipes.redirect(nullfd);

        // run the task
        match self {
            Self::Metadata(task, tx) => {
                tx.send(result.and_then(|_| task.run(config))).unwrap();
            }
            Self::PkgPretend(task, tx) => {
                tx.send(result.and_then(|_| task.run(config))).unwrap();
            }
            Self::SourceEnv(task, tx) => {
                tx.send(result.and_then(|_| task.run(config))).unwrap();
            }
        }
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
struct Pipes {
    stdout: Option<RawFd>,
    stderr: Option<RawFd>,
}

impl Pipes {
    /// Redirect stdout and stderr for the current process.
    fn all() -> Self {
        Self {
            stdout: Some(stdout().as_raw_fd()),
            stderr: Some(stderr().as_raw_fd()),
        }
    }

    /// Redirect stdout and stderr to the specified fds, if any.
    fn redirect(&self, nullfd: RawFd) -> crate::Result<()> {
        dup2(self.stdout.unwrap_or(nullfd), 1)
            .map_err(|e| Error::InvalidValue(format!("failed redirecting stdout: {e}")))?;

        dup2(self.stderr.unwrap_or(nullfd), 2)
            .map_err(|e| Error::InvalidValue(format!("failed redirecting stderr: {e}")))?;

        Ok(())
    }
}

/// Build pool command.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Serialize, Deserialize)]
enum Command {
    Task(Task, Pipes),
    Stop,
}

#[derive(Debug)]
pub struct BuildPool {
    jobs: usize,
    tx: IpcSender<Command>,
    rx: IpcReceiver<Command>,
    running: OnceLock<bool>,
}

// needed due to IpcSender lacking Sync
unsafe impl Sync for BuildPool {}

impl Default for BuildPool {
    fn default() -> Self {
        Self::new(num_cpus::get())
    }
}

impl BuildPool {
    pub(crate) fn new(jobs: usize) -> Self {
        let (tx, rx) = ipc::channel().unwrap();
        Self {
            jobs,
            tx,
            rx,
            running: OnceLock::new(),
        }
    }

    /// Start the build pool loop.
    pub(crate) fn start(&self, config: &Config) -> crate::Result<()> {
        self.running
            .set(true)
            .map_err(|_| Error::InvalidValue("task pool already running".to_string()))?;

        // initialize bash
        super::init()?;

        let mut sem = SharedSemaphore::new(self.jobs)?;
        let null = File::options().write(true).open("/dev/null")?;
        let nullfd = null.as_raw_fd();

        match unsafe { fork() } {
            Ok(ForkResult::Parent { .. }) => Ok(()),
            Ok(ForkResult::Child) => {
                // signal child to exit on parent death
                #[cfg(target_os = "linux")]
                prctl::set_pdeathsig(Signal::SIGTERM).unwrap();

                scallop::shell::fork_init();
                // enable internal bash SIGCHLD handler
                unsafe { scallop::bash::set_sigchld_handler() };

                // TODO: skip variables from allowed set
                // wipe external environment variables
                for (name, _value) in std::env::vars() {
                    std::env::remove_var(name);
                }

                while let Ok(Command::Task(task, pipes)) = self.rx.recv() {
                    // wait on bounded semaphore for pool space
                    sem.acquire().unwrap();
                    match unsafe { fork() } {
                        Ok(ForkResult::Parent { .. }) => (),
                        Ok(ForkResult::Child) => {
                            scallop::shell::fork_init();
                            task.run(config, pipes, nullfd);
                            sem.release().unwrap();
                            unsafe { libc::_exit(0) };
                        }
                        Err(e) => panic!("process pool fork failed: {e}"), // pragma: no cover
                    }
                }

                // wait for forked processes to complete
                sem.wait().unwrap();
                unsafe { libc::_exit(0) }
            }
            Err(e) => panic!("process pool failed start: {e}"), // pragma: no cover
        }
    }

    /// Create an ebuild package metadata regeneration task builder.
    pub fn metadata_task(&self, repo: &EbuildRepo) -> MetadataTaskBuilder {
        MetadataTaskBuilder::new(self, repo)
    }

    /// Run the pkg_pretend phase for an ebuild package.
    pub fn pretend<T: Into<Cpv>>(
        &self,
        repo: &EbuildRepo,
        cpv: T,
    ) -> crate::Result<Option<String>> {
        let task = PkgPretendTask::new(repo, cpv);
        let (tx, rx) = ipc::channel()
            .map_err(|e| Error::InvalidValue(format!("failed creating task channel: {e}")))?;
        let task = Task::PkgPretend(task, tx);
        self.tx
            .send(Command::Task(task, Default::default()))
            .map_err(|e| Error::InvalidValue(format!("failed queuing task: {e}")))?;
        rx.recv()
            .map_err(|e| Error::InvalidValue(format!("failed receiving task status: {e}")))?
    }

    /// Return the mapping of global environment variables exported by a package.
    pub fn source_env<T: Into<Cpv>>(
        &self,
        repo: &EbuildRepo,
        cpv: T,
    ) -> crate::Result<IndexMap<String, String>> {
        let task = SourceEnvTask::new(repo, cpv);
        let (tx, rx) = ipc::channel()
            .map_err(|e| Error::InvalidValue(format!("failed creating task channel: {e}")))?;
        let task = Task::SourceEnv(task, tx);
        self.tx
            .send(Command::Task(task, Default::default()))
            .map_err(|e| Error::InvalidValue(format!("failed queuing task: {e}")))?;
        rx.recv()
            .map_err(|e| Error::InvalidValue(format!("failed receiving task status: {e}")))?
    }
}

impl Drop for BuildPool {
    fn drop(&mut self) {
        self.tx.send(Command::Stop).ok();
    }
}
