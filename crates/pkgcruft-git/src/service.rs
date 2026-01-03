use std::fs;
use std::io::Write;
use std::net::SocketAddr;
use std::os::unix::net::UnixStream;
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use futures::FutureExt;
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::config::Config as PkgcraftConfig;
use pkgcraft::repo::RepoFormat;
use pkgcraft::restrict::Restrict;
use pkgcruft::report::ReportLevel;
use pkgcruft::scan::Scanner;
use tempfile::{TempDir, tempdir};
use tokio::net::{TcpListener, UnixListener};
use tokio::sync::{Semaphore, mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio_stream::wrappers::{ReceiverStream, TcpListenerStream, UnixListenerStream};
use tonic::transport::Server;
use tonic::{Request, Response, Status};

use crate::proto::pkgcruft_server::{Pkgcruft, PkgcruftServer};
use crate::proto::{EmptyRequest, PushRequest, PushResponse, StringResponse};
use crate::{Error, git};

enum Listener {
    Tcp(TcpListener),
    Unix(UnixListener),
}

impl Listener {
    /// Return a UnixListener for a valid domain socket.
    fn unix_listener<P: AsRef<Utf8Path>>(path: P) -> crate::Result<UnixListener> {
        let path = path.as_ref();
        let socket_dir = &path
            .parent()
            .ok_or_else(|| Error::InvalidValue(format!("invalid socket: {path}")))?;

        // check if the socket is already in use
        if UnixStream::connect(path).is_ok() {
            return Err(Error::InvalidValue(format!("service already running: {path}")));
        }

        // create dirs and remove old socket file if it exists
        fs::create_dir_all(socket_dir).map_err(|e| {
            Error::InvalidValue(format!("failed creating socket dir: {socket_dir}: {e}"))
        })?;
        fs::remove_file(path).ok();

        UnixListener::bind(path)
            .map_err(|e| Error::Service(format!("failed binding to socket: {path}: {e}")))
    }

    /// Try creating a new listener for the pkgcruft service.
    async fn try_new<S: AsRef<str>>(socket: S) -> crate::Result<(String, Self)> {
        let socket = socket.as_ref();
        if let Ok(socket) = socket.parse::<SocketAddr>() {
            let listener = TcpListener::bind(&socket).await.map_err(|e| {
                Error::Service(format!("failed binding to socket: {socket}: {e}"))
            })?;
            let addr = listener
                .local_addr()
                .unwrap_or_else(|e| unreachable!("invalid socket: {socket}: {e}"));
            Ok((addr.to_string(), Listener::Tcp(listener)))
        } else if socket.starts_with('/') {
            let listener = Self::unix_listener(socket)?;
            Ok((socket.to_string(), Listener::Unix(listener)))
        } else {
            Err(Error::InvalidValue(format!("invalid socket: {socket}")))
        }
    }
}

pub struct PkgcruftServiceBuilder {
    uri: String,
    socket: Option<String>,
    jobs: usize,
    temp: bool,
}

impl PkgcruftServiceBuilder {
    /// Create a new service builder.
    pub fn new<S: ToString>(uri: S) -> Self {
        Self {
            uri: uri.to_string(),
            socket: None,
            jobs: num_cpus::get(),
            temp: false,
        }
    }

    /// Set the network socket to bind.
    pub fn socket<S: Into<String>>(mut self, socket: S) -> Self {
        self.socket = Some(socket.into());
        self
    }

    /// Set the number of jobs to run.
    pub fn jobs(mut self, value: usize) -> Self {
        self.jobs = value;
        self
    }

    /// Use a temporary directory for the git repo.
    pub fn temp(mut self, value: bool) -> Self {
        self.temp = value;
        self
    }

    /// Create a PkgcruftService from the builder.
    pub fn build(self) -> crate::Result<PkgcruftService> {
        let mut _tempdir = None;
        let path = if self.temp {
            // create temporary git repo dir
            let tempdir = tempdir()
                .map_err(|e| Error::Service(format!("failed creating temp dir: {e}")))?;
            let path = Utf8PathBuf::from_path_buf(tempdir.path().to_owned())
                .map_err(|p| Error::Service(format!("invalid tempdir path: {p:?}")))?;
            _tempdir = Some(tempdir);

            // clone git repo into temporary dir
            let uri = &self.uri;
            git::clone(uri, &path)
                .map_err(|e| Error::Service(format!("failed cloning git repo: {uri}: {e}")))?;

            path
        } else {
            self.uri.into()
        };

        // verify target path is a valid ebuild repo
        let mut config = PkgcraftConfig::new("pkgcraft", "");
        let repo = config
            .add_repo_path("repo", &path, 0)
            .map_err(|e| Error::Service(format!("invalid repo: {e}")))?;
        let repo = repo
            .into_ebuild()
            .map_err(|e| Error::Service(format!("invalid ebuild repo: {path}: {e}")))?;
        config
            .finalize()
            .map_err(|e| Error::Service(format!("failed finalizing config: {e}")))?;

        // generate ebuild repo metadata ignoring failures
        repo.metadata()
            .cache()
            .regen(&repo)
            .progress(true)
            .run()
            .ok();

        // TODO: generate or verify db of existing pkgcruft reports

        Ok(PkgcruftService {
            _tempdir,
            path,
            scanning: Arc::new(Semaphore::new(1)),
            jobs: self.jobs,
            socket: self.socket,
        })
    }
}

/// Pkgcruft service spawned into a tokio task providing socket access for tests.
#[derive(Debug)]
pub struct PkgcruftdTask {
    pub socket: String,
    _service: JoinHandle<crate::Result<Pkgcruftd>>,
}

/// Pkgcruft service wrapper that forces the service to end when dropped.
#[derive(Debug)]
pub struct Pkgcruftd {
    _tx: oneshot::Sender<()>,
}

/// Pkgcruft service.
#[derive(Debug)]
pub struct PkgcruftService {
    _tempdir: Option<TempDir>,
    path: Utf8PathBuf,
    scanning: Arc<Semaphore>,
    jobs: usize,
    socket: Option<String>,
}

impl PkgcruftService {
    /// Create a network listener for the service.
    async fn create_listener(&self) -> crate::Result<(String, Listener)> {
        // determine network socket
        let socket = if let Some(value) = &self.socket {
            value.to_string()
        } else {
            // default to using UNIX domain socket for the executing user
            let config = PkgcraftConfig::new("pkgcraft", "");
            config.path().run.join("pkgcruft-gitd.sock").to_string()
        };
        Listener::try_new(&socket).await
    }

    /// Start the service listening on the given Listener.
    async fn listen(self, listener: Listener) -> crate::Result<Pkgcruftd> {
        let server = Server::builder().add_service(PkgcruftServer::new(self));

        let (tx, rx) = oneshot::channel::<()>();
        match listener {
            Listener::Unix(listener) => {
                server
                    .serve_with_incoming_shutdown(
                        UnixListenerStream::new(listener),
                        rx.map(drop),
                    )
                    .await
            }
            Listener::Tcp(listener) => {
                server
                    .serve_with_incoming_shutdown(
                        TcpListenerStream::new(listener),
                        rx.map(drop),
                    )
                    .await
            }
        }
        .map_err(|e| Error::Service(e.to_string()))?;

        Ok(Pkgcruftd { _tx: tx })
    }

    /// Start the service.
    pub async fn start(self) -> crate::Result<Pkgcruftd> {
        let (socket, listener) = self.create_listener().await?;
        tracing::info!("service listening at: {socket}");
        self.listen(listener).await
    }

    /// Spawn the service in a tokio task.
    pub async fn spawn(self) -> crate::Result<PkgcruftdTask> {
        let (socket, listener) = self.create_listener().await?;
        let _service = tokio::spawn(async move { self.listen(listener).await });
        Ok(PkgcruftdTask { socket, _service })
    }

    /// Perform a scanning run for a push request.
    fn handle_push(
        &self,
        git_repo: &git2::Repository,
        push: &PushRequest,
    ) -> crate::Result<PushResponse> {
        // write pack file to odb
        let odb = git_repo.odb()?;
        let mut pack_writer = odb.packwriter()?;
        pack_writer
            .write_all(&push.pack)
            .map_err(|e| Error::IO(format!("failed writing pack file: {e}")))?;
        pack_writer
            .flush()
            .map_err(|e| Error::IO(format!("failed flushing pack file: {e}")))?;
        pack_writer.commit()?;

        // determine target commit
        let ref_name = &push.ref_name;
        let old_oid: git2::Oid = push.old_ref.parse()?;
        let new_oid: git2::Oid = push.new_ref.parse()?;
        let commit = git_repo.find_annotated_commit(new_oid)?;

        // update target reference for unborn or fast-forward merge variants
        let (analysis, _prefs) = git_repo.merge_analysis(&[&commit])?;
        if analysis.is_unborn() {
            let msg = format!("unborn: setting {ref_name}: {new_oid}");
            git_repo.reference("HEAD", new_oid, false, &msg)?;
        } else if analysis.is_fast_forward() {
            // verify HEAD points to the expected commit
            let head = git_repo.head()?;
            let head_oid = head.peel_to_commit()?.id();
            if head_oid != old_oid {
                return Err(Error::InvalidValue(format!("invalid git repo HEAD: {head_oid}")));
            }

            // update target reference
            let msg = format!("fast-forward: setting {ref_name}: {new_oid}");
            git_repo
                .find_reference(ref_name)?
                .set_target(new_oid, &msg)?;
        } else {
            return Err(Error::InvalidValue(format!("non-fast-forward merge: {analysis:?}")));
        }

        // update HEAD for target reference
        git_repo.set_head(ref_name)?;
        git_repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;

        // determine changed paths from diff
        let diff = git::diff(git_repo, &push.old_ref, &push.new_ref)?;
        let paths = diff.deltas().filter_map(|d| d.new_file().path());

        // initialize ebuild repo
        let mut config = PkgcraftConfig::new("pkgcraft", "");
        let repo = config.add_format_repo_path("repo", &self.path, 0, RepoFormat::Ebuild)?;
        let repo = repo.into_ebuild().expect("invalid ebuild repo");
        config.finalize()?;

        // determine target Cpns from diff
        let mut cpns = IndexSet::new();
        let mut eclass = false;
        for path in paths {
            if let Ok(cpn) = repo.cpn_from_path(path) {
                cpns.insert(cpn);
            } else if path.starts_with("eclass") {
                eclass = true;
            }
        }

        let mut reports = IndexSet::new();

        // scan individual packages that were changed
        let mut scanner = Scanner::new()
            .jobs(self.jobs)
            .exit([ReportLevel::Critical, ReportLevel::Error]);
        for cpn in cpns {
            let reports_iter = scanner.run(&repo, &cpn)?;
            reports.extend(reports_iter.into_iter().map(|r| r.to_json()));
        }

        // scan full tree for metadata errors on eclass changes
        if eclass {
            scanner = scanner.reports([pkgcruft::check::CheckKind::Metadata]);
            let reports_iter = scanner.run(&repo, Restrict::True)?;
            reports.extend(reports_iter.into_iter().map(|r| r.to_json()));
        }

        Ok(PushResponse {
            reports: reports.into_iter().sorted().collect(),
            failed: scanner.failed(),
        })
    }
}

#[tonic::async_trait]
impl Pkgcruft for PkgcruftService {
    async fn version(
        &self,
        _request: Request<EmptyRequest>,
    ) -> Result<Response<StringResponse>, Status> {
        let data = env!("CARGO_PKG_VERSION").to_string();
        let reply = StringResponse { data };
        Ok(Response::new(reply))
    }

    type ScanStream = ReceiverStream<Result<StringResponse, Status>>;

    async fn scan(
        &self,
        _request: Request<EmptyRequest>,
    ) -> Result<Response<Self::ScanStream>, Status> {
        let scan = async || -> crate::Result<Self::ScanStream> {
            // TODO: use try_acquire_owned() with custom timeout
            // acquire exclusive scanning permission
            let permit = self.scanning.clone().acquire_owned().await.unwrap();

            // TODO: partially reload repo or reset lazy metadata fields
            let mut config = PkgcraftConfig::new("pkgcraft", "");
            let repo =
                config.add_format_repo_path("repo", &self.path, 0, RepoFormat::Ebuild)?;
            let repo = repo.into_ebuild().expect("invalid ebuild repo");
            config.finalize()?;

            // TODO: process request data into a restrict target
            let scanner = Scanner::new().jobs(self.jobs);
            let reports = scanner.run(&repo, repo.path())?;

            let (tx, rx) = mpsc::channel(4);

            tokio::spawn(async move {
                for report in reports {
                    if tx.send(Ok(report.into())).await.is_err() {
                        break;
                    }
                }

                // explicitly own until scanning is finished
                drop(scanner);
                drop(repo);
                drop(config);
                drop(permit);
            });

            Ok(ReceiverStream::new(rx))
        };

        scan()
            .await
            .map(Response::new)
            .map_err(|e| Status::from_error(Box::new(e)))
    }

    async fn push(
        &self,
        request: Request<PushRequest>,
    ) -> Result<Response<PushResponse>, Status> {
        let push = async || {
            // TODO: use try_acquire_owned() with custom timeout
            // acquire exclusive scanning permission
            let permit = self.scanning.clone().acquire_owned().await.unwrap();

            let push = request.into_inner();
            let record = indoc::formatdoc! {"
                scanning push:
                old ref: {}
                new ref: {}
                ref name: {}
            ", push.old_ref, push.new_ref, push.ref_name};
            tracing::info!("{record}");

            let git_repo = git2::Repository::open(&self.path)?;

            // run targeted pkgcruft scanning
            let result = self.handle_push(&git_repo, &push);

            // reset HEAD on error or failure
            if result.is_err() || result.as_ref().map(|r| r.failed).unwrap_or_default() {
                // reset reference and HEAD
                let old_oid: git2::Oid = push.old_ref.parse()?;
                git_repo
                    .find_reference(&push.ref_name)?
                    .set_target(old_oid, "")?;
                git_repo.set_head(&push.ref_name)?;

                // asynchronously revert working tree and index
                tokio::spawn(async move {
                    git_repo
                        .checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
                        .ok();

                    // explicitly own until repo mangling is finished
                    drop(git_repo);
                    drop(permit);
                });
            }

            result
        };

        push()
            .await
            .map(Response::new)
            .map_err(|e| Status::from_error(Box::new(e)))
    }
}
