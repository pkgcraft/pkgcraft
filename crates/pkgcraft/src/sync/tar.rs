use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::LazyLock;
use std::time::Duration;

use flate2::read::GzDecoder;
use futures::StreamExt;
use regex::Regex;
use reqwest::header::{HeaderMap, ETAG};
use serde::{Deserialize, Serialize};
use tar::Archive;
use tempfile::Builder;

use crate::sync::{Syncable, Syncer};
use crate::Error;

static HANDLED_URI_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^tar\+(?P<url>https://.+)$").unwrap());

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct Repo {
    pub(crate) uri: String,
    url: String,
}

impl Syncable for Repo {
    fn uri_to_syncer(uri: &str) -> crate::Result<Syncer> {
        match HANDLED_URI_RE.captures(uri) {
            Some(m) => Ok(Syncer::TarHttps(Repo {
                uri: uri.to_string(),
                url: m.name("url").unwrap().as_str().to_string(),
            })),
            None => Err(Error::RepoInit(format!("invalid tar+https repo: {uri:?}"))),
        }
    }

    async fn sync<P: AsRef<Path> + Send>(&self, path: P) -> crate::Result<()> {
        let path = path.as_ref();
        let repos_dir = path.parent().unwrap();
        let repo_name = path.file_name().unwrap().to_str().unwrap();

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

        // try unpacking via tar first since it's a lot faster for large repos
        let tar_unpack = Command::new("tar")
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
            .stderr(Stdio::null())
            .status();

        // fallback to built-in support on tar failure
        if tar_unpack.is_err() || !tar_unpack.unwrap().success() {
            let tar_file = fs::File::open(temp_file.path())
                .map_err(|e| Error::RepoSync(e.to_string()))?;
            // TODO: run decompression in a separate thread
            let mut archive = Archive::new(GzDecoder::new(tar_file));
            archive
                .entries()
                .map_err(|e| Error::RepoSync(e.to_string()))?
                .filter_map(|e| e.ok())
                .map(|mut entry| -> crate::Result<PathBuf> {
                    // drop first directory component in archive paths
                    let stripped_path: PathBuf = entry
                        .path()
                        .map_err(|e| Error::RepoSync(format!("failed unpacking archive: {e}")))
                        .iter()
                        .skip(1)
                        .collect();
                    entry
                        .unpack(tmp_dir.path().join(&stripped_path))
                        .map_err(|e| {
                            Error::RepoSync(format!("failed unpacking archive: {e}"))
                        })?;
                    Ok(stripped_path)
                })
                .filter_map(|e| e.ok())
                .for_each(drop);
        }

        // move old repo out of the way if it exists and replace with unpacked repo
        if path.exists() {
            fs::rename(path, &tmp_dir_old).map_err(|e| {
                Error::RepoSync(format!(
                    "failed moving old repo {path:?} -> {tmp_dir_old:?}: {e}"
                ))
            })?;
        }
        fs::rename(&tmp_dir, path).map_err(|e| {
            Error::RepoSync(format!("failed moving repo {tmp_dir:?} -> {path:?}: {e}"))
        })?;

        // TODO: store this in cache instead of repo file
        // update cached ETag value
        if let Some(etag) = resp_headers.get(ETAG) {
            fs::write(&etag_path, etag.as_bytes()).map_err(|e| {
                Error::RepoSync(format!("failed writing etag {etag_path:?}: {e}"))
            })?;
        }

        Ok(())
    }
}
