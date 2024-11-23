mod error;
mod utils;

pub mod proto {
    tonic::include_proto!("pkgcruft");
}

pub use self::proto::pkgcruft_client::PkgcruftClient as Client;
pub use self::proto::pkgcruft_server::PkgcruftServer as Server;

pub use self::error::{Error, Result};
pub use self::utils::{connect_or_spawn, spawn};
