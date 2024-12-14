use camino::Utf8Path;
use futures::StreamExt;
use indicatif::ProgressBar;
use reqwest::{Client, StatusCode};
use tokio::io::AsyncWriteExt;

use crate::dep::Uri;
use crate::error::Error;

/// Download the file related to a URI.
pub async fn download(
    client: &Client,
    uri: &Uri,
    path: &Utf8Path,
    pb: &ProgressBar,
    mut size: Option<u64>,
) -> crate::Result<()> {
    // determine the file position to start at supporting resumed downloads
    let mut request = client.get(uri.as_ref());
    let mut position = if let Ok(meta) = tokio::fs::metadata(path).await {
        // determine the target size for existing files without manifest entries
        if size.is_none() {
            let response = client.get(uri.as_ref()).send().await;
            size = response.ok().and_then(|r| r.content_length());
        }

        // check if completed or invalid
        let current_size = meta.len();
        if current_size - size.unwrap_or_default() == 0 {
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
        .map_err(|e| Error::InvalidValue(format!("failed to get: {uri}: {e}")))?;

    // create file or open it for appending
    let mut file = match response.status() {
        StatusCode::PARTIAL_CONTENT => tokio::fs::OpenOptions::new().append(true).open(path).await,
        _ => tokio::fs::File::create(path).await,
    }?;

    // initialize progress bar
    pb.set_message(format!("Downloading {uri}"));
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
        let chunk =
            item.map_err(|e| Error::InvalidValue(format!("error while downloading file: {e}")))?;
        file.write_all(&chunk).await?;
        position += chunk.len() as u64;
        // TODO: handle progress differently for unsized downloads?
        pb.set_position(position);
    }

    file.flush().await?;
    pb.finish_and_clear();
    Ok(())
}
