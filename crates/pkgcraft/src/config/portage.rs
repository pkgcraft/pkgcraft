use camino::Utf8Path;
use ini::Ini;
use itertools::Itertools;
use tracing::info;

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
        Err(e) => return Err(Error::Config(format!("failed reading repos.conf: {path}: {e}"))),
    };

    // load all repos from a given file
    let repos_from_file = |f: &Utf8Path| -> crate::Result<Vec<Repo>> {
        let ini = Ini::load_from_file(f)
            .map_err(|e| Error::Config(format!("invalid repos.conf file: {f:?}: {e}")))?;

        let repos: Result<Vec<_>, _> = ini
            .iter()
            .filter_map(|(section, p)| match section {
                Some(s) if s != "DEFAULT" => Some((s, p)),
                _ => None,
            })
            .map(|(name, settings)| {
                // pull supported fields from config
                let priority = settings.get("priority").unwrap_or("0").parse().unwrap_or(0);
                if let Some(path) = settings.get("location") {
                    Repo::from_path(name, path, priority, false)
                } else {
                    Err(Error::Config(format!(
                        "invalid repos.conf file: {f:?}: missing location field: {name}"
                    )))
                }
            })
            .collect();

        // log repos loaded from the file
        if let Ok(vals) = &repos {
            let msg = if !vals.is_empty() {
                vals.iter().map(|r| r.id()).join(", ")
            } else {
                "no repos found".to_string()
            };

            info!("loading portage config: {f}: {msg}");
        }

        repos
    };

    // load ini files in lexical order
    files.sort();

    files
        .iter()
        .map(|f| repos_from_file(f))
        .collect::<crate::Result<Vec<Vec<_>>>>()
        .map(|repos| repos.into_iter().flatten().collect())
}
