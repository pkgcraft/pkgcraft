use ipc_channel::ipc::{self, IpcSender};
use nix::unistd::{fork, ForkResult};
use scallop::pool::{suppress_output, SharedSemaphore};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::dep::Cpv;
use crate::error::PackageError;
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

impl Cmd {
    fn run(self, config: &Config) -> crate::Result<()> {
        match self {
            Self::Metadata(repo_id, cpv, verify) => {
                let repo = config.repos.get(&repo_id).unwrap().as_ebuild().unwrap();
                let pkg = EbuildRawPkg::try_new(cpv, repo.clone())?;
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

#[derive(Debug, Clone)]
pub struct BuildPool {
    tx: IpcSender<Msg>,
}

unsafe impl Sync for BuildPool {}

impl BuildPool {
    pub(crate) fn new(config: &Config, jobs: usize) -> Self {
        let config = config.clone();
        let mut sem = SharedSemaphore::new(jobs).unwrap();
        let (tx, rx) = ipc::channel().unwrap();

        match unsafe { fork() } {
            Ok(ForkResult::Parent { .. }) => (),
            Ok(ForkResult::Child) => {
                scallop::shell::fork_init();
                // enable internal bash SIGCHLD handler
                unsafe { scallop::bash::set_sigchld_handler() };

                // suppress stdout and stderr in forked processes
                suppress_output().unwrap();

                while let Ok(Msg::Cmd(cmd, result_tx)) = rx.recv() {
                    // wait on bounded semaphore for pool space
                    sem.acquire().unwrap();
                    match unsafe { fork() } {
                        Ok(ForkResult::Parent { .. }) => (),
                        Ok(ForkResult::Child) => {
                            scallop::shell::fork_init();
                            let result = cmd.run(&config);
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
        };

        Self { tx }
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
            let pkg = EbuildRawPkg::try_new(cpv.clone(), repo.clone())?;
            if repo.metadata().cache().get(&pkg).is_ok() {
                return Ok(());
            }
        }
        let cmd = Cmd::Metadata(repo.id().to_string(), cpv.clone(), verify);
        let (tx, rx) = ipc::channel().unwrap();
        self.tx.send(Msg::Cmd(cmd, tx)).unwrap();
        rx.recv().unwrap()
    }
}

impl Drop for BuildPool {
    fn drop(&mut self) {
        self.tx.send(Msg::Stop).ok();
    }
}
