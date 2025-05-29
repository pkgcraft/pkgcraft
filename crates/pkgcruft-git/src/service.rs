use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexSet;
use pkgcraft::config::Config as PkgcraftConfig;
use pkgcraft::restrict::Restrict;
use pkgcruft::report::ReportLevel;
use pkgcruft::scan::Scanner;
use tempfile::{TempDir, tempdir};
use tokio::net::{TcpListener, UnixListener};
use tokio::sync::{Semaphore, mpsc};
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
}

impl PkgcruftServiceBuilder {
    /// Create a new service builder.
    pub fn new(uri: &str) -> Self {
        Self {
            uri: uri.to_string(),
            socket: None,
        }
    }

    /// Set the network socket to bind.
    pub fn socket<S: Into<String>>(mut self, socket: S) -> Self {
        self.socket = Some(socket.into());
        self
    }

    /// Start the service, waiting for it to finish.
    pub async fn start(self) -> crate::Result<()> {
        // determine network socket
        let socket = if let Some(value) = self.socket {
            value
        } else {
            // default to using UNIX domain socket for the executing user
            let config = PkgcraftConfig::new("pkgcraft", "");
            config.path().run.join("pkgcruft.sock").to_string()
        };

        let service = PkgcruftService::try_new(self.uri)?;
        let server = Server::builder().add_service(PkgcruftServer::new(service));

        let listener = Listener::try_new(socket).await?;
        match listener {
            Listener::Unix(listener) => {
                server
                    .serve_with_incoming(UnixListenerStream::new(listener))
                    .await
            }
            Listener::Tcp(listener) => {
                server
                    .serve_with_incoming(TcpListenerStream::new(listener))
                    .await
            }
        }
        .map_err(|e| Error::Service(e.to_string()))
    }
}

struct PkgcruftService {
    _tempdir: TempDir,
    path: Utf8PathBuf,
    scanning: Arc<Semaphore>,
}

impl PkgcruftService {
    /// Try creating a new service.
    fn try_new(uri: String) -> crate::Result<Self> {
        let _tempdir =
            tempdir().map_err(|e| Error::Start(format!("failed creating temp dir: {e}")))?;
        let path = Utf8PathBuf::from_path_buf(_tempdir.path().to_owned())
            .map_err(|p| Error::Start(format!("invalid tempdir path: {p:?}")))?;

        // clone target git repo into temporary dir
        git::clone(&uri, &path)
            .map_err(|e| Error::Start(format!("failed cloning git repo: {uri}: {e}")))?;

        // verify target path is a valid ebuild repo
        let mut config = PkgcraftConfig::new("pkgcraft", "");
        let _ = config
            .add_repo_path("repo", &path, 0)
            .map(|r| r.into_ebuild())
            .map_err(|e| Error::Start(format!("invalid ebuild repo: {path}: {e}")))?;

        Ok(Self {
            _tempdir,
            path,
            scanning: Arc::new(Semaphore::new(1)),
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
        let scanner = Scanner::new();
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
        let _permit = self.scanning.clone().acquire_owned().await.unwrap();

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

        // write pack file to odb
        let odb = git_repo
            .odb()
            .map_err(|e| Status::from_error(Box::new(e)))?;
        let mut pack_writer = odb
            .packwriter()
            .map_err(|e| Status::from_error(Box::new(e)))?;
        pack_writer
            .write_all(&push.pack)
            .map_err(|e| Status::from_error(Box::new(e)))?;
        pack_writer
            .flush()
            .map_err(|e| Status::from_error(Box::new(e)))?;
        pack_writer
            .commit()
            .map_err(|e| Status::from_error(Box::new(e)))?;

        // merge incoming commits to HEAD
        let new_oid: git2::Oid = push
            .new_ref
            .parse()
            .map_err(|e| Status::from_error(Box::new(e)))?;
        let commit = git_repo
            .find_annotated_commit(new_oid)
            .map_err(|e| Status::from_error(Box::new(e)))?;
        git_repo
            .merge(&[&commit], None, None)
            .map_err(|e| Status::from_error(Box::new(e)))?;

        // determine diff
        let diff = git::diff(&git_repo, &push.old_ref, &push.new_ref)
            .map_err(|e| Status::from_error(Box::new(e)))?;

        // initialize ebuild repo
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

        // determine target Cpns from diff
        let mut cpns = IndexSet::new();
        let mut eclass = false;
        for delta in diff.deltas() {
            if let Some(path) = delta.new_file().path().and_then(Utf8Path::from_path) {
                if let Ok(cpn) = repo.cpn_from_path(path) {
                    cpns.insert(cpn);
                } else if path.as_str().starts_with("eclass/") {
                    eclass = true;
                }
            }
        }

        let mut reply = PushResponse { reports: vec![], failed: false };

        // scan individual packages that were changed
        let mut scanner = Scanner::new().exit([ReportLevel::Critical, ReportLevel::Warning]);
        for cpn in cpns {
            let reports = scanner
                .run(&repo, &cpn)
                .map_err(|e| Status::from_error(Box::new(e)))?;
            reply
                .reports
                .extend(reports.into_iter().map(|r| r.to_json()));
        }

        // scan full tree for metadata errors on eclass changes
        if eclass {
            scanner = scanner.reports([pkgcruft::check::Check::Metadata]);
            let reports = scanner
                .run(&repo, Restrict::True)
                .map_err(|e| Status::from_error(Box::new(e)))?;
            reply
                .reports
                .extend(reports.into_iter().map(|r| r.to_json()));
        }

        if scanner.failed() {
            reply.failed = true;

            // reset HEAD to the old commit and revert changes
            let old_oid: git2::Oid = push.old_ref.parse().expect("invalid old ref");
            let old_commit = git_repo.find_commit(old_oid).expect("invalid old ref");
            git_repo
                .reset(&old_commit.into_object(), git2::ResetType::Hard, None)
                .map_err(|e| Status::from_error(Box::new(e)))?;
        }

        Ok(Response::new(reply))
    }
}
