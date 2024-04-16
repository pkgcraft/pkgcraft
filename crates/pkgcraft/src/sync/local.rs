use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::sync::{Syncable, Syncer};
use crate::Error;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct Repo {
    pub(crate) path: PathBuf,
}

impl Syncable for Repo {
    fn uri_to_syncer(uri: &str) -> crate::Result<Syncer> {
        let path = PathBuf::from(uri);
        if path.exists() {
            Ok(Syncer::Local(Repo { path: PathBuf::from(uri) }))
        } else {
            Err(Error::RepoInit(format!("invalid local repo: {uri:?}")))
        }
    }

    async fn sync<P: AsRef<Path> + Send>(&self, _path: P) -> crate::Result<()> {
        Ok(())
    }
}
