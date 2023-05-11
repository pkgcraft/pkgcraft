use camino::Utf8Path;
use ini::Ini;

use crate::repo::Repo;
use crate::Error;

/// Load repos from a given repos.conf path.
pub(super) fn load_repos_conf<P: AsRef<Utf8Path>>(path: P) -> crate::Result<Vec<Repo>> {
    let path = path.as_ref();

    // expand directory path into files
    let files = match path.read_dir_utf8() {
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

        ini.iter()
            .filter_map(|(section, p)| match section {
                Some(s) if s != "DEFAULT" => Some((s, p)),
                _ => None,
            })
            .map(|(name, settings)| {
                // pull supported fields from config
                let priority = settings.get("priority").unwrap_or("0").parse().unwrap_or(0);
                if let Some(path) = settings.get("location") {
                    Repo::from_path(name, priority, path, false)
                } else {
                    Err(Error::Config(format!(
                        "invalid repos.conf file: {f:?}: missing location field: {name}"
                    )))
                }
            })
            .collect()
    };

    files
        .iter()
        .map(|f| repos_from_file(f))
        .collect::<crate::Result<Vec<Vec<_>>>>()
        .map(|repos| repos.into_iter().flatten().collect())
}
