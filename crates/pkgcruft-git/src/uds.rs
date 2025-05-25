use std::fs;
use std::os::unix::net::UnixStream;

use camino::Utf8Path;

use crate::Error;

// Verify if a given path is an accept UNIX domain socket path.
pub(crate) fn verify_socket_path<P: AsRef<Utf8Path>>(path: P) -> crate::Result<()> {
    let path = path.as_ref();
    let path = path
        .canonicalize_utf8()
        .map_err(|e| Error::InvalidValue(format!("invalid socket: {path}: {e}")))?;
    let socket_dir = &path
        .parent()
        .ok_or_else(|| Error::InvalidValue(format!("invalid socket: {path}")))?;

    // check if the socket is already in use
    if UnixStream::connect(&path).is_ok() {
        return Err(Error::InvalidValue(format!("service already running on: {path}")));
    }

    // create dirs and remove old socket file if it exists
    fs::create_dir_all(socket_dir).map_err(|e| {
        Error::InvalidValue(format!("failed creating socket dir: {socket_dir}: {e}"))
    })?;
    fs::remove_file(&path).unwrap_or_default();

    Ok(())
}
