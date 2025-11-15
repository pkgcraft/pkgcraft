use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;

use camino::Utf8PathBuf;
use futures::FutureExt;
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::config::Config as PkgcraftConfig;
use pkgcraft::restrict::Restrict;
use pkgcruft::report::ReportLevel;
use pkgcruft::scan::Scanner;
use tempfile::{TempDir, tempdir};
use tokio::net::{TcpListener, UnixListener};
use tokio::sync::{Semaphore, mpsc, oneshot};
use tokio_stream::wrappers::{ReceiverStream, TcpListenerStream, UnixListenerStream};
use tonic::transport::Server;
use tonic::{Request, Response, Status};

use crate::proto::pkgcruft_server::{Pkgcruft, PkgcruftServer};
use crate::proto::{EmptyRequest, PushRequest, PushResponse, StringResponse};
use crate::uds::verify_socket_path;
use crate::{Error, git};

enum Listener {
    Tcp(TcpListener),
    Unix(UnixListener),
}

impl Listener {
    /// Try creating a new listener for the pkgcruft service.
    async fn try_new<S: AsRef<str>>(socket: S) -> crate::Result<Self> {
        let socket = socket.as_ref();
        let (socket, listener) = match socket.parse::<SocketAddr>() {
            Err(_) if socket.starts_with('/') => {
                verify_socket_path(socket)?;
                let listener = UnixListener::bind(socket).map_err(|e| {
                    Error::Start(format!("failed binding to socket: {socket}: {e}"))
                })?;
                (socket.to_string(), Listener::Unix(listener))
            }
            Err(_) => return Err(Error::InvalidValue(format!("invalid socket: {socket}"))),
            Ok(socket) => {
                let listener = TcpListener::bind(&socket).await.map_err(|e| {
                    Error::Start(format!("failed binding to socket: {socket}: {e}"))
                })?;
                let addr = listener.local_addr().map_err(|e| {
                    Error::Start(format!("invalid local address: {socket}: {e}"))
                })?;
                (addr.to_string(), Listener::Tcp(listener))
            }
        };

        tracing::info!("service listening at: {socket}");
        Ok(listener)
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
    pub fn new(uri: &str) -> Self {
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

    /// Start the service, waiting for it to finish.
    pub async fn start(self) -> crate::Result<Pkgcruftd> {
        // determine network socket
        let socket = if let Some(value) = self.socket {
            value
        } else {
            // default to using UNIX domain socket for the executing user
            let config = PkgcraftConfig::new("pkgcraft", "");
            config.path().run.join("pkgcruft.sock").to_string()
        };

        let service = PkgcruftService::try_new(self.uri, self.temp, self.jobs)?;
        let server = Server::builder().add_service(PkgcruftServer::new(service));

        let listener = Listener::try_new(socket).await?;
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
}

/// Pkgcruft service wrapper that forces the service to end when dropped.
pub struct Pkgcruftd {
    _tx: oneshot::Sender<()>,
}

struct PkgcruftService {
    _tempdir: Option<TempDir>,
    path: Utf8PathBuf,
    scanning: Arc<Semaphore>,
    jobs: usize,
}

impl PkgcruftService {
    /// Try creating a new service.
    fn try_new(uri: String, temp: bool, jobs: usize) -> crate::Result<Self> {
        let mut _tempdir = None;
        let path = if temp {
            // create temporary git repo dir
            let tempdir = tempdir()
                .map_err(|e| Error::Start(format!("failed creating temp dir: {e}")))?;
            let path = Utf8PathBuf::from_path_buf(tempdir.path().to_owned())
                .map_err(|p| Error::Start(format!("invalid tempdir path: {p:?}")))?;
            _tempdir = Some(tempdir);

            // clone git repo into temporary dir
            git::clone(&uri, &path)
                .map_err(|e| Error::Start(format!("failed cloning git repo: {uri}: {e}")))?;

            path
        } else {
            uri.into()
        };

        // verify target path is a valid ebuild repo
        let mut config = PkgcraftConfig::new("pkgcraft", "");
        let repo = config
            .add_repo_path("repo", &path, 0)
            .map_err(|e| Error::Start(format!("invalid repo: {e}")))?;
        let repo = repo
            .into_ebuild()
            .map_err(|e| Error::Start(format!("invalid ebuild repo: {path}: {e}")))?;
        config
            .finalize()
            .map_err(|e| Error::Start(format!("failed finalizing config: {e}")))?;

        // generate ebuild repo metadata ignoring failures
        repo.metadata()
            .cache()
            .regen(&repo)
            .progress(true)
            .run()
            .ok();

        // TODO: generate or verify db of existing pkgcruft reports

        Ok(Self {
            _tempdir,
            path,
            scanning: Arc::new(Semaphore::new(1)),
            jobs,
        })
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

        // determine diff
        let diff = git::diff(git_repo, &push.old_ref, &push.new_ref)?;

        // initialize ebuild repo
        let mut config = PkgcraftConfig::new("pkgcraft", "");
        let repo = config.add_repo_path("repo", &self.path, 0)?;
        let repo = repo
            .into_ebuild()
            .map_err(|e| Error::InvalidValue(format!("invalid ebuild repo: {e}")))?;
        config.finalize()?;

        // determine target Cpns from diff
        let mut cpns = IndexSet::new();
        let mut eclass = false;
        for delta in diff.deltas() {
            if let Some(path) = delta.new_file().path() {
                if let Ok(cpn) = repo.cpn_from_path(path) {
                    cpns.insert(cpn);
                } else if path.starts_with("eclass") {
                    eclass = true;
                }
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
        // TODO: use try_acquire_owned() with custom timeout
        // acquire exclusive scanning permission
        let permit = self.scanning.clone().acquire_owned().await.unwrap();

        // TODO: partially reload repo or reset lazy metadata fields
        let mut config = PkgcraftConfig::new("pkgcraft", "");
        let repo = config
            .add_repo_path("repo", &self.path, 0)
            .map_err(|e| Status::from_error(Box::new(e)))?;
        let repo = repo
            .into_ebuild()
            .map_err(|e| Status::invalid_argument(format!("invalid ebuild repo: {e}")))?;
        config
            .finalize()
            .map_err(|e| Status::from_error(Box::new(e)))?;

        // TODO: process request data into a restrict target
        let scanner = Scanner::new().jobs(self.jobs);
        let reports = scanner
            .run(&repo, repo.path())
            .map_err(|e| Status::from_error(Box::new(e)))?;

        let (tx, rx) = mpsc::channel(4);

        tokio::spawn(async move {
            for report in reports {
                if tx.send(Ok(report.into())).await.is_err() {
                    break;
                }
            }

            // explicitly own until scanning is finished
            drop(permit);
            drop(scanner);
            drop(repo);
            drop(config);
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn push(
        &self,
        request: Request<PushRequest>,
    ) -> Result<Response<PushResponse>, Status> {
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

        let git_repo =
            git2::Repository::open(&self.path).map_err(|e| Status::from_error(Box::new(e)))?;

        // run targeted pkgcruft scanning
        let result = self.handle_push(&git_repo, &push);

        // reset HEAD on error or failure
        if result.is_err() || result.as_ref().map(|r| r.failed).unwrap_or_default() {
            // reset reference and HEAD
            let old_oid: git2::Oid = push
                .old_ref
                .parse()
                .map_err(|e| Status::from_error(Box::new(e)))?;
            git_repo
                .find_reference(&push.ref_name)
                .map_err(|e| Status::from_error(Box::new(e)))?
                .set_target(old_oid, "")
                .map_err(|e| Status::from_error(Box::new(e)))?;
            git_repo
                .set_head(&push.ref_name)
                .map_err(|e| Status::from_error(Box::new(e)))?;

            // asynchronously revert working tree and index
            tokio::spawn(async move {
                git_repo
                    .checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
                    .ok();

                // explicitly own until repo mangling is finished
                drop(permit);
                drop(git_repo);
            });
        }

        match result {
            Ok(reply) => Ok(Response::new(reply)),
            Err(e) => Err(Status::from_error(Box::new(e))),
        }
    }
}
