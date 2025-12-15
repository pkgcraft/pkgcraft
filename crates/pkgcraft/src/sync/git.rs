use std::fmt::Display;
use std::fs;
use std::sync::LazyLock;

use camino::Utf8Path;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::repo::RepoFormat;
use crate::sync::Syncable;

static HANDLED_URI_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(https?|git)://(?P<path>.+)(\.git|)$").unwrap());

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct Repo {
    pub(crate) uri: String,
}

impl Display for Repo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.uri)
    }
}

/// Create an Error::RepoSync error using a given format string.
macro_rules! sync_error {
    ($msg:expr) => {
        Error::RepoSync(format!($msg))
    };
}

impl Syncable for Repo {
    fn uri_to_syncer(uri: &str) -> crate::Result<Self> {
        if HANDLED_URI_RE.is_match(uri) {
            Ok(Repo { uri: uri.to_string() })
        } else {
            Err(Error::NotARepo {
                kind: RepoFormat::Ebuild,
                id: uri.to_string(),
                err: "invalid git repo".to_string(),
            })
        }
    }

    fn fallback_name(&self) -> Option<String> {
        HANDLED_URI_RE.captures(&self.uri).and_then(|m| {
            Utf8Path::new(m.name("path").unwrap().as_str())
                .file_stem()
                .map(|n| n.to_string())
        })
    }

    async fn sync<P: AsRef<Utf8Path> + Send>(&self, path: P) -> crate::Result<()> {
        let path = path.as_ref();
        let uri = self.uri.as_str();
        let url = gix::url::parse(uri.into())
            .map_err(|e| Error::RepoSync(format!("invalid repo URL: {uri}: {e}")))?;

        if let Ok(repo) = gix::open(path) {
            let mut remote = repo
                .find_default_remote(gix::remote::Direction::Fetch)
                .transpose()
                .map_err(|e| sync_error!("invalid git repo: {path}: {e}"))?
                .ok_or_else(|| sync_error!("no remote found for git repo: {path}"))?;

            // don't fetch tags
            remote = remote.with_fetch_tags(gix::remote::fetch::Tags::None);

            let connection = remote
                .connect(gix::remote::Direction::Fetch)
                .map_err(|e| sync_error!("failed connecting to git repo: {uri}: {e}"))?;

            let prepare_fetch = connection
                .prepare_fetch(gix::progress::Discard, Default::default())
                .map_err(|e| sync_error!("failed fetching git repo: {uri}: {e}"))?;

            // TODO: support shallow repos
            prepare_fetch
                .receive(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)
                .map_err(|e| sync_error!("failed fetching git repo: {uri}: {e}"))?;
        } else {
            let mut prepare_fetch = gix::prepare_clone(url, path)
                .map_err(|e| Error::RepoSync(format!("failed cloning repo: {uri}: {e}")))?;
            let (mut prepare_checkout, _) = prepare_fetch
                .fetch_then_checkout(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)
                .map_err(|e| sync_error!("failed fetching git repo: {uri}: {e}"))?;
            let (_repo, _) = prepare_checkout
                .main_worktree(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)
                .map_err(|e| sync_error!("failed checking out git repo: {uri}: {e}"))?;
        }

        Ok(())
    }

    fn remove<P: AsRef<Utf8Path> + Send>(&self, path: P) -> crate::Result<()> {
        fs::remove_dir_all(path.as_ref())?;
        Ok(())
    }
}
