use std::fs;
use std::os::unix::net::UnixStream;

use anyhow::Context;
use camino::Utf8Path;

// Verify if a given path is an accept UNIX domain socket path.
pub(crate) fn verify_socket_path<P: AsRef<Utf8Path>>(path: P) -> anyhow::Result<()> {
    let path = path.as_ref();
    let socket_dir = &path.parent().context(format!("invalid socket: {path}"))?;

    // check if the socket is already in use
    if UnixStream::connect(path).is_ok() {
        anyhow::bail!("service already running on: {path}");
    }

    // create dirs and remove old socket file if it exists
    fs::create_dir_all(socket_dir)
        .context(format!("failed creating socket dir: {socket_dir}"))?;
    fs::remove_file(path).unwrap_or_default();

    Ok(())
}
