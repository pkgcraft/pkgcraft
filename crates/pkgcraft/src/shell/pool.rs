use std::collections::HashSet;
use std::fs;
use std::io::stdout;
use std::marker::PhantomData;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use indexmap::IndexMap;
use ipc_channel::ipc::{self, IpcOneShotServer, IpcReceiver, IpcSender};
use itertools::Itertools;
use nix::sys::wait::waitpid;
use nix::unistd::{ForkResult, Pid, dup, dup2_stdout, fork};
use scallop::pool::{NamedSemaphore, redirect_output, suppress_output};
use scallop::variables::{self, ShellVariable};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::config::ConfigRepos;
use crate::dep::Cpv;
use crate::error::Error;
use crate::pkg::ebuild::metadata::Metadata;
use crate::pkg::{Package, PkgPretend, Source};
use crate::repo::EbuildRepo;
use crate::repo::Repository;
use crate::repo::ebuild::cache::{Cache, CacheEntry, MetadataCache};

use super::environment::{BASH, EXTERNAL};

/// Get an ebuild repo from a config matching a given ID.
fn get_ebuild_repo<'a>(repos: &'a ConfigRepos, repo: &str) -> crate::Result<&'a EbuildRepo> {
    repos
        .get(repo)?
        .as_ebuild()
        .ok_or_else(|| Error::InvalidValue(format!("non-ebuild repo: {repo}")))
}

/// Update an ebuild repo's package metadata cache for a given [`Cpv`].
#[derive(Debug, Serialize, Deserialize)]
struct MetadataTask {
    repo: String,
    cpv: Cpv,
    cache: MetadataCache,
    output: bool,
    verify: bool,
}

impl MetadataTask {
    fn new(
        repo: &EbuildRepo,
        cpv: Cpv,
        cache: MetadataCache,
        output: bool,
        verify: bool,
    ) -> Self {
        Self {
            repo: repo.id().to_string(),
            cpv,
            cache,
            output,
            verify,
        }
    }

    fn run(self, config: &ConfigRepos) -> crate::Result<Option<String>> {
        let repo = get_ebuild_repo(config, &self.repo)?;
        let pkg = repo.get_pkg_raw(self.cpv)?;

        // TODO: use a wrapper method to capture output
        // conditionally capture stdin and stderr
        let output = if self.output {
            let file = NamedTempFile::new()?;
            let fd = dup(stdout()).unwrap();
            redirect_output(&file)?;
            Some((file, fd))
        } else {
            None
        };

        let meta = Metadata::try_from(&pkg).map_err(|e| e.into_invalid_pkg_err(&pkg))?;

        // process captured output to send back to the main process
        let output = if let Some((file, fd)) = output {
            dup2_stdout(fd).unwrap();
            let data = fs::read_to_string(file.path()).unwrap_or_default();
            let data = data.trim();
            if !data.is_empty() {
                // indent output data and add package header
                let data = data.lines().join("\n  ");
                Some(format!("{pkg}:\n  {data}"))
            } else {
                None
            }
        } else {
            None
        };

        if !self.verify {
            self.cache.update(&pkg, &meta)?;
        }

        Ok(output)
    }
}

/// Task builder for ebuild package metadata cache generation.
#[derive(Debug)]
pub struct MetadataTaskBuilder {
    tx: IpcSender<Command>,
    repo: EbuildRepo,
    cache: Option<MetadataCache>,
    force: bool,
    output: bool,
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
        self.output = value;
        self
    }

    /// Verify the package's metadata.
    pub fn verify(mut self, value: bool) -> Self {
        self.verify = value;
        self
    }

    /// Run the task for a target [`Cpv`].
    pub fn run<T: Into<Cpv>>(&self, cpv: T) -> crate::Result<Option<String>> {
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
                    return result.and_then(|e| e.to_metadata(&pkg)).map(|_| None);
                } else if result.is_ok() {
                    // skip deserialization, assuming existing cache entry is valid
                    return Ok(None);
                }
            }
        }

        let task = MetadataTask::new(&self.repo, cpv, cache.clone(), self.output, self.verify);
        Command::run_task(&self.tx, task)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct PretendTask {
    repo: String,
    cpv: Cpv,
}

