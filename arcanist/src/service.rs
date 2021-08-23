use tonic::{Request, Response, Status};

pub mod arcanist {
    tonic::include_proto!("arcanist");
}

use arcanist::{arcanist_server::Arcanist, ArcanistRequest, ArcanistResponse};

pub use arcanist::arcanist_server::ArcanistServer;

#[derive(Default)]
pub struct ArcanistService;

#[tonic::async_trait]
impl Arcanist for ArcanistService {
    async fn version(
        &self,
        request: Request<ArcanistRequest>,
    ) -> Result<Response<ArcanistResponse>, Status> {
        let version = format!("{}-{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        let req = request.into_inner();
        let reply = ArcanistResponse {
            message: format!("client: {}, server: {}", req.message, version),
        };
        Ok(Response::new(reply))
    }
}
