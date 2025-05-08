use std::os::unix::fs::symlink;

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::sync::{Syncable, Syncer};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct Repo {
    pub(crate) path: Utf8PathBuf,
    pub(crate) name: String,
}

impl Syncable for Repo {
    fn uri_to_syncer(uri: &str) -> crate::Result<Syncer> {
        let path = Utf8PathBuf::from(uri);
        let name = path
            .file_name()
            .ok_or_else(|| Error::RepoInit(format!("invalid local repo: {uri}")))
            .map(|x| x.to_string())?;

        if path.exists() {
            Ok(Syncer::Local(Repo { path, name }))
        } else {
            Err(Error::RepoInit(format!("invalid local repo: {uri}")))
        }
    }

    async fn sync<P: AsRef<Utf8Path> + Send>(&self, path: P) -> crate::Result<()> {
        let path = path.as_ref();

        if !path.exists() {
            symlink(&self.path, path)
                .map_err(|e| Error::IO(format!("failed creating symlink: {path}: {e}")))?;
        }

        Ok(())
    }
}
