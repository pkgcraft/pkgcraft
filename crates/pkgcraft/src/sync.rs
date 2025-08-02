use std::fmt;
use std::fs;
use std::str::FromStr;

use camino::Utf8Path;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use tracing::debug;

use crate::Error;

mod git;
mod local;
mod tar;

#[derive(Debug, Clone, PartialEq, Eq, DeserializeFromStr, SerializeDisplay)]
pub(crate) enum Syncer {
    Git(git::Repo),
    Local(local::Repo),
    TarHttps(tar::Repo),
}

impl fmt::Display for Syncer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Syncer::Git(repo) => write!(f, "{}", repo.uri),
            Syncer::TarHttps(repo) => write!(f, "{}", repo.uri),
            Syncer::Local(repo) => write!(f, "{}", repo.path),
        }
    }
}

trait Syncable {
    fn uri_to_syncer(uri: &str) -> crate::Result<Syncer>;
    async fn sync<P: AsRef<Utf8Path> + Send>(&self, path: P) -> crate::Result<()>;
}

impl Syncer {
    pub(crate) async fn sync<P: AsRef<Utf8Path>>(&self, path: P) -> crate::Result<()> {
        let path = path.as_ref();

        // make sure repos dir exists
        let dir = path.parent().expect("invalid repos dir");
        fs::create_dir_all(dir)
            .map_err(|e| Error::RepoSync(format!("failed creating repos dir: {dir}: {e}")))?;

        match self {
            Syncer::Git(repo) => repo.sync(path).await,
            Syncer::TarHttps(repo) => repo.sync(path).await,
            Syncer::Local(repo) => repo.sync(path).await,
        }
    }
}

impl FromStr for Syncer {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        #[rustfmt::skip]
        let syncers = [
            git::Repo::uri_to_syncer,
            tar::Repo::uri_to_syncer,
            local::Repo::uri_to_syncer,
        ];

        for syncer in syncers {
            match syncer(s) {
                Err(e @ Error::NotARepo { .. }) => debug!("{e}"),
                result => return result,
            }
        }

        Err(Error::InvalidValue(format!("no syncers available: {s}")))
    }
}
