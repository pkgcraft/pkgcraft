use camino::Utf8Path;
use ini::Ini;

use crate::repo::Repo;
use crate::Error;

/// Load repos.conf settings from a given directory or file.
pub(super) fn load_repos_conf<P: AsRef<Utf8Path>>(path: P) -> crate::Result<Vec<Repo>> {
    let path = path.as_ref();
    let files: Vec<_> = match path.read_dir_utf8() {
        Ok(entries) => Ok(entries
            .filter_map(|d| d.ok())
            .map(|d| d.path().to_path_buf())
            .collect()),
        // TODO: switch to `e.kind() == ErrorKind::NotADirectory` on rust stabilization
        // https://github.com/rust-lang/rust/issues/86442
        Err(e) if e.raw_os_error() == Some(20) => Ok(vec![path.to_path_buf()]),
        Err(e) => Err(Error::Config(format!("failed reading repos.conf: {path}: {e}"))),
    }?;

    let mut repos = vec![];

    for f in files {
        Ini::load_from_file(&f)
            .map_err(|e| Error::Config(format!("invalid repos.conf file: {f:?}: {e}")))
            .and_then(|ini| {
                for (name, settings) in ini.iter().filter_map(|(section, p)| match section {
                    Some(s) if s != "DEFAULT" => Some((s, p)),
                    _ => None,
                }) {
                    // pull supported fields from config
                    let priority = settings.get("priority").unwrap_or("0").parse().unwrap_or(0);
                    let path = settings.get("location").ok_or_else(|| {
                        Error::Config(format!(
                            "invalid repos.conf file: {f:?}: missing location field: {name}"
                        ))
                    })?;

                    repos.push(Repo::from_path(name, priority, path, false)?);
                }
                Ok(())
            })?;
    }

    Ok(repos)
}
