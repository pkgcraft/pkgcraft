use std::path::Path;
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

pub fn wait_until_file_created(path: &Path, timeout: Option<u64>) -> crate::Result<()> {
    if !path.is_absolute() || path.file_name().is_none() {
        return Err(Error::IO(format!("invalid file to watch: {:?}", &path)));
    }

    // zero or an unset value effectively means no timeout occurs
    let timeout = match timeout {
        None | Some(0) => u64::MAX,
        Some(x) => x,
    };

    let (tx, rx) = mpsc::channel();
    let mut watcher = raw_watcher(tx)
        .map_err(|e| Error::IO(format!("failed creating file watcher: {:?}: {}", &path, e)))?;

    // watch parent directory for changes until given file exists
    let path_dir = path.parent().unwrap();
    watcher
        .watch(&path_dir, RecursiveMode::NonRecursive)
        .map_err(|e| Error::IO(format!("failed watching file: {:?}: {}", &path, e)))?;

    if !path.exists() {
        loop {
            match rx
                .recv_timeout(Duration::from_secs(timeout))
                .map_err(|_| Error::Timeout(format!("waiting for path existence: {:?}", &path)))?
            {
                RawEvent {
                    path: Some(p),
                    op: Ok(notify::op::CREATE),
                    ..
                } => {
                    if p == path {
                        break;
                    }
                }
                _ => continue,
            }
        }
    }

    watcher
        .unwatch(path_dir)
        .map_err(|e| Error::IO(format!("failed unwatching file: {:?}: {}", &path, e)))?;

    Ok(())
}
