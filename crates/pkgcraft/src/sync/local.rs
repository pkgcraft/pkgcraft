use std::fmt::Display;
use std::fs;
use std::os::unix::fs::symlink;

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::repo::RepoFormat;
use crate::sync::Syncable;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct Repo {
    pub(crate) path: Utf8PathBuf,
}

impl Display for Repo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path)
    }
}

impl Syncable for Repo {
    fn uri_to_syncer(uri: &str) -> crate::Result<Self> {
        let path = Utf8PathBuf::from(uri);

        match path.canonicalize_utf8() {
            Ok(path) => Ok(Repo { path }),
            Err(e) => {
                if uri.starts_with('/') {
                    Err(Error::RepoInit(format!("invalid local repo: {uri}: {e}")))
                } else {
                    Err(Error::NotARepo {
                        kind: RepoFormat::Ebuild,
                        id: uri.to_string(),
                        err: "invalid local repo".to_string(),
                    })
                }
            }
        }
    }

    fn fallback_name(&self) -> Option<String> {
        self.path.file_stem().map(|n| n.to_string())
    }

    async fn sync<P: AsRef<Utf8Path> + Send>(&self, path: P) -> crate::Result<()> {
        let path = path.as_ref();

        if !path.exists() {
            symlink(&self.path, path)
                .map_err(|e| Error::IO(format!("failed creating symlink: {path}: {e}")))?;
        }

        Ok(())
    }

    fn remove<P: AsRef<Utf8Path> + Send>(&self, path: P) -> crate::Result<()> {
        fs::remove_file(path.as_ref())?;

        Ok(())
    }
}
