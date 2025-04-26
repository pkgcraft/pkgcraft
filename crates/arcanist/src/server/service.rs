use std::sync::Arc;

use pkgcraft::config::Config as PkgcraftConfig;
use pkgcraft::{repo::Repository, Error};
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use crate::settings::Settings;

use arcanist::proto::{
    arcanist_server::Arcanist, AddRepoRequest, ListRequest, ListResponse, StringRequest,
    StringResponse,
};

#[derive(Debug)]
pub struct ArcanistService {
    pub settings: Settings,
    pub config: Arc<RwLock<PkgcraftConfig>>,
}

#[tonic::async_trait]
impl Arcanist for ArcanistService {
    async fn add_repo(
        &self,
        request: Request<AddRepoRequest>,
    ) -> Result<Response<StringResponse>, Status> {
        let req = request.into_inner();
        let config = &mut self.config.write().await;
        match config.add_repo_uri(&req.name, 0, &req.uri) {
            Err(Error::Config(e)) => Err(Status::failed_precondition(e)),
            Err(e) => Err(Status::internal(format!("{e}"))),
            Ok(_) => {
                let reply = StringResponse { data: req.name };
                Ok(Response::new(reply))
            }
        }
    }

    async fn remove_repos(
        &self,
        request: Request<ListRequest>,
    ) -> Result<Response<ListResponse>, Status> {
        let req = request.into_inner();
        let config = &mut self.config.write().await;
        match config.del_repos(&req.data, true) {
            Err(Error::Config(e)) => Err(Status::failed_precondition(e)),
            Err(e) => Err(Status::internal(format!("{e}"))),
            Ok(_) => {
                let reply = ListResponse { data: req.data };
                Ok(Response::new(reply))
            }
        }
    }

    async fn list_repos(
        &self,
        _request: Request<StringRequest>,
    ) -> Result<Response<ListResponse>, Status> {
        let mut repos = vec![];
        let config = self.config.read().await;
        for (id, repo) in &config.repos {
            repos.push(format!("{id}: {:?}", repo.path()));
        }
        let reply = ListResponse { data: repos };
        Ok(Response::new(reply))
    }

    async fn sync_repos(
        &self,
        request: Request<ListRequest>,
    ) -> Result<Response<ListResponse>, Status> {
        let req = request.into_inner();
        let config = &mut self.config.write().await;
        match config.repos.sync(&req.data) {
            Err(Error::Config(e)) => Err(Status::failed_precondition(e)),
            Err(e) => Err(Status::internal(format!("{e}"))),
            Ok(_) => {
                let reply = ListResponse { data: req.data };
                Ok(Response::new(reply))
            }
        }
    }

    type SearchPackagesStream = ReceiverStream<Result<StringResponse, Status>>;

    async fn search_packages(
        &self,
        request: Request<ListRequest>,
    ) -> Result<Response<Self::SearchPackagesStream>, Status> {
        let (tx, rx) = mpsc::channel(4);
        tokio::spawn(async move {
            for pkg in request.into_inner().data {
                tx.send(Ok(StringResponse { data: pkg.to_string() }))
                    .await
                    .unwrap();
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    type AddPackagesStream = ReceiverStream<Result<StringResponse, Status>>;

    async fn add_packages(
        &self,
        _request: Request<ListRequest>,
    ) -> Result<Response<Self::AddPackagesStream>, Status> {
        todo!()
    }

    type RemovePackagesStream = ReceiverStream<Result<StringResponse, Status>>;

    async fn remove_packages(
        &self,
        _request: Request<ListRequest>,
    ) -> Result<Response<Self::RemovePackagesStream>, Status> {
        todo!()
    }

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
}
