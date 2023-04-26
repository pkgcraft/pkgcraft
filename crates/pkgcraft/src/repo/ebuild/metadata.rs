use std::collections::{HashMap, HashSet};
use std::str::{FromStr, SplitWhitespace};
use std::{fmt, fs, io};

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexSet;
use ini::Ini;
use once_cell::sync::OnceCell;
use strum::{Display, EnumString};
use tracing::{error, warn};

use crate::dep::{parse, Dep};
use crate::eapi::{Eapi, EAPI0};
use crate::repo::RepoFormat;
use crate::Error;

const DEFAULT_SECTION: Option<String> = None;

pub struct IniConfig {
    path: Option<Utf8PathBuf>,
    ini: Ini,
}

impl Default for IniConfig {
    fn default() -> Self {
        Self { path: None, ini: Ini::new() }
    }
}

impl fmt::Debug for IniConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let section = self.ini.section(DEFAULT_SECTION);
        f.debug_struct("Metadata")
            .field("path", &self.path)
            .field("ini", &section)
            .finish()
    }
}

impl IniConfig {
    fn new(repo_path: &Utf8Path) -> Self {
        let path = repo_path.join("metadata/layout.conf");
        match Ini::load_from_file(&path) {
            Ok(ini) => Self { path: Some(path), ini },
            Err(ini::Error::Io(e)) if e.kind() == io::ErrorKind::NotFound => Self {
                path: Some(path),
                ini: Ini::new(),
            },
            Err(e) => {
                error!("invalid repo config: {path}: {e}");
                Self::default()
            }
        }
    }

    #[cfg(test)]
    fn set<S1, S2>(&mut self, key: S1, val: S2)
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        self.ini.set_to(DEFAULT_SECTION, key.into(), val.into());
    }

    #[cfg(test)]
    pub(crate) fn write(&self, data: Option<&str>) -> crate::Result<()> {
        use std::io::Write;
        if let Some(path) = &self.path {
            self.ini
                .write_to_file(path)
                .map_err(|e| Error::IO(e.to_string()))?;

            if let Some(data) = data {
                let mut f = fs::File::options()
                    .append(true)
                    .open(path)
                    .map_err(|e| Error::IO(e.to_string()))?;
                write!(f, "{data}").map_err(|e| Error::IO(e.to_string()))?;
            }
        }

        Ok(())
    }

    pub(super) fn iter(&self, key: &str) -> SplitWhitespace {
        self.ini
            .get_from(DEFAULT_SECTION, key)
            .unwrap_or_default()
            .split_whitespace()
    }

    pub fn properties_allowed(&self) -> HashSet<&str> {
        self.iter("properties-allowed").collect()
    }

    pub fn restrict_allowed(&self) -> HashSet<&str> {
        self.iter("restrict-allowed").collect()
    }

    pub fn is_empty(&self) -> bool {
        self.ini.is_empty()
    }
}

#[derive(Display, EnumString, Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[strum(serialize_all = "snake_case")]
pub enum ArchStatus {
    Stable,
    Testing,
    Transitional,
}

#[derive(Debug, Default)]
pub struct Metadata {
    pub(super) name: String,
    pub(super) eapi: &'static Eapi,
    config: IniConfig,
    path: Utf8PathBuf,
    arches: OnceCell<IndexSet<String>>,
    arches_desc: OnceCell<HashMap<ArchStatus, HashSet<String>>>,
    categories: OnceCell<IndexSet<String>>,
    pkg_mask: OnceCell<HashSet<Dep>>,
}

impl Metadata {
    pub(super) fn new(path: &Utf8Path) -> crate::Result<Self> {
        let invalid_repo = |err: String| -> Error {
            Error::InvalidRepo {
                format: RepoFormat::Ebuild,
                path: Utf8PathBuf::from(path),
                err,
            }
        };

        // verify repo name
        let repo_name_path = path.join("profiles/repo_name");
        let name = match fs::read_to_string(&repo_name_path) {
            Ok(data) => match data.lines().next() {
                // TODO: verify repo name matches spec
                Some(s) => s.trim_end().to_string(),
                None => {
                    let err = format!("invalid repo name: {repo_name_path}");
                    return Err(invalid_repo(err));
                }
            },
            Err(e) => {
                let err = format!("failed reading repo name: {repo_name_path}: {e}");
                return Err(invalid_repo(err));
            }
        };

        // verify repo EAPI
        let eapi_path = path.join("profiles/eapi");
        let eapi = match fs::read_to_string(&eapi_path) {
            Ok(data) => {
                let s = data.lines().next().unwrap_or_default();
                <&Eapi>::from_str(s.trim_end())
                    .map_err(|e| invalid_repo(format!("invalid repo eapi: {e}")))?
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => &*EAPI0,
            Err(e) => {
                let err = format!("failed reading repo eapi: {eapi_path}: {e}");
                return Err(invalid_repo(err));
            }
        };

        Ok(Self {
            name,
            eapi,
            config: IniConfig::new(path),
            path: Utf8PathBuf::from(path),
            ..Default::default()
        })
    }

    pub fn config(&self) -> &IniConfig {
        &self.config
    }

    /// Return a repo's known architectures from `profiles/arch.list`.
    pub fn arches(&self) -> &IndexSet<String> {
        self.arches.get_or_init(|| {
            let path = self.path.join("profiles/arch.list");
            match fs::read_to_string(path) {
                Ok(s) => s
                    .lines()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty() && !s.starts_with('#'))
                    .map(String::from)
                    .collect(),
                Err(e) => {
                    if e.kind() != io::ErrorKind::NotFound {
                        warn!("{}::profiles/arch.list: {e}", self.name);
                    }
                    IndexSet::new()
                }
            }
        })
    }

