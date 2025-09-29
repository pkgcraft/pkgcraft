use std::io::{self, Write};
use std::path::Path;

use git2::{Diff, Oid};

use crate::Error;

/// Clone a git repo into a path.
pub(crate) fn clone<P: AsRef<Path>>(uri: &str, path: P) -> Result<(), git2::Error> {
    let path = path.as_ref();
    let mut cb = git2::RemoteCallbacks::new();

    // show transfer progress
    cb.transfer_progress(|stats| {
        if stats.received_objects() == stats.total_objects() {
            print!("Resolving deltas {}/{}\r", stats.indexed_deltas(), stats.total_deltas());
        } else if stats.total_objects() > 0 {
            print!(
                "Received {}/{} objects ({}) in {} bytes\r",
                stats.received_objects(),
                stats.total_objects(),
                stats.indexed_objects(),
                stats.received_bytes()
            );
        }
        io::stdout().flush().unwrap();
        true
    });

    let mut fo = git2::FetchOptions::new();
    fo.remote_callbacks(cb);

    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fo);
    builder.clone(uri, path)?;

    Ok(())
}

/// Determine the file differences between two references.
pub fn diff<'a>(repo: &'a git2::Repository, old: &str, new: &str) -> crate::Result<Diff<'a>> {
    // parse old reference and get related tree
    let old_oid: Oid = old
        .parse()
        .map_err(|_| Error::InvalidPushRequest(format!("invalid old ref: {old}")))?;
    let old_commit = repo
        .find_commit(old_oid)
        .map_err(|_| Error::InvalidPushRequest(format!("nonexistent old ref: {old}")))?;
    let old_tree = old_commit.tree().unwrap();

    // parse new reference and get related tree
    let new_oid: Oid = new
        .parse()
        .map_err(|_| Error::InvalidPushRequest(format!("invalid new ref: {new}")))?;
    let new_commit = repo
        .find_commit(new_oid)
        .map_err(|_| Error::InvalidPushRequest(format!("nonexistent new ref: {new}")))?;
    let new_tree = new_commit.tree().unwrap();

    repo.diff_tree_to_tree(Some(&old_tree), Some(&new_tree), None)
        .map_err(|e| Error::InvalidPushRequest(format!("failed diff: {old} -> {new}: {e}")))
}
