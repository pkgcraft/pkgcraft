use camino::Utf8PathBuf;
use pkgcraft::config::Config as PkgcraftConfig;
use pkgcruft::scan::Scanner;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use pkgcruft_git::proto::{StringRequest, StringResponse, pkgcruft_server::Pkgcruft};

#[derive(Debug)]
pub(crate) struct PkgcruftService {
    pub repo: Utf8PathBuf,
}

#[tonic::async_trait]
impl Pkgcruft for PkgcruftService {
    async fn version(
        &self,
        request: Request<StringRequest>,
    ) -> Result<Response<StringResponse>, Status> {
        let version = format!("{}-{}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));
        let req = request.into_inner();
        let reply = StringResponse {
            data: format!("client: {}, server: {version}", req.data),
        };
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
