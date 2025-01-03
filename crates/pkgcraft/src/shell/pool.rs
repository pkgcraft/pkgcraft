use std::sync::OnceLock;

use ipc_channel::ipc::{self, IpcReceiver, IpcSender};
use nix::sys::{prctl, signal::Signal};
use nix::unistd::{fork, ForkResult};
use scallop::pool::{suppress_output, SharedSemaphore};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::dep::Cpv;
use crate::error::Error;
use crate::pkg::ebuild::metadata::Metadata;
use crate::pkg::ebuild::EbuildRawPkg;
use crate::repo::ebuild::cache::{Cache, CacheEntry, MetadataCache};
use crate::repo::ebuild::EbuildRepo;
use crate::repo::Repository;

/// Get an ebuild repo from a config matching a given ID.
fn get_ebuild_repo(config: &Config, repo: String) -> crate::Result<&EbuildRepo> {
    config
        .repos
        .get(&repo)
        .and_then(|r| r.as_ebuild())
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
        let repo = get_ebuild_repo(config, self.repo)?;
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
            cache: None,
            force: false,
            verify: false,
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
            let pkg = EbuildRawPkg::try_new(cpv.clone(), &self.repo)?;
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
            .send(Command::Task(task))
            .map_err(|e| Error::InvalidValue(format!("failed queuing task: {e}")))?;
        rx.recv()
            .map_err(|e| Error::InvalidValue(format!("failed receiving task status: {e}")))?
    }
}

/// Build pool task.
#[derive(Debug, Serialize, Deserialize)]
enum Task {
    Metadata(MetadataTask, IpcSender<crate::Result<()>>),
}

impl Task {
    fn run(self, config: &Config) {
        match self {
            Self::Metadata(task, tx) => {
                let result = task.run(config);
                tx.send(result).unwrap();
            }
        }
    }
}

/// Build pool command.
#[derive(Debug, Serialize, Deserialize)]
enum Command {
    Task(Task),
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

    /// Return true if the build pool is running, false otherwise.
    pub fn running(&self) -> bool {
        self.running.get().is_some()
    }

    /// Start the build pool loop.
    pub(crate) fn start(&self, config: &Config) {
        self.running.get_or_init(|| true);
        let mut sem = SharedSemaphore::new(self.jobs).unwrap();

        match unsafe { fork() } {
            Ok(ForkResult::Parent { .. }) => (),
            Ok(ForkResult::Child) => {
                // signal child to exit on parent death
                #[cfg(target_os = "linux")]
                prctl::set_pdeathsig(Signal::SIGTERM).unwrap();

                scallop::shell::fork_init();
                // enable internal bash SIGCHLD handler
                unsafe { scallop::bash::set_sigchld_handler() };

                // suppress stdout and stderr in forked processes
                suppress_output().unwrap();

                while let Ok(Command::Task(task)) = self.rx.recv() {
                    // wait on bounded semaphore for pool space
                    sem.acquire().unwrap();
                    match unsafe { fork() } {
                        Ok(ForkResult::Parent { .. }) => (),
                        Ok(ForkResult::Child) => {
                            scallop::shell::fork_init();
                            task.run(config);
                            sem.release().unwrap();
                            unsafe { libc::_exit(0) };
                        }
                        Err(e) => panic!("process pool fork failed: {e}"),
                    }
                }

                // wait for forked processes to complete
                sem.wait().unwrap();
                unsafe { libc::_exit(0) }
            }
            Err(e) => panic!("process pool failed start: {e}"),
        }
    }

    /// Create an ebuild package metadata regeneration task builder.
    pub fn metadata_task(&self, repo: &EbuildRepo) -> MetadataTaskBuilder {
        MetadataTaskBuilder::new(self, repo)
    }
}

impl Drop for BuildPool {
    fn drop(&mut self) {
        self.tx.send(Command::Stop).ok();
    }
}