    /// Architecture stability status from `profiles/arches.desc`.
    /// See GLEP 72 (https://www.gentoo.org/glep/glep-0072.html).
    pub fn arches_desc(&self) -> &HashMap<ArchStatus, HashSet<String>> {
        self.arches_desc.get_or_init(|| {
            let path = self.path.join("profiles/arches.desc");
            let mut vals = HashMap::<ArchStatus, HashSet<String>>::new();
            match fs::read_to_string(path) {
                Ok(s) => {
                    s.lines()
                        .map(|s| s.trim())
                        .enumerate()
                        .filter(|(_, s)| !s.is_empty() && !s.starts_with('#'))
                        .map(|(i, s)| (i, s.split_whitespace()))
                        // only pull the first two columns, ignoring any additional
                        .for_each(|(i, mut iter)| match (iter.next(), iter.next()) {
                            (Some(arch), Some(status)) => {
                                if !self.arches().contains(arch) {
                                    warn!(
                                        "{}::profiles/arches.desc, line {}: unknown arch: {arch}",
                                        self.name,
                                        i + 1
                                    );
                                    return;
                                }

                                if let Ok(status) = ArchStatus::from_str(status) {
                                    let arches = vals.entry(status).or_insert_with(HashSet::new);
                                    arches.insert(arch.to_string());
                                } else {
                                    warn!(
                                        "{}::profiles/arches.desc, line {}: unknown status: {status}",
                                        self.name, i + 1
                                    );
                                }
                            }
                            _ => error!(
                                "{}::profiles/arches.desc, line {}: \
                                invalid line format: should be '<arch> <status>'",
                                self.name,
                                i + 1
                            ),
                        })
                }
                Err(e) => {
                    if e.kind() != io::ErrorKind::NotFound {
                        warn!("{}::profiles/arches.desc: {e}", self.name);
                    }
                }
            }
            vals
        })
    }

    /// Return a repo's configured categories from `profiles/categories`.
    pub fn categories(&self) -> &IndexSet<String> {
        self.categories.get_or_init(|| {
            let path = self.path.join("profiles/categories");
            match fs::read_to_string(path) {
                Ok(s) => s
                    .lines()
                    .map(|s| s.trim())
                    .enumerate()
                    .filter(|(_, s)| !s.is_empty() && !s.starts_with('#'))
                    .filter_map(|(i, s)| match parse::category(s) {
                        Ok(_) => Some(s.to_string()),
                        Err(e) => {
                            warn!("{}::profiles/categories, line {}: {e}", self.name, i + 1);
                            None
                        }
                    })
                    .collect(),
                Err(e) => {
                    if e.kind() != io::ErrorKind::NotFound {
                        warn!("{}::profiles/categories: {e}", self.name);
                    }
                    IndexSet::new()
                }
            }
        })
    }

