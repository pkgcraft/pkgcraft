use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::sync::{Syncable, Syncer};
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct Repo {
    pub(crate) path: PathBuf,
}

#[async_trait]
impl Syncable for Repo {
    fn uri_to_syncer(uri: &str) -> Result<Syncer> {
        let path = PathBuf::from(uri);
        match path.exists() {
            true => Ok(Syncer::Local(Repo {
                path: PathBuf::from(uri),
            })),
            false => Err(Error::RepoInit(format!("invalid local repo: {uri:?}"))),
        }
    }

    async fn sync<P: AsRef<Path> + Send>(&self, _path: P) -> Result<()> {
        Ok(())
    }
}
