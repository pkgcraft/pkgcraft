use std::fs;
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, ETAG};
use serde::{Deserialize, Serialize};
use tar::Archive;
use tempfile::Builder;

use crate::error::Error::SyncError;
use crate::error::{Error, Result};
use crate::sync::{Syncable, Syncer};

static HANDLED_URI_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^tar\+(?P<url>https://.+)$").unwrap());

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Repo {
    pub uri: String,
    url: String,
}

impl Syncable for Repo {
    fn uri_to_syncer(uri: &str) -> Result<Syncer> {
        match HANDLED_URI_RE.captures(uri) {
            Some(m) => Ok(Syncer::TarHttps(Repo {
                uri: uri.to_string(),
                url: m.name("url").unwrap().as_str().to_string(),
            })),
            None => Err(Error::Error(format!("invalid tar+https repo: {:?}", uri))),
        }
    }

    fn sync<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        let repos_dir = path.parent().unwrap();
        let repo_name = path.file_name().unwrap().to_str().unwrap();

        // use cached ETag to check if update exists
        let etag_path = path.join(".etag");
        let mut headers = HeaderMap::new();
        if let Ok(previous_etag) = fs::read_to_string(&etag_path) {
            if let Ok(value) = HeaderValue::from_str(&previous_etag) {
                headers.insert("If-None-Match", value);
            }
        }

        let client = reqwest::blocking::Client::new();
        let mut resp = client
            .get(&self.url)
            .headers(headers)
            .send()
            .map_err(|e| SyncError(format!("failed downloading {:?}: {}", &self.url, e)))?;

        // content is unchanged
        if resp.status().as_u16() == 304 {
            return Ok(());
        }

        // download tarball to tempfile
        let mut temp_file = Builder::new()
            .suffix(&format!(".{}.tarball", &repo_name))
            .tempfile_in(&repos_dir)?;
        resp.copy_to(&mut temp_file)
            .map_err(|e| SyncError(format!("failed copy repo data: {}", e)))?;

        // unpack repo data to tempdir
        let tmp_dir = Builder::new()
            .suffix(&format!(".{}.update", &repo_name))
            .tempdir_in(&repos_dir)?;
        let tmp_dir_old = Builder::new()
            .suffix(&format!(".{}.old", &repo_name))
            .tempdir_in(&repos_dir)?;
        let tar_file = fs::File::open(temp_file.path())?;
        let mut archive = Archive::new(GzDecoder::new(tar_file));
        archive
            .entries()?
            .filter_map(|e| e.ok())
            .map(|mut entry| -> Result<PathBuf> {
                // drop first directory component in archive paths
                let stripped_path: PathBuf = entry.path()?.iter().skip(1).collect();
                entry.unpack(&tmp_dir.path().join(&stripped_path))?;
                Ok(stripped_path)
            })
            .filter_map(|e| e.ok())
            .for_each(drop);

        // move old repo out of the way if it exists and replace with unpacked repo
        if path.exists() {
            fs::rename(&path, &tmp_dir_old).map_err(|e| {
                SyncError(format!(
                    "failed moving old repo {:?} -> {:?}: {}",
                    &path, &tmp_dir_old, e
                ))
            })?;
        }
        fs::rename(&tmp_dir, &path).map_err(|e| {
            SyncError(format!(
                "failed moving repo {:?} -> {:?}: {}",
                &tmp_dir, &path, e
            ))
        })?;

        // TODO: store this in cache instead of repo file
        // update cached ETag value
        if let Some(etag) = resp.headers().get(ETAG) {
            fs::write(&etag_path, etag.as_bytes())
                .map_err(|e| SyncError(format!("failed writing etag {:?}: {}", &etag_path, e)))?;
        }

        Ok(())
    }
}
