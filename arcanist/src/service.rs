use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use crate::settings::Settings;

pub mod arcanist {
    tonic::include_proto!("arcanist");
}

pub use arcanist::arcanist_server::ArcanistServer;
use arcanist::{
    arcanist_server::Arcanist, ArcanistRequest, ArcanistResponse, ListRequest, ListResponse,
};

#[derive(Debug)]
pub struct ArcanistService {
    pub settings: Settings,
}

#[tonic::async_trait]
impl Arcanist for ArcanistService {
    async fn list_repos(
        &self,
        _request: Request<ArcanistRequest>,
    ) -> Result<Response<ListResponse>, Status> {
        let mut repos: Vec<String> = Vec::new();
        for (id, config) in self.settings.config.repos.configs.iter() {
            repos.push(format!("{}: {:?}", id, config.location));
        }
        let reply = ListResponse { data: repos };
        Ok(Response::new(reply))
    }

    type SearchPackagesStream = ReceiverStream<Result<ArcanistResponse, Status>>;

    async fn search_packages(
        &self,
        request: Request<ListRequest>,
    ) -> Result<Response<Self::SearchPackagesStream>, Status> {
        let (tx, rx) = mpsc::channel(4);
        tokio::spawn(async move {
            for pkg in request.into_inner().data.iter() {
                tx.send(Ok(ArcanistResponse {
                    data: pkg.to_string(),
                }))
                .await
                .unwrap();
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    type AddPackagesStream = ReceiverStream<Result<ArcanistResponse, Status>>;

    async fn add_packages(
        &self,
        _request: Request<ListRequest>,
    ) -> Result<Response<Self::AddPackagesStream>, Status> {
        unimplemented!()
    }

    type RemovePackagesStream = ReceiverStream<Result<ArcanistResponse, Status>>;

    async fn remove_packages(
        &self,
        _request: Request<ListRequest>,
    ) -> Result<Response<Self::RemovePackagesStream>, Status> {
        unimplemented!()
    }

    async fn version(
        &self,
        request: Request<ArcanistRequest>,
    ) -> Result<Response<ArcanistResponse>, Status> {
        let version = format!("{}-{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        let req = request.into_inner();
        let reply = ArcanistResponse {
            data: format!("client: {}, server: {}", req.data, version),
        };
        Ok(Response::new(reply))
    }
}
