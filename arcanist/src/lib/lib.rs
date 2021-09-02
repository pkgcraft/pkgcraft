mod error;
mod utils;

pub mod proto {
    tonic::include_proto!("arcanist");
}

pub use self::proto::arcanist_client::ArcanistClient as Client;
pub use self::proto::arcanist_server::ArcanistServer as Server;

pub use self::error::{Error, Result};
pub use self::utils::{connect_or_spawn, spawn};
