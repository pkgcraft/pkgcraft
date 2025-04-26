use std::io;

use camino::Utf8Path;
use ini::Ini;
use itertools::Itertools;
use tracing::{error, info};

use crate::Error;
use crate::repo::{Repo, Repository};

/// Load repos from a repos.conf file.
fn repos_from_file(path: &Utf8Path) -> crate::Result<Vec<Repo>> {
    let ini = Ini::load_from_file(path)
        .map_err(|e| Error::Config(format!("invalid repos.conf file: {path}: {e}")))?;

    let repos: Vec<_> = ini
        .iter()
        .filter_map(|(section, settings)| match section {
            Some(name) if name != "DEFAULT" => Some((name, settings)),
            _ => None,
        })
        .filter_map(|(name, settings)| {
            // pull supported fields from config
            let priority = settings.get("priority").unwrap_or("0").parse().unwrap_or(0);
            let Some(repo_path) = settings.get("location") else {
                error!("invalid repos.conf file: {path}: missing location field: {name}");
                return None;
            };

            // ignore invalid repos
            match Repo::from_path(name, repo_path, priority) {
                Ok(repo) => Some(repo),
                Err(err) => {
                    error!("{err}");
                    None
                }
            }
        })
        .collect();

    // log repos loaded from the file
    let msg = if !repos.is_empty() {
        repos.iter().map(|r| r.id()).join(", ")
    } else {
        "no repos found".to_string()
    };

    info!("loading portage config: {path}: {msg}");
    Ok(repos)
}

/// Load all repos defined in a repos.conf path.
///
/// For directory targets, files are loaded in lexical order.
pub(super) fn load_repos_conf<P: AsRef<Utf8Path>>(path: P) -> crate::Result<Vec<Repo>> {
    let path = path.as_ref();

    // expand directory path into files
    let mut files = match path.read_dir_utf8() {
        Ok(entries) => Ok(entries
            .filter_map(Result::ok)
            .map(|d| d.path().to_path_buf())
            .collect()),
        Err(e) if e.kind() == io::ErrorKind::NotADirectory => Ok(vec![path.to_path_buf()]),
        Err(e) => Err(Error::Config(format!("failed reading repos.conf: {path}: {e}"))),
    }?;

    // load ini files in lexical order
    files.sort();
    let repos: Vec<_> = files.iter().map(|f| repos_from_file(f)).try_collect()?;
    Ok(repos.into_iter().flatten().collect())
}
