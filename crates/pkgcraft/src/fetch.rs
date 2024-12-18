use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::LazyLock;

use camino::Utf8Path;
use futures::StreamExt;
use indexmap::IndexSet;
use indicatif::ProgressBar;
use itertools::Either;
use reqwest::{Client, ClientBuilder, StatusCode};
use tokio::io::AsyncWriteExt;
use tracing::warn;
use url::Url;

use crate::dep::Uri;
use crate::error::Error;
use crate::repo::ebuild::EbuildRepo;

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
    url: Url,
    rename: Option<String>,
    mirrors: Option<IndexSet<String>>,
}

impl Fetchable {
    /// Create a [`Fetchable`] from a [`Uri`].
    pub(crate) fn from_uri(uri: &Uri, repo: &EbuildRepo) -> crate::Result<Self> {
        let url = Url::parse(uri.as_str()).map_err(|e| Error::InvalidFetchable(format!("{e}")))?;

        // URLs without paths are invalid
        if url.path() == "/" {
            return Err(Error::InvalidFetchable(format!("lacks path: {url}")));
        }

        // validate protocol
        if !SUPPORTED_PROTOCOLS.contains(url.scheme()) {
            return Err(Error::InvalidFetchable(format!("unsupported protocol: {url}")));
        }

        // validate mirrors
        let mirrors = if url.scheme() == "mirror" {
            let Some(name) = url.domain() else {
                return Err(Error::InvalidFetchable(format!("invalid mirror: {url}")));
            };

            if let Some(values) = repo.mirrors().get(name) {
                Some(values.clone())
            } else {
                return Err(Error::InvalidFetchable(format!("unknown mirror {name}: {url}")));
            }
        } else {
            None
        };

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

    /// Return the file name for the [`Fetchable`].
    pub fn filename(&self) -> &str {
        self.rename.as_deref().unwrap_or_else(|| {
            self.url
                .path()
                .rsplit_once('/')
                .map(|(_, s)| s)
                .expect("invalid fetchable")
        })
    }

    /// Return an iterator of fetchables applying mirrors.
    fn mirrors(&self) -> impl Iterator<Item = Self> + '_ {
        if let Some(mirrors) = &self.mirrors {
            // TODO: support some type of mirror choice algorithm
            Either::Left(mirrors.iter().filter_map(|mirror| {
                let mirror = mirror.trim_end_matches('/');
                let path = self.url.path().trim_start_matches('/');
                let url = format!("{mirror}/{path}");
                Url::parse(&url).ok().map(|url| Self {
                    url,
                    rename: self.rename.clone(),
                    mirrors: None,
                })
            }))
        } else {
            Either::Right(std::iter::once(self.clone()))
        }
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
        pb: &ProgressBar,
        size: Option<u64>,
    ) -> crate::Result<()> {
        let mut mirrors = f.mirrors().peekable();
        while let Some(fetchable) = mirrors.next() {
            match self.fetch(&fetchable, path, pb, size).await {
                Ok(()) => return Ok(()),
                Err(e @ Error::FetchFailed { .. }) => {
                    if mirrors.peek().is_some() {
                        warn!("{e}");
                        continue;
                    } else {
                        return Err(e);
                    }
                }
                Err(e) => return Err(e),
            }
        }

        unreachable!("invalid fetchable mirror looping")
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
