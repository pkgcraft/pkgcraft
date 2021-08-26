use std::fs;
use std::io;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use notify::{raw_watcher, RawEvent, RecursiveMode, Watcher};

use crate::error::Error;

// Return a string slice stripping the given character from the right side. Note that this assumes
// the string only contains ASCII characters.
pub(crate) fn rstrip(s: &str, c: char) -> &str {
    let mut count = 0;
    for x in s.chars().rev() {
        if x != c {
            break;
        }
        count += 1;
    }
    // We can't use chars.as_str().len() since std::iter::Rev doesn't support it.
    &s[..s.len() - count]
}

pub struct FileWatcher {
    watcher: notify::RecommendedWatcher,
    path: PathBuf,
    rx: mpsc::Receiver<RawEvent>,
}

impl FileWatcher {
    pub fn new<P: AsRef<Path>>(path: P) -> crate::Result<Self> {
        let path = PathBuf::from(path.as_ref());
        let (tx, rx) = mpsc::channel();
        let mut watcher = raw_watcher(tx)
            .map_err(|e| Error::IO(format!("failed creating watcher: {:?}: {}", &path, e)))?;
        let watched_dir = path
            .parent()
            .ok_or_else(|| Error::IO(format!("invalid path: {:?}", &path)))?
            .to_path_buf();
        // watch path parent directory for changes
        watcher
            .watch(&watched_dir, RecursiveMode::NonRecursive)
            .map_err(|e| Error::IO(format!("failed watching path: {:?}: {}", &path, e)))?;

        Ok(FileWatcher { watcher, path, rx })
    }

    pub fn watch_for(&self, event: notify::op::Op, timeout: Option<u64>) -> crate::Result<()> {
        // zero or an unset value effectively means no timeout occurs
        let timeout = match timeout {
            None | Some(0) => u64::MAX,
            Some(x) => x,
        };

        loop {
            match self
                .rx
                .recv_timeout(Duration::from_secs(timeout))
                .map_err(|_| {
                    Error::Timeout(format!("waiting for path existence: {:?}", &self.path))
                })? {
                RawEvent {
                    path: Some(p),
                    op: Ok(e),
                    ..
                } if p == self.path && e == event => break,
                _ => continue,
            }
        }
        Ok(())
    }

    pub fn unwatch<P: AsRef<Path>>(&mut self, path: P) -> crate::Result<()> {
        let path = path.as_ref();
        self.watcher
            .unwatch(&path)
            .map_err(|e| Error::IO(format!("failed unwatching path: {:?}: {}", &path, e)))?;
        Ok(())
    }
}

pub fn connect_or_spawn_arcanist<P: AsRef<Path>>(
    path: P,
    timeout: Option<u64>,
) -> crate::Result<()> {
    let socket = path.as_ref();

    if let Err(e) = UnixStream::connect(&socket) {
        match e.kind() {
            // spawn arcanist if it's not running
            io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound => {
                // remove potentially existing, old socket file
                fs::remove_file(&socket).unwrap_or_default();
                // watch for socket file creation
                let socket_watcher = FileWatcher::new(&socket)?;
                // start arcanist detached from the current process
                Command::new("arcanist")
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .map_err(|e| Error::Config(format!("failed starting arcanist: {}", e)))?;
                // wait for arcanist to bind to its socket file
                socket_watcher
                    .watch_for(notify::op::CREATE, timeout)
                    .map_err(|e| Error::Config(format!("failed starting arcanist: {}", e)))?;
            }
            _ => {
                return Err(Error::Config(format!(
                    "failed connecting to arcanist: {}: {:?}",
                    e, &socket
                )))
            }
        }
    }

    Ok(())
}
