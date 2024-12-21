use std::cmp::Ordering;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::LazyLock;
use std::{fmt, iter};

use camino::Utf8Path;
use futures::StreamExt;
use indexmap::IndexSet;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::{Client, ClientBuilder, StatusCode};
use tokio::io::AsyncWriteExt;
use tracing::warn;
use url::Url;

use crate::dep::Uri;
use crate::eapi::Feature::SrcUriUnrestrict;
use crate::error::Error;
use crate::pkg::ebuild::manifest::HashType;
use crate::pkg::ebuild::EbuildPkg;
use crate::pkg::{Package, RepoPackage};
use crate::repo::ebuild::Mirror;
use crate::repo::Repository;
use crate::traits::Contains;

static SUPPORTED_PROTOCOLS: LazyLock<HashSet<String>> = LazyLock::new(|| {
    ["http", "https", "mirror"]
        .into_iter()
        .map(Into::into)
        .collect()
});

/// Convert an error into an error string.
trait IntoReason {
    fn into_reason(self) -> String;
}

impl IntoReason for reqwest::Error {
    fn into_reason(self) -> String {
        if self.is_timeout() {
            "request timed out".to_string()
        } else if self.is_builder() {
            "unsupported URI".to_string()
        } else if let Some(value) = self.status() {
            value.to_string()
        } else {
            // drop URL from error to avoid potentially leaking authentication parameters
            self.without_url().to_string()
        }
    }
}

/// Wrapper for URI objects to generate valid URLs.
#[derive(Debug, Clone)]
pub struct Fetchable {
    pub url: Url,
    rename: Option<String>,
    mirrors: IndexSet<Mirror>,
}

impl Fetchable {
    /// Create a [`Fetchable`] from a [`Uri`].
    pub(crate) fn from_uri(
        uri: &Uri,
        pkg: &EbuildPkg,
        default_mirrors: bool,
    ) -> crate::Result<Self> {
        let mut fetch_restricted = pkg.restrict().contains("fetch");
        let mut mirror_restricted = fetch_restricted || pkg.restrict().contains("mirror");

        // strip selective URI restrictions
        let mut value = uri.as_str();
        if pkg.eapi().has(SrcUriUnrestrict) {
            if let Some(s) = value.strip_prefix("mirror+") {
                value = s;
                fetch_restricted = false;
                mirror_restricted = false;
            } else if let Some(s) = value.strip_prefix("fetch+") {
                value = s;
                fetch_restricted = false;
            }
        }

        // error out for fetch-restricted URIs
        if fetch_restricted {
            return Err(Error::RestrictedFetchable(Box::new(uri.clone())));
        }

        let url =
            Url::parse(value).map_err(|e| Error::InvalidFetchable(format!("{e}: {value}")))?;

        // validate protocol
        if !SUPPORTED_PROTOCOLS.contains(url.scheme()) {
            return Err(Error::InvalidFetchable(format!("unsupported protocol: {url}")));
        }

        // URLs without paths or queries are invalid
        if url.path().trim_start_matches('/').is_empty() && url.query().is_none() {
            return Err(Error::InvalidFetchable(format!("target missing: {url}")));
        }

        let repo = pkg.repo();
        let mut mirrors = IndexSet::new();

        // add default mirrors
        if default_mirrors && !mirror_restricted {
            if let Some(values) = repo.mirrors().get(repo.name()) {
                mirrors.extend(values.clone());
            }
        }

        // validate mirror URIs
        if url.scheme() == "mirror" {
            let Some(name) = url.domain() else {
                return Err(Error::InvalidFetchable(format!("mirror missing: {url}")));
            };

            if let Some(values) = repo.mirrors().get(name) {
                mirrors.extend(values.clone());
            } else {
                return Err(Error::InvalidFetchable(format!("mirror unknown: {url}")));
            }
        }

        Ok(Self {
            url,
            rename: uri.rename().map(Into::into),
            mirrors,
        })
    }

    /// Return the string serialization for the [`Fetchable`].
    pub fn as_str(&self) -> &str {
        self.url.as_str()
    }

    /// Return the renamed file name for the [`Fetchable`] if it exists.
    pub fn rename(&self) -> Option<&str> {
        self.rename.as_deref()
    }

    /// Return the file name for the [`Fetchable`].
    pub fn filename(&self) -> &str {
        self.rename().unwrap_or_else(|| {
            self.url
                .as_str()
                .rsplit_once('/')
                .map(|(_, s)| s)
                .expect("invalid fetchable")
        })
    }

    /// Return the mirrors for the [`Fetchable`].
    pub fn mirrors(&self) -> &IndexSet<Mirror> {
        &self.mirrors
    }
}

impl<'a> IntoIterator for &'a Fetchable {
    type Item = Fetchable;
    type IntoIter = iter::Chain<IterFetchableMirrors<'a>, iter::Once<Fetchable>>;

    fn into_iter(self) -> Self::IntoIter {
        // TODO: support some type of mirror choice algorithm
        let mirrors = IterFetchableMirrors {
            fetchable: self,
            mirrors: self.mirrors.iter(),
        };
        mirrors.chain(iter::once(self.clone()))
    }
}

pub struct IterFetchableMirrors<'a> {
    fetchable: &'a Fetchable,
    mirrors: indexmap::set::Iter<'a, Mirror>,
}

