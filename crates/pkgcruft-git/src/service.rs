use std::net::SocketAddr;
use std::sync::Arc;

use camino::Utf8PathBuf;
use itertools::Itertools;
use pkgcraft::config::Config as PkgcraftConfig;
use pkgcruft::scan::Scanner;
use tokio::net::{TcpListener, UnixListener};
use tokio::sync::{Semaphore, mpsc};
use tokio_stream::wrappers::{ReceiverStream, TcpListenerStream, UnixListenerStream};
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use tracing::info;

use crate::Error;
use crate::git::diff_to_cpns;
use crate::proto::pkgcruft_server::{Pkgcruft, PkgcruftServer};
use crate::proto::{EmptyRequest, PushRequest, StringResponse};
use crate::uds::verify_socket_path;

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

        info!("service listening at: {socket}");
        Ok(listener)
    }
}

pub struct PkgcruftServiceBuilder {
    path: Utf8PathBuf,
    socket: Option<String>,
}

impl PkgcruftServiceBuilder {
    /// Create a new service builder.
    pub fn new<P: Into<Utf8PathBuf>>(path: P) -> Self {
        Self {
            path: path.into(),
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

        let service = PkgcruftService::try_new(self.path)?;
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
    path: Utf8PathBuf,
    scanning: Arc<Semaphore>,
}

impl PkgcruftService {
    /// Try creating a new service.
    fn try_new<P: Into<Utf8PathBuf>>(path: P) -> crate::Result<Self> {
        let path = path.into();

        // WARNING: This appears to invalidate the environment in some fashion so
        // std::env::var() calls don't work as expected after it.
        //
        // verify target path is a valid git repo
        git2::Repository::open(&path)
            .map_err(|e| Error::Start(format!("invalid git repo: {path}: {e}")))?;

        // verify target path is a valid ebuild repo
        let mut config = PkgcraftConfig::new("pkgcraft", "");
        let _ = config
            .add_repo_path("repo", &path, 0)
            .map(|r| r.into_ebuild())
            .map_err(|e| Error::Start(format!("invalid ebuild repo: {path}: {e}")))?;

        Ok(Self {
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

    type PushStream = ReceiverStream<Result<StringResponse, Status>>;

    async fn push(
        &self,
        request: Request<PushRequest>,
    ) -> Result<Response<Self::ScanStream>, Status> {
        let path = self.path.clone();
        let git_repo =
            git2::Repository::open(&path).map_err(|e| Status::from_error(Box::new(e)))?;

        // get the difference between push refs
        let diff = request
            .into_inner()
            .diff(&git_repo)
            .map_err(|e| Status::from_error(Box::new(e)))?;

        // TODO: skip pushes where the ref name doesn't match the default branch

        // determine target Cpns from diff
        let cpns = diff_to_cpns(&diff).map_err(|e| Status::from_error(Box::new(e)))?;

        // TODO: use try_acquire_owned() with custom timeout
        // acquire exclusive scanning permission
        let permit = self.scanning.clone().acquire_owned().await.unwrap();

        // TODO: partially reload repo or reset lazy metadata fields
        let mut config = PkgcraftConfig::new("pkgcraft", "");
        let repo = config
            .add_repo_path("repo", path, 0)
            .map_err(|e| Status::from_error(Box::new(e)))?;
        let repo = repo
            .into_ebuild()
            .map_err(|e| Status::invalid_argument(format!("invalid ebuild repo: {e}")))?;
        config
            .finalize()
            .map_err(|e| Status::from_error(Box::new(e)))?;

        // TODO: process request data into a restrict target
        let scanner = Scanner::new();
        let reports: Vec<_> = cpns
            .into_iter()
            .map(move |cpn| scanner.run(&repo, &cpn))
            .try_collect()
            .map_err(|e| Status::from_error(Box::new(e)))?;

        let (tx, rx) = mpsc::channel(4);

        tokio::spawn(async move {
            for report in reports.into_iter().flatten() {
                if tx.send(Ok(report.into())).await.is_err() {
                    break;
                }
            }

            // explicitly own until scanning is finished
            drop(permit);
            drop(config);
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}
