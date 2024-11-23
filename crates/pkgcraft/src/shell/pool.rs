use std::sync::OnceLock;

use ipc_channel::ipc::{self, IpcReceiver, IpcSender};
use nix::sys::{prctl, signal::Signal};
use nix::unistd::{fork, ForkResult};
use scallop::pool::{suppress_output, SharedSemaphore};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::dep::Cpv;
use crate::error::{Error, PackageError};
use crate::pkg::ebuild::metadata::Metadata as PkgMetadata;
use crate::pkg::ebuild::EbuildRawPkg;
use crate::repo::ebuild::cache::Cache;
use crate::repo::ebuild::EbuildRepo;
use crate::repo::Repository;

#[derive(Debug, Serialize, Deserialize)]
enum Cmd {
    /// Update an ebuild repo's package metadata cache for a given [`Cpv`].
    Metadata(String, Cpv, bool),
}

/// Get an ebuild repo from a config matching a given ID.
fn get_ebuild_repo(config: &Config, repo_id: String) -> crate::Result<&EbuildRepo> {
    config
        .repos
        .get(&repo_id)
        .and_then(|r| r.as_ebuild())
        .ok_or_else(|| Error::InvalidValue(format!("unknown ebuild repo: {repo_id}")))
}

impl Cmd {
    fn run(self, config: &Config) -> crate::Result<()> {
        match self {
            Self::Metadata(repo_id, cpv, verify) => {
                let repo = get_ebuild_repo(config, repo_id)?;
                let pkg = EbuildRawPkg::try_new(cpv, repo)?;
                let meta = PkgMetadata::try_from(&pkg).map_err(|e| pkg.invalid_pkg_err(e))?;
                if !verify {
                    repo.metadata().cache().update(&pkg, &meta)?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
enum Msg {
    Cmd(Cmd, IpcSender<crate::Result<()>>),
    Stop,
}

#[derive(Debug)]
pub struct BuildPool {
    jobs: usize,
    tx: IpcSender<Msg>,
    rx: IpcReceiver<Msg>,
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
                prctl::set_pdeathsig(Signal::SIGTERM).unwrap();

                scallop::shell::fork_init();
                // enable internal bash SIGCHLD handler
                unsafe { scallop::bash::set_sigchld_handler() };

                // suppress stdout and stderr in forked processes
                suppress_output().unwrap();

                while let Ok(Msg::Cmd(cmd, result_tx)) = self.rx.recv() {
                    // wait on bounded semaphore for pool space
                    sem.acquire().unwrap();
                    match unsafe { fork() } {
                        Ok(ForkResult::Parent { .. }) => (),
                        Ok(ForkResult::Child) => {
                            scallop::shell::fork_init();
                            let result = cmd.run(config);
                            result_tx.send(result).unwrap();
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
        force: bool,
        verify: bool,
    ) -> crate::Result<()> {
        if !force {
            let pkg = EbuildRawPkg::try_new(cpv.clone(), repo)?;
            if repo.metadata().cache().get(&pkg).is_ok() {
                return Ok(());
            }
        }
        let cmd = Cmd::Metadata(repo.id().to_string(), cpv.clone(), verify);
        let (tx, rx) = ipc::channel().expect("failed creating IPC task channel");
        self.tx
            .send(Msg::Cmd(cmd, tx))
            .expect("failed queuing task");
        rx.recv().expect("failed receiving task status")
    }
}

impl Drop for BuildPool {
    fn drop(&mut self) {
        self.tx.send(Msg::Stop).ok();
    }
}