impl PretendTask {
    fn new<T: Into<Cpv>>(repo: &EbuildRepo, cpv: T) -> Self {
        Self {
            repo: repo.id().to_string(),
            cpv: cpv.into(),
        }
    }

    fn run(self, config: &ConfigRepos) -> crate::Result<Option<String>> {
        let repo = get_ebuild_repo(config, &self.repo)?;
        let pkg = repo.get_pkg(self.cpv)?;
        Ok(pkg.pkg_pretend()?)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct EnvTask {
    repo: String,
    cpv: Cpv,
}

impl EnvTask {
    fn new<T: Into<Cpv>>(repo: &EbuildRepo, cpv: T) -> Self {
        Self {
            repo: repo.id().to_string(),
            cpv: cpv.into(),
        }
    }

    fn run(self, config: &ConfigRepos) -> crate::Result<IndexMap<String, String>> {
        let repo = get_ebuild_repo(config, &self.repo)?;
        let pkg = repo.get_pkg_raw(self.cpv)?;
        let eapi_vars = pkg.eapi().env();
        let metadata_vars = pkg.eapi().metadata_keys();
        let skip: HashSet<_> = ["PIPESTATUS", "_"].into_iter().collect();
        pkg.source()?;
        Ok(variables::visible()
            .into_iter()
            .filter(|var| {
                let name = var.as_ref();
                eapi_vars.contains(name)
                    || metadata_vars.contains(name)
                    || (!skip.contains(name)
                        && !EXTERNAL.contains(name)
                        && !BASH.contains(name))
            })
            .filter_map(|var| var.to_vec().map(|v| (var.to_string(), v.join(" "))))
            .collect())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct DurationTask {
    repo: String,
    cpv: Cpv,
}

impl DurationTask {
    fn new<T: Into<Cpv>>(repo: &EbuildRepo, cpv: T) -> Self {
        Self {
            repo: repo.id().to_string(),
            cpv: cpv.into(),
        }
    }

    fn run(self, config: &ConfigRepos) -> crate::Result<Duration> {
        let repo = get_ebuild_repo(config, &self.repo)?;
        let pkg = repo.get_pkg_raw(self.cpv)?;
        let start = Instant::now();
        pkg.source()
            .map_err(|e| Error::from(e).into_invalid_pkg_err(&pkg))?;
        Ok(start.elapsed())
    }
}

/// Build pool task.
#[derive(Debug, Serialize, Deserialize)]
enum Task {
    Env(EnvTask, Sender<crate::Result<IndexMap<String, String>>>),
    Metadata(MetadataTask, Sender<crate::Result<Option<String>>>),
    Pretend(PretendTask, Sender<crate::Result<Option<String>>>),
    Duration(DurationTask, Sender<crate::Result<Duration>>),
}

/// Wrapper for sending task results via IpcOneShotServer.
#[derive(Debug, Serialize, Deserialize)]
struct Sender<T: Serialize + for<'a> Deserialize<'a>> {
    name: String,
    _ret: PhantomData<T>,
}

impl<T: Serialize + for<'a> Deserialize<'a>> Sender<T> {
    fn new(name: String) -> Self {
        Self { name, _ret: PhantomData }
    }

    fn send(self, value: T) {
        let tx = IpcSender::connect(self.name).expect("failed connecting to the server");
        tx.send(value).expect("failed sending task result")
    }
}

/// Convert a task into a Task variant.
trait IntoTask: Serialize + for<'a> Deserialize<'a> {
    type R: for<'a> Deserialize<'a> + Serialize;

    fn into_task(self, name: String) -> Task;

    /// Create an IPC sender that connects to a named IpcOneShotServer.
    fn sender(name: String) -> Sender<crate::Result<Self::R>> {
        Sender::new(name)
    }
}

impl IntoTask for EnvTask {
    type R = IndexMap<String, String>;
    fn into_task(self, name: String) -> Task {
        Task::Env(self, Self::sender(name))
    }
}
impl IntoTask for MetadataTask {
    type R = Option<String>;
    fn into_task(self, name: String) -> Task {
        Task::Metadata(self, Self::sender(name))
    }
}
impl IntoTask for PretendTask {
    type R = Option<String>;
    fn into_task(self, name: String) -> Task {
        Task::Pretend(self, Self::sender(name))
    }
}
impl IntoTask for DurationTask {
    type R = Duration;
    fn into_task(self, name: String) -> Task {
        Task::Duration(self, Self::sender(name))
    }
}

impl Task {
    /// Run the task, sending the result back to the main process.
    fn run(self, config: &ConfigRepos) {
        match self {
            Self::Env(task, tx) => tx.send(task.run(config)),
            Self::Metadata(task, tx) => tx.send(task.run(config)),
            Self::Pretend(task, tx) => tx.send(task.run(config)),
            Self::Duration(task, tx) => tx.send(task.run(config)),
        }
    }
}

/// Build pool command.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Serialize, Deserialize)]
enum Command {
    Task(Task),
    Stop,
}

impl Command {
    /// Run a task variant and return the result.
    fn run_task<T: IntoTask>(
        tx: &IpcSender<Command>,
        task: T,
    ) -> crate::Result<<T as IntoTask>::R> {
        let (server, name) = IpcOneShotServer::new()?;
        let task = Self::Task(task.into_task(name));
        tx.send(task).expect("failed queuing task");
        let (_, data) = server.accept().expect("failed receiving task result");
        data
    }
}

#[derive(Debug)]
pub struct BuildPool {
    jobs: usize,
    tx: IpcSender<Command>,
    rx: IpcReceiver<Command>,
    pid: OnceLock<Pid>,
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
            pid: OnceLock::new(),
        }
    }

    /// Start the build pool loop.
    pub(crate) fn start(&self, config: ConfigRepos) -> crate::Result<()> {
        if self.pid.get().is_some() {
            // task pool already running
            return Ok(());
        }

        match unsafe { fork() } {
            Ok(ForkResult::Parent { child }) => {
                self.pid.set(child).expect("task pool already running");
                Ok(())
            }
            Ok(ForkResult::Child) => {
                // signal child to exit on parent death
                #[cfg(target_os = "linux")]
                {
                    use nix::sys::{prctl, signal::Signal};
                    prctl::set_pdeathsig(Signal::SIGTERM).unwrap();
                }

                // initialize semaphore to track jobs
                let pid = std::process::id();
                let name = format!("/pkgcraft-task-pool-{pid}");
                let mut sem = NamedSemaphore::new(&name, self.jobs)?;

                // initialize bash
                super::init()?;

                // enable internal bash SIGCHLD handler
                unsafe { scallop::bash::set_sigchld_handler() };

                // suppress global output by default
                suppress_output()?;

                while let Ok(Command::Task(task)) = self.rx.recv() {
                    // wait on bounded semaphore for pool space
                    sem.acquire().unwrap();
                    match unsafe { fork() } {
                        Ok(ForkResult::Parent { .. }) => (),
                        Ok(ForkResult::Child) => {
                            scallop::shell::fork_init();
                            task.run(&config);
                            sem.release().unwrap();
                            unsafe { libc::_exit(0) };
                        }
                        Err(e) => panic!("process pool fork failed: {e}"), // grcov-excl-line
                    }
                }

                // wait for forked processes to complete
                sem.wait().unwrap();
                unsafe { libc::_exit(0) }
            }
            Err(e) => panic!("process pool failed start: {e}"), // grcov-excl-line
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
        let task = PretendTask::new(repo, cpv);
        Command::run_task(&self.tx, task)
    }

    /// Return the mapping of global environment variables exported by a package.
    pub fn env<T: Into<Cpv>>(
        &self,
        repo: &EbuildRepo,
        cpv: T,
    ) -> crate::Result<IndexMap<String, String>> {
        let task = EnvTask::new(repo, cpv);
        Command::run_task(&self.tx, task)
    }

    /// Return the time duration required to source a package.
    pub fn duration<T: Into<Cpv>>(
        &self,
        repo: &EbuildRepo,
        cpv: T,
    ) -> crate::Result<Duration> {
        let task = DurationTask::new(repo, cpv);
        Command::run_task(&self.tx, task)
    }
}

impl Drop for BuildPool {
    fn drop(&mut self) {
        self.tx.send(Command::Stop).ok();
        // TODO: consider combining with bash SIGCHLD handler
        if let Some(pid) = self.pid.get() {
            waitpid(*pid, None).ok();
        }
    }
}
