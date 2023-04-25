use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::{fs, io};

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexSet;
use once_cell::sync::OnceCell;
use strum::{Display, EnumString};
use tracing::{error, warn};

#[derive(Display, EnumString, Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[strum(serialize_all = "snake_case")]
pub enum ArchStatus {
    Stable,
    Testing,
    Transitional,
}

#[derive(Debug, Default)]
pub struct Metadata {
    repo: String,
    profiles_base: Utf8PathBuf,
    arches: OnceCell<IndexSet<String>>,
    arches_desc: OnceCell<HashMap<ArchStatus, HashSet<String>>>,
    categories: OnceCell<IndexSet<String>>,
}

impl Metadata {
    pub(super) fn new<P: Into<Utf8PathBuf>>(repo: &str, path: P) -> Self {
        Self {
            repo: repo.to_string(),
            profiles_base: path.into(),
            ..Default::default()
        }
    }

    /// Return the full path to a repo's `profiles` directory.
    pub fn profiles_base(&self) -> &Utf8Path {
        &self.profiles_base
    }

    /// Return a repo's known architectures from `profiles/arch.list`.
    pub fn arches(&self) -> &IndexSet<String> {
        self.arches.get_or_init(|| {
            let path = self.profiles_base.join("arch.list");
            match fs::read_to_string(path) {
                Ok(s) => s
                    .lines()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty() && !s.starts_with('#'))
                    .map(String::from)
                    .collect(),
                Err(e) => {
                    if e.kind() != io::ErrorKind::NotFound {
                        warn!("{}::profiles/arch.list: {e}", self.repo);
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
            let path = self.profiles_base.join("arches.desc");
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
                                        self.repo,
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
                                        self.repo, i + 1
                                    );
                                }
                            }
                            _ => error!(
                                "{}::profiles/arches.desc, line {}: \
                                invalid line format: should be '<arch> <status>'",
                                self.repo,
                                i + 1
                            ),
                        })
                }
                Err(e) => {
                    if e.kind() != io::ErrorKind::NotFound {
                        warn!("{}::profiles/arches.desc: {e}", self.repo);
                    }
                }
            }
            vals
        })
    }

    /// Return a repo's configured categories from `profiles/categories`.
    pub fn categories(&self) -> &IndexSet<String> {
        self.categories.get_or_init(|| {
            let path = self.profiles_base.join("categories");
            match fs::read_to_string(path) {
                Ok(s) => s
                    .lines()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty() && !s.starts_with('#'))
                    .map(String::from)
                    .collect(),
                Err(e) => {
                    if e.kind() != io::ErrorKind::NotFound {
                        warn!("{}::profiles/categories: {e}", self.repo);
                    }
                    IndexSet::new()
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
    fn test_arches() {
        let repo = TempRepo::new("test", None, None).unwrap();
        let mut metadata: Metadata;

        // nonexistent file
        metadata = Metadata::new("test", repo.path());
        assert!(metadata.arches().is_empty());

        // empty file
        metadata = Metadata::new("test", repo.path());
        fs::write(metadata.profiles_base().join("arch.list"), "").unwrap();
        assert!(metadata.arches().is_empty());

        // multiple
        let data = indoc::indoc! {r#"
            amd64
            arm64
            amd64-linux
        "#};
        metadata = Metadata::new("test", repo.path());
        fs::write(metadata.profiles_base().join("arch.list"), data).unwrap();
        assert_ordered_eq(metadata.arches(), ["amd64", "arm64", "amd64-linux"]);
    }

    #[traced_test]
    #[test]
    fn test_arches_desc() {
        let repo = TempRepo::new("test", None, None).unwrap();
        let mut metadata: Metadata;

        // nonexistent file
        metadata = Metadata::new("test", repo.path());
        assert!(metadata.arches_desc().is_empty());

        // empty file
        metadata = Metadata::new("test", repo.path());
        fs::write(metadata.profiles_base().join("arches.desc"), "").unwrap();
        assert!(metadata.arches_desc().is_empty());

        // invalid line format
        metadata = Metadata::new("test", repo.path());
        fs::write(metadata.profiles_base().join("arch.list"), "amd64\narm64").unwrap();
        fs::write(metadata.profiles_base().join("arches.desc"), "amd64 stable\narm64").unwrap();
        assert!(!metadata.arches_desc().is_empty());
        assert_logs_re!(format!(".+, line 2: invalid line format: .+$"));

        // unknown arch
        metadata = Metadata::new("test", repo.path());
        fs::write(metadata.profiles_base().join("arch.list"), "amd64").unwrap();
        fs::write(metadata.profiles_base().join("arches.desc"), "arm64 stable").unwrap();
        assert!(metadata.arches_desc().is_empty());
        assert_logs_re!(format!(".+, line 1: unknown arch: arm64$"));

        // unknown status
        metadata = Metadata::new("test", repo.path());
        fs::write(metadata.profiles_base().join("arch.list"), "amd64").unwrap();
        fs::write(metadata.profiles_base().join("arches.desc"), "amd64 test").unwrap();
        assert!(metadata.arches_desc().is_empty());
        assert_logs_re!(format!(".+, line 1: unknown status: test$"));

        // multiple with ignored 3rd column
        metadata = Metadata::new("test", repo.path());
        fs::write(metadata.profiles_base().join("arch.list"), "amd64\narm64\nppc64").unwrap();
        fs::write(
            metadata.profiles_base().join("arches.desc"),
            "amd64 stable\narm64 testing\nppc64 transitional 3rd-col",
        )
        .unwrap();
        assert_unordered_eq(&metadata.arches_desc()[&ArchStatus::Stable], ["amd64"]);
        assert_unordered_eq(&metadata.arches_desc()[&ArchStatus::Testing], ["arm64"]);
        assert_unordered_eq(&metadata.arches_desc()[&ArchStatus::Transitional], ["ppc64"]);
    }

    #[test]
    fn test_categories() {
        let repo = TempRepo::new("test", None, None).unwrap();
        let mut metadata: Metadata;

        // nonexistent file
        metadata = Metadata::new("test", repo.path());
        assert!(metadata.categories().is_empty());

        // empty file
        metadata = Metadata::new("test", repo.path());
        fs::write(metadata.profiles_base().join("categories"), "").unwrap();
        assert!(metadata.categories().is_empty());

        // multiple
        let data = indoc::indoc! {r#"
            cat1
            cat2
            cat-3
        "#};
        metadata = Metadata::new("test", repo.path());
        fs::write(metadata.profiles_base().join("categories"), data).unwrap();
        assert_ordered_eq(metadata.categories(), ["cat1", "cat2", "cat-3"]);
    }
}
