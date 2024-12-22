use camino::Utf8Path;
use ini::Ini;
use itertools::Itertools;
use tracing::{error, info};

use crate::repo::{Repo, Repository};
use crate::Error;

/// Load repos from a given repos.conf path.
pub(super) fn load_repos_conf<P: AsRef<Utf8Path>>(path: P) -> crate::Result<Vec<Repo>> {
    let path = path.as_ref();

    // expand directory path into files
    let mut files = match path.read_dir_utf8() {
        Ok(entries) => entries
            .filter_map(|d| d.ok())
            .map(|d| d.path().to_path_buf())
            .collect(),
        // TODO: switch to `e.kind() == ErrorKind::NotADirectory` on rust stabilization
        // https://github.com/rust-lang/rust/issues/86442
        Err(e) if e.raw_os_error() == Some(20) => vec![path.to_path_buf()],
        Err(e) => {
            return Err(Error::Config(format!("failed reading repos.conf: {path}: {e}")))
        }
    };

    // load all repos from a given file
    let repos_from_file = |f: &Utf8Path| -> crate::Result<Vec<Repo>> {
        let ini = Ini::load_from_file(f)
            .map_err(|e| Error::Config(format!("invalid repos.conf file: {f:?}: {e}")))?;

        let repos: Vec<_> = ini
            .iter()
            .filter_map(|(section, p)| match section {
                Some(s) if s != "DEFAULT" => Some((s, p)),
                _ => None,
            })
            .filter_map(|(name, settings)| {
                // pull supported fields from config
                let priority = settings.get("priority").unwrap_or("0").parse().unwrap_or(0);
                let Some(path) = settings.get("location") else {
                    error!("invalid repos.conf file: {f:?}: missing location field: {name}");
                    return None;
                };

                // ignore invalid repos
                match Repo::from_path(name, path, priority) {
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

        info!("loading portage config: {f}: {msg}");
        Ok(repos)
    };

    // load ini files in lexical order
    files.sort();
    let repos: Vec<_> = files.iter().map(|f| repos_from_file(f)).try_collect()?;
    Ok(repos.into_iter().flatten().collect())
}
