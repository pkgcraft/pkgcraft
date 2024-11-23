use std::fs;
use std::os::unix::net::UnixStream as StdUnixStream;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use anyhow::{Context as AnyhowContext, Result, bail};
use camino::Utf8Path;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tonic::transport::server::Connected;

#[derive(Debug)]
pub(crate) struct UnixStream(pub tokio::net::UnixStream);

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) struct UdsConnectInfo {
    pub peer_addr: Option<Arc<tokio::net::unix::SocketAddr>>,
    pub peer_cred: Option<tokio::net::unix::UCred>,
}

impl Connected for UnixStream {
    type ConnectInfo = UdsConnectInfo;

    fn connect_info(&self) -> Self::ConnectInfo {
        UdsConnectInfo {
            peer_addr: self.0.peer_addr().ok().map(Arc::new),
            peer_cred: self.0.peer_cred().ok(),
        }
    }
}

impl AsyncRead for UnixStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl AsyncWrite for UnixStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}

// Verify if a given path is an accept UNIX domain socket path.
pub(crate) fn verify_socket_path<P: AsRef<Utf8Path>>(path: P) -> Result<()> {
    let path = path.as_ref();
    let socket_dir = &path.parent().context(format!("invalid socket: {path}"))?;

    // check if the socket is already in use
    if StdUnixStream::connect(path).is_ok() {
        bail!("service already running on: {path}");
    }

    // create dirs and remove old socket file if it exists
    fs::create_dir_all(socket_dir)
        .context(format!("failed creating socket dir: {socket_dir}"))?;
    fs::remove_file(path).unwrap_or_default();

    Ok(())
}
