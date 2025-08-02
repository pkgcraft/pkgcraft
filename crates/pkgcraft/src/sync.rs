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
            Syncer::Git(repo) => write!(f, "{repo}"),
            Syncer::TarHttps(repo) => write!(f, "{repo}"),
            Syncer::Local(repo) => write!(f, "{repo}"),
        }
    }
}

trait Syncable: fmt::Display + fmt::Debug + Sized {
    fn uri_to_syncer(uri: &str) -> crate::Result<Self>;
    fn fallback_name(&self) -> Option<String>;
    async fn sync<P: AsRef<Utf8Path> + Send>(&self, path: P) -> crate::Result<()>;
    // TODO decide if we want it async as well.
    fn remove<P: AsRef<Utf8Path> + Send>(&self, path: P) -> crate::Result<()>;
}

impl Syncer {
    pub(crate) fn fallback_name(&self) -> Option<String> {
        match self {
            Syncer::Git(repo) => repo.fallback_name(),
            Syncer::TarHttps(repo) => repo.fallback_name(),
            Syncer::Local(repo) => repo.fallback_name(),
        }
    }

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
    pub(crate) fn remove<P: AsRef<Utf8Path> + Send>(&self, path: P) -> crate::Result<()> {
        match self {
            Syncer::Git(repo) => repo.remove(path),
            Syncer::TarHttps(repo) => repo.remove(path),
            Syncer::Local(repo) => repo.remove(path),
        }
    }
}

impl FromStr for Syncer {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        #[rustfmt::skip]
        let syncers = [
            |uri| git::Repo::uri_to_syncer(uri).map(Syncer::Git),
            |uri| tar::Repo::uri_to_syncer(uri).map(Syncer::TarHttps),
            |uri| local::Repo::uri_to_syncer(uri).map(Syncer::Local),
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
