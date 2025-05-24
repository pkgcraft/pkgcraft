use std::sync::Arc;

use camino::Utf8PathBuf;
use pkgcraft::config::Config as PkgcraftConfig;
use pkgcruft::scan::Scanner;
use tokio::sync::{Semaphore, mpsc};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use pkgcruft_git::Error;
use pkgcruft_git::git::diff_to_cpns;
use pkgcruft_git::proto::{
    EmptyRequest, PushRequest, StringResponse, pkgcruft_server::Pkgcruft,
};

pub(crate) struct PkgcruftService {
    path: Utf8PathBuf,
    scanning: Arc<Semaphore>,
}

impl PkgcruftService {
    pub(crate) fn new<P: Into<Utf8PathBuf>>(path: P) -> pkgcruft_git::Result<Self> {
        let path = path.into();

        // verify target path is a valid git repo
        git2::Repository::open(&path)
            .map_err(|e| Error::Start(format!("invalid git repo: {path}: {e}")))?;

        // verify target path is a valid ebuild repo
        let mut config = PkgcraftConfig::new("pkgcraft", "");
        let _ = config
            .add_repo_path("repo", &path, 0)
            .map(|r| r.into_ebuild())
            .map_err(|_| Error::Start(format!("invalid ebuild repo: {path}")))?;

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
        let data = format!("{}-{}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));
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

        let (tx, rx) = mpsc::channel(4);
        let path = self.path.clone();

        tokio::spawn(async move {
            // TODO: partially reload repo or reset lazy metadata fields
            let mut config = PkgcraftConfig::new("pkgcraft", "");
            let repo = config
                .add_repo_path("repo", path, 0)
                .map_err(|e| Status::from_error(Box::new(e)))?;
            let repo = repo.into_ebuild().map_err(|repo| {
                Status::invalid_argument(format!("invalid ebuild repo: {repo}"))
            })?;
            config
                .finalize()
                .map_err(|e| Status::from_error(Box::new(e)))?;

            // TODO: process request data into a restrict target
            let scanner = Scanner::new();
            let reports = scanner
                .run(&repo, repo.path())
                .map_err(|e| Status::from_error(Box::new(e)))?;

            for report in reports {
                let data = report.to_json();
                if tx.send(Ok(StringResponse { data })).await.is_err() {
                    break;
                }
            }

            // explicitly own the permit until scanning is finished
            drop(permit);

            Ok::<(), Status>(())
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
        let (tx, rx) = mpsc::channel(4);

        tokio::spawn(async move {
            // TODO: partially reload repo or reset lazy metadata fields
            let mut config = PkgcraftConfig::new("pkgcraft", "");
            let repo = config
                .add_repo_path("repo", path, 0)
                .map_err(|e| Status::from_error(Box::new(e)))?;
            let repo = repo.into_ebuild().map_err(|repo| {
                Status::invalid_argument(format!("invalid ebuild repo: {repo}"))
            })?;
            config
                .finalize()
                .map_err(|e| Status::from_error(Box::new(e)))?;

            // TODO: process request data into a restrict target
            let scanner = Scanner::new();

            for cpn in cpns {
                let reports = scanner
                    .run(&repo, &cpn)
                    .map_err(|e| Status::from_error(Box::new(e)))?;

                for report in reports {
                    let data = report.to_json();
                    if tx.send(Ok(StringResponse { data })).await.is_err() {
                        break;
                    }
                }
            }

            // explicitly own the scanning permit until it's finished
            drop(permit);

            Ok::<(), Status>(())
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}
