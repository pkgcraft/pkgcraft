use std::str::FromStr;

use git2::{Diff, Oid};
use itertools::Itertools;

use crate::{Error, Result};

tonic::include_proto!("pkgcruft");

impl PushRequest {
    /// Try converting a push request into a git diff.
    pub fn diff<'a>(&self, git_repo: &'a git2::Repository) -> Result<Diff<'a>> {
        // parse old reference and get related tree
        let old_ref = &self.old_ref;
        let old_oid: Oid = old_ref
            .parse()
            .map_err(|_| Error::InvalidPushRequest(format!("invalid old ref: {old_ref}")))?;
        let old_tree = git_repo.find_tree(old_oid).map_err(|_| {
            Error::InvalidPushRequest(format!("nonexistent old ref: {old_ref}"))
        })?;

        // parse new reference and get related tree
        let new_ref = &self.new_ref;
        let new_oid: Oid = new_ref
            .parse()
            .map_err(|_| Error::InvalidPushRequest(format!("invalid new ref: {new_ref}")))?;
        let new_tree = git_repo.find_tree(new_oid).map_err(|_| {
            Error::InvalidPushRequest(format!("nonexistent new ref: {new_ref}"))
        })?;

        git_repo
            .diff_tree_to_tree(Some(&old_tree), Some(&new_tree), None)
            .map_err(|e| {
                Error::InvalidPushRequest(format!(
                    "failed creating diff: {old_ref} -> {new_ref}: {e}"
                ))
            })
    }
}

impl FromStr for PushRequest {
    type Err = Error;

    fn from_str(line: &str) -> Result<Self> {
        let (old_ref, new_ref, ref_name) = line
            .split(' ')
            .map(|s| s.to_string())
            .collect_tuple()
            .ok_or_else(|| Error::InvalidPushRequest(line.to_string()))?;
        Ok(Self { old_ref, new_ref, ref_name })
    }
}
