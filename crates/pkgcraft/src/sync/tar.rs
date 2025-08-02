use std::fmt::Display;
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::LazyLock;
use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};
use futures::StreamExt;
use regex::Regex;
use reqwest::header::{ETAG, HeaderMap};
use serde::{Deserialize, Serialize};
use tempfile::Builder;

use crate::Error;
use crate::repo::RepoFormat;
use crate::sync::Syncable;

static HANDLED_URI_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^tar\+(?P<url>https://(?P<path>.+))$").unwrap());

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct Repo {
    uri: String,
    url: String,
    path: Utf8PathBuf,
}

impl Display for Repo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.uri)
    }
}

impl Syncable for Repo {
    fn uri_to_syncer(uri: &str) -> crate::Result<Self> {
        match HANDLED_URI_RE.captures(uri) {
            Some(m) => Ok(Repo {
                uri: uri.to_string(),
                url: m.name("url").unwrap().as_str().to_string(),
                path: Utf8PathBuf::from(m.name("path").unwrap().as_str()),
            }),
            None => Err(Error::NotARepo {
                kind: RepoFormat::Ebuild,
                id: uri.to_string(),
                err: "invalid tar+https repo".to_string(),
            }),
        }
    }

    fn fallback_name(&self) -> Option<String> {
        self.path.file_stem().map(|n| n.to_string())
    }

    async fn sync<P: AsRef<Utf8Path> + Send>(&self, path: P) -> crate::Result<()> {
        let path = path.as_ref();
        let repos_dir = path.parent().unwrap();
        let repo_name = path.file_name().unwrap();

        // use cached ETag to check if update exists
        let etag_path = path.join(".etag");
        let mut req_headers = HeaderMap::new();
        if let Ok(previous_etag) = fs::read_to_string(&etag_path) {
            if let Ok(value) = previous_etag.parse() {
                req_headers.insert("If-None-Match", value);
            }
        }

        let resp = reqwest::Client::new()
            .get(&self.url)
            .headers(req_headers)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| Error::RepoSync(e.to_string()))?
            .error_for_status()
            .map_err(|e| Error::RepoSync(e.to_string()))?;

        // content is unchanged
        if resp.status().as_u16() == 304 {
            return Ok(());
        }

        // Clone headers used to later extract ETAG data since streaming the response body consumes
        // the response object.
        let resp_headers = &resp.headers().clone();

        // create tempfile
        let mut temp_file = Builder::new()
            .suffix(&format!(".{repo_name}.tar.gz"))
            .tempfile_in(repos_dir)
            .map_err(|e| Error::RepoSync(e.to_string()))?;

        // download tarball to tempfile
        let mut stream = resp.bytes_stream();
        while let Some(item) = stream.next().await {
            let chunk =
                item.map_err(|e| Error::RepoSync(format!("failed downloading repo: {e}")))?;
            temp_file
                .write(&chunk)
                .map_err(|e| Error::RepoSync(format!("failed writing repo: {e}")))?;
        }

        // unpack repo data to tempdir
        let tmp_dir = Builder::new()
            .suffix(&format!(".{repo_name}.update"))
            .tempdir_in(repos_dir)
            .map_err(|e| Error::RepoSync(e.to_string()))?;
        let tmp_dir_old = Builder::new()
            .suffix(&format!(".{repo_name}.old"))
            .tempdir_in(repos_dir)
            .map_err(|e| Error::RepoSync(e.to_string()))?;

        let output = Command::new("tar")
            .args([
                "--extract",
                "--gzip",
                "-f",
                temp_file.path().to_str().unwrap(),
                "--strip-components=1",
                "--no-same-owner",
                "-C",
                tmp_dir.path().to_str().unwrap(),
            ])
            .stdout(Stdio::null())
            .output()
            .map_err(|e| Error::RepoSync(format!("failed unpacking repo: {e}")))?;

        if !output.status.success() {
            let msg = String::from_utf8_lossy(&output.stderr);
            return Err(Error::RepoSync(format!("failed unpacking repo: {}", msg.trim())));
        }

        // move old repo out of the way if it exists and replace with unpacked repo
        if path.exists() {
            fs::rename(path, &tmp_dir_old).map_err(|e| {
                Error::RepoSync(format!("failed moving old repo: {path}: {e}"))
            })?;
        }
        fs::rename(&tmp_dir, path)
            .map_err(|e| Error::RepoSync(format!("failed moving repo: {path}: {e}")))?;

        // TODO: store this in cache instead of repo file
        // update cached ETag value
        if let Some(etag) = resp_headers.get(ETAG) {
            fs::write(&etag_path, etag.as_bytes()).map_err(|e| {
                Error::RepoSync(format!("failed writing etag: {etag_path}: {e}"))
            })?;
        }

        Ok(())
    }

    fn remove<P: AsRef<Utf8Path> + Send>(&self, path: P) -> crate::Result<()> {
        fs::remove_dir_all(path.as_ref())?;
        Ok(())
    }
}