impl Iterator for IterFetchableMirrors<'_> {
    type Item = Fetchable;

    fn next(&mut self) -> Option<Self::Item> {
        self.mirrors.find_map(|mirror| {
            // TODO: properly implement GLEP 75 by fetching and parsing layout.conf
            let path = if mirror.name() == "gentoo" {
                let filename = self.fetchable.filename();
                let hash = HashType::Blake2b.hash(filename.as_bytes());
                format!("{}/{filename}", &hash[..2])
            } else {
                self.fetchable.url.path().to_string()
            };
            mirror.get_url(&path).ok().map(|url| Self::Item {
                url,
                rename: self.fetchable.rename.clone(),
                mirrors: Default::default(),
            })
        })
    }
}

impl PartialEq for Fetchable {
    fn eq(&self, other: &Self) -> bool {
        self.filename() == other.filename()
    }
}

impl Eq for Fetchable {}

impl Hash for Fetchable {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.filename().hash(state)
    }
}

impl Ord for Fetchable {
    fn cmp(&self, other: &Self) -> Ordering {
        self.filename().cmp(other.filename())
    }
}

impl PartialOrd for Fetchable {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Fetchable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.url)?;
        if let Some(value) = &self.rename {
            write!(f, " -> {value}")?;
        }
        Ok(())
    }
}

/// HTTP client wrapper handling mirror support.
pub struct Fetcher {
    client: Client,
}

impl Deref for Fetcher {
    type Target = Client;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

// TODO: support custom templates or colors?
/// Create a progress bar for a file download.
fn progress_bar(hidden: bool) -> ProgressBar {
    let pb = if hidden {
        ProgressBar::hidden()
    } else {
        ProgressBar::no_length()
    };
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})").unwrap()
        .progress_chars("#>-"));
    pb
}

impl Fetcher {
    /// Create a [`ClientBuilder`] to configure a [`Fetcher`].
    pub fn new(builder: ClientBuilder) -> crate::Result<Self> {
        let client = builder
            .build()
            .map_err(|e| Error::InvalidValue(format!("failed creating fetcher: {e}")))?;
        Ok(Self { client })
    }

    /// Fetch the file related to a [`Fetchable`], iterating over mirrors.
    pub async fn fetch_from_mirrors(
        &self,
        f: &Fetchable,
        path: &Utf8Path,
        mb: &MultiProgress,
        size: Option<u64>,
    ) -> crate::Result<()> {
        let mut result = Ok(());
        let pb = mb.add(progress_bar(mb.is_hidden()));

        for fetchable in f {
            match self.fetch(&fetchable, path, &pb, size).await {
                Err(e @ Error::FetchFailed { .. }) => result = Err(e),
                res => {
                    result = res;
                    break;
                }
            }

            if let Err(e) = &result {
                mb.suspend(|| warn!("{e}"));
            }
        }

        mb.remove(&pb);
        result
    }

    /// Fetch the file related to a [`Fetchable`].
    pub async fn fetch(
        &self,
        f: &Fetchable,
        path: &Utf8Path,
        pb: &ProgressBar,
        mut size: Option<u64>,
    ) -> crate::Result<()> {
        // determine the file position to start at supporting resumed downloads
        let mut request = self.get(f.url.clone());
        let mut position = if let Ok(meta) = tokio::fs::metadata(path).await {
            // determine the target size for existing files without manifest entries
            if size.is_none() {
                let response = self.get(f.url.clone()).send().await;
                size = response.ok().and_then(|r| r.content_length());
            }

            // check if completed or invalid
            let current_size = meta.len();
            if current_size != 0 && current_size == size.unwrap_or_default() {
                return Ok(());
            } else if let Some(value) = size {
                if current_size > value {
                    return Err(Error::InvalidValue(format!("file larger than expected: {path}")));
                }
            }

            // request remaining data assuming sequential downloads
            request = request.header("Range", format!("bytes={current_size}-"));
            current_size
        } else {
            0
        };

        let response = request
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| Error::FetchFailed {
                url: f.url.to_string(),
                reason: e.into_reason(),
            })?;

        // create file or open it for appending
        let mut file = match response.status() {
            StatusCode::PARTIAL_CONTENT => {
                pb.set_message(format!("Resuming {f}"));
                tokio::fs::OpenOptions::new().append(true).open(path).await
            }
            _ => {
                pb.set_message(format!("Downloading {f}"));
                position = 0;
                tokio::fs::File::create(path).await
            }
        }?;

        // initialize progress bar
        // enable completion progress if content size is available
        if let Some(value) = size.or(response.content_length()) {
            pb.set_length(value);
        }
        pb.set_position(position);
        // reset progress bar state so resumed download speed is accurate
        pb.reset();

        // download chunks while tracking progress
        let mut stream = response.bytes_stream();
        while let Some(item) = stream.next().await {
            let chunk = item
                .map_err(|e| Error::InvalidValue(format!("error while downloading file: {e}")))?;
            file.write_all(&chunk).await?;
            position += chunk.len() as u64;
            // TODO: handle progress differently for unsized downloads?
            pb.set_position(position);
        }

        file.flush().await?;
        pb.finish_and_clear();
        Ok(())
    }
}
