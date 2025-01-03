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
    fn new(repo: &EbuildRepo, cpv: &Cpv, cache: &MetadataCache, verify: bool) -> Self {
        Self {
            repo: repo.id().to_string(),
            cpv: cpv.clone(),
            cache: cache.clone(),
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

    /// Update an ebuild repo's package metadata cache for a given [`Cpv`].
    pub fn metadata(
        &self,
        repo: &EbuildRepo,
        cpv: &Cpv,
        cache: &MetadataCache,
        force: bool,
        verify: bool,
    ) -> crate::Result<()> {
        if !force {
            let pkg = EbuildRawPkg::try_new(cpv.clone(), repo)?;
            if let Some(result) = cache.get(&pkg) {
                if verify {
                    // perform deserialization, returning any occurring error
                    return result.and_then(|e| e.to_metadata(&pkg)).map(|_| ());
                } else if result.is_ok() {
                    // skip deserialization, assuming existing cache entry is valid
                    return Ok(());
                }
            }
        }
        let meta = MetadataTask::new(repo, cpv, cache, verify);
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

impl Drop for BuildPool {
    fn drop(&mut self) {
        self.tx.send(Command::Stop).ok();
    }
}
