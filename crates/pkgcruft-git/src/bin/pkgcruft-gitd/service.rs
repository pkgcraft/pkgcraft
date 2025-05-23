use camino::Utf8PathBuf;
use pkgcraft::config::Config as PkgcraftConfig;
use pkgcruft::scan::Scanner;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use pkgcruft_git::Error;
use pkgcruft_git::proto::{
    EmptyRequest, StringRequest, StringResponse, pkgcruft_server::Pkgcruft,
};

#[derive(Debug)]
pub(crate) struct PkgcruftService {
    repo: Utf8PathBuf,
}

impl PkgcruftService {
    pub(crate) fn new<P: Into<Utf8PathBuf>>(repo: P) -> pkgcruft_git::Result<Self> {
        let repo = repo.into();

        // verify target path is a valid ebuild repo
        let mut config = PkgcraftConfig::new("pkgcraft", "");
        config
            .add_repo_path("repo", &repo, 0)?
            .into_ebuild()
            .map_err(|repo| Error::Start(format!("invalid ebuild repo: {repo}")))?;

        Ok(Self { repo })
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
        _request: Request<StringRequest>,
    ) -> Result<Response<Self::ScanStream>, Status> {
        let (tx, rx) = mpsc::channel(4);
        let path = self.repo.clone();

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

            Ok::<(), Status>(())
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}
