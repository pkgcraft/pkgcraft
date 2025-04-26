use std::fmt;
use std::fs;
use std::path::Path;
use std::str::FromStr;

use serde_with::{DeserializeFromStr, SerializeDisplay};

use crate::Error;

#[cfg(feature = "git")]
mod git;
#[cfg(feature = "https")]
mod tar;

#[derive(Debug, Clone, PartialEq, Eq, DeserializeFromStr, SerializeDisplay)]
pub(crate) enum Syncer {
    #[cfg(feature = "git")]
    Git(git::Repo),
    #[cfg(feature = "https")]
    TarHttps(tar::Repo),
}

impl fmt::Display for Syncer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            #[cfg(feature = "git")]
            Syncer::Git(repo) => write!(f, "{}", repo.uri),
            #[cfg(feature = "https")]
            Syncer::TarHttps(repo) => write!(f, "{}", repo.uri),
        }
    }
}

trait Syncable {
    fn uri_to_syncer(uri: &str) -> crate::Result<Syncer>;
    async fn sync<P: AsRef<Path> + Send>(&self, path: P) -> crate::Result<()>;
}

impl Syncer {
    pub(crate) fn sync<P: AsRef<Path>>(&self, path: P) -> crate::Result<()> {
        let path = path.as_ref();

        // make sure repos dir exists
        let repos_dir = path.parent().unwrap();
        if !repos_dir.exists() {
            fs::create_dir_all(repos_dir).map_err(|e| {
                Error::RepoSync(format!("failed creating repos dir {repos_dir:?}: {e}"))
            })?;
        }

        match self {
            #[cfg(feature = "git")]
            Syncer::Git(repo) => futures::executor::block_on(repo.sync(path)),
            #[cfg(feature = "https")]
            Syncer::TarHttps(repo) => futures::executor::block_on(repo.sync(path)),
        }
    }
}

impl FromStr for Syncer {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        #[rustfmt::skip]
        let prioritized_syncers = [
            #[cfg(feature = "git")]
            git::Repo::uri_to_syncer,
            #[cfg(feature = "https")]
            tar::Repo::uri_to_syncer,
        ];

        let mut syncer: Option<Syncer> = None;
        for func in prioritized_syncers {
            if let Ok(sync) = func(s) {
                syncer = Some(sync);
                break;
            }
        }

        match syncer {
            Some(s) => Ok(s),
            None => Err(Error::InvalidValue(format!("no syncers available: {s}"))),
        }
    }
}