    /// Return a repo's globally masked packages.
    pub fn pkg_mask(&self) -> &HashSet<Dep> {
        self.pkg_mask.get_or_init(|| {
            let path = self.path.join("profiles/package.mask");
            match fs::read_to_string(path) {
                Ok(s) => s
                    .lines()
                    .map(|s| s.trim())
                    .enumerate()
                    .filter(|(_, s)| !s.is_empty() && !s.starts_with('#'))
                    .filter_map(|(i, s)| match self.eapi.dep(s) {
                        Ok(dep) => Some(dep),
                        Err(e) => {
                            warn!("{}::profiles/package.mask, line {}: {e}", self.name, i + 1);
                            None
                        }
                    })
                    .collect(),
                Err(e) => {
                    if e.kind() != io::ErrorKind::NotFound {
                        warn!("{}::profiles/package.mask: {e}", self.name);
                    }
                    HashSet::new()
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use tracing_test::traced_test;

    use crate::macros::*;
    use crate::repo::ebuild_temp::Repo as TempRepo;
    use crate::test::{assert_ordered_eq, assert_unordered_eq};

    use super::*;

    #[test]
    fn test_config() {
        let t = TempRepo::new("test", None, None).unwrap();
        let metadata = Metadata::new(t.path()).unwrap();

        // invalid config
        metadata.config.write(Some("data")).unwrap();
        assert!(metadata.config().is_empty());
    }

    #[test]
    fn test_config_properties_and_restrict_allowed() {
        // empty
        let t = TempRepo::new("test", None, None).unwrap();
        let metadata = Metadata::new(t.path()).unwrap();
        assert!(metadata.config().properties_allowed().is_empty());
        assert!(metadata.config().restrict_allowed().is_empty());

        // existing
        let t = TempRepo::new("test", None, None).unwrap();
        let mut metadata = Metadata::new(t.path()).unwrap();
        metadata
            .config
            .set("properties-allowed", "interactive live");
        metadata.config.set("restrict-allowed", "fetch mirror");
        metadata.config.write(None).unwrap();
        assert_unordered_eq(metadata.config().properties_allowed(), ["live", "interactive"]);
        assert_unordered_eq(metadata.config().restrict_allowed(), ["fetch", "mirror"]);
    }

    #[test]
    fn test_arches() {
        let t = TempRepo::new("test", None, None).unwrap();

        // nonexistent file
        let metadata = Metadata::new(t.path()).unwrap();
        assert!(metadata.arches().is_empty());

        // empty file
        let metadata = Metadata::new(t.path()).unwrap();
        fs::write(metadata.path.join("profiles/arch.list"), "").unwrap();
        assert!(metadata.arches().is_empty());

        // multiple
        let data = indoc::indoc! {r#"
            amd64
            arm64
            amd64-linux
        "#};
        let metadata = Metadata::new(t.path()).unwrap();
        fs::write(metadata.path.join("profiles/arch.list"), data).unwrap();
        assert_ordered_eq(metadata.arches(), ["amd64", "arm64", "amd64-linux"]);
    }

    #[traced_test]
    #[test]
    fn test_arches_desc() {
        let t = TempRepo::new("test", None, None).unwrap();

        // nonexistent file
        let metadata = Metadata::new(t.path()).unwrap();
        assert!(metadata.arches_desc().is_empty());

        // empty file
        let metadata = Metadata::new(t.path()).unwrap();
        fs::write(metadata.path.join("profiles/arches.desc"), "").unwrap();
        assert!(metadata.arches_desc().is_empty());

        // invalid line format
        let metadata = Metadata::new(t.path()).unwrap();
        fs::write(metadata.path.join("profiles/arch.list"), "amd64\narm64").unwrap();
        fs::write(metadata.path.join("profiles/arches.desc"), "amd64 stable\narm64").unwrap();
        assert!(!metadata.arches_desc().is_empty());
        assert_logs_re!(format!(".+, line 2: invalid line format: .+$"));

        // unknown arch
        let metadata = Metadata::new(t.path()).unwrap();
        fs::write(metadata.path.join("profiles/arch.list"), "amd64").unwrap();
        fs::write(metadata.path.join("profiles/arches.desc"), "arm64 stable").unwrap();
        assert!(metadata.arches_desc().is_empty());
        assert_logs_re!(format!(".+, line 1: unknown arch: arm64$"));

        // unknown status
        let metadata = Metadata::new(t.path()).unwrap();
        fs::write(metadata.path.join("profiles/arch.list"), "amd64").unwrap();
        fs::write(metadata.path.join("profiles/arches.desc"), "amd64 test").unwrap();
        assert!(metadata.arches_desc().is_empty());
        assert_logs_re!(format!(".+, line 1: unknown status: test$"));

        // multiple with ignored 3rd column
        let metadata = Metadata::new(t.path()).unwrap();
        fs::write(metadata.path.join("profiles/arch.list"), "amd64\narm64\nppc64").unwrap();
        fs::write(
            metadata.path.join("profiles/arches.desc"),
            "amd64 stable\narm64 testing\nppc64 transitional 3rd-col",
        )
        .unwrap();
        assert_unordered_eq(&metadata.arches_desc()[&ArchStatus::Stable], ["amd64"]);
        assert_unordered_eq(&metadata.arches_desc()[&ArchStatus::Testing], ["arm64"]);
        assert_unordered_eq(&metadata.arches_desc()[&ArchStatus::Transitional], ["ppc64"]);
    }

    #[traced_test]
    #[test]
    fn test_categories() {
        let t = TempRepo::new("test", None, None).unwrap();

        // nonexistent file
        let metadata = Metadata::new(t.path()).unwrap();
        assert!(metadata.categories().is_empty());

        // empty file
        let metadata = Metadata::new(t.path()).unwrap();
        fs::write(metadata.path.join("profiles/categories"), "").unwrap();
        assert!(metadata.categories().is_empty());

        // multiple with invalid entry
        let metadata = Metadata::new(t.path()).unwrap();
        fs::write(metadata.path.join("profiles/categories"), "cat\nc@t").unwrap();
        assert_ordered_eq(metadata.categories(), ["cat"]);
        assert_logs_re!(format!(".+, line 2: .* invalid category name: c@t$"));

        // multiple
        let data = indoc::indoc! {r#"
            cat1
            cat2
            cat-3
        "#};
        let metadata = Metadata::new(t.path()).unwrap();
        fs::write(metadata.path.join("profiles/categories"), data).unwrap();
        assert_ordered_eq(metadata.categories(), ["cat1", "cat2", "cat-3"]);
    }
}
