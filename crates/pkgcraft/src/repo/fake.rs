use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::{fmt, fs};

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::{IndexMap, IndexSet};
use tracing::error;

use crate::config::RepoConfig;
use crate::dep::{Cpn, Cpv, Dep, Version};
use crate::pkg::fake::Pkg;
use crate::restrict::{Restrict, Restriction};
use crate::traits::Contains;
use crate::types::OrderedSet;
use crate::Error;

use super::{make_repo_traits, PkgRepository, RepoFormat, Repository};

type VersionMap = IndexMap<String, IndexSet<Version>>;
type PkgMap = IndexMap<String, VersionMap>;

#[derive(Debug)]
struct InternalFakeRepo {
    id: String,
    repo_config: RepoConfig,
    pkgmap: PkgMap,
    cpvs: OrderedSet<Cpv>,
}

#[derive(Debug, Clone)]
pub struct FakeRepo(Arc<InternalFakeRepo>);

impl PartialEq for FakeRepo {
    fn eq(&self, other: &Self) -> bool {
        self.0.id == other.0.id && self.0.cpvs == other.0.cpvs
    }
}

impl Eq for FakeRepo {}

impl Hash for FakeRepo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.id.hash(state);
        self.0.cpvs.hash(state);
    }
}

make_repo_traits!(FakeRepo);

impl FakeRepo {
    pub fn new(id: &str, priority: i32) -> Self {
        Self(Arc::new(InternalFakeRepo {
            id: id.to_string(),
            repo_config: RepoConfig { priority, ..Default::default() },
            pkgmap: Default::default(),
            cpvs: Default::default(),
        }))
    }

    pub fn pkgs<I>(mut self, iter: I) -> crate::Result<Self>
    where
        I: IntoIterator,
        I::Item: TryInto<Cpv>,
        <I::Item as TryInto<Cpv>>::Error: fmt::Display,
    {
        self.extend(iter)?;
        Ok(self)
    }

    pub fn from_path<P: AsRef<Utf8Path>, S: AsRef<str>>(
        id: S,
        priority: i32,
        path: P,
    ) -> crate::Result<Self> {
        let id = id.as_ref();
        let path = path.as_ref();
        let data = fs::read_to_string(path).map_err(|e| Error::NotARepo {
            kind: RepoFormat::Fake,
            id: id.to_string(),
            err: e.to_string(),
        })?;
        let repo_config = RepoConfig {
            location: Utf8PathBuf::from(path),
            priority,
            ..Default::default()
        };
        let mut repo = Self(Arc::new(InternalFakeRepo {
            id: id.to_string(),
            repo_config,
            pkgmap: Default::default(),
            cpvs: Default::default(),
        }));
        repo.extend(data.lines())?;
        Ok(repo)
    }

    pub(super) fn repo_config(&self) -> &RepoConfig {
        &self.0.repo_config
    }

    pub fn extend<I>(&mut self, iter: I) -> crate::Result<()>
    where
        I: IntoIterator,
        I::Item: TryInto<Cpv>,
        <I::Item as TryInto<Cpv>>::Error: fmt::Display,
    {
        let repo = Arc::get_mut(&mut self.0)
            .ok_or_else(|| Error::InvalidValue("repo already in use".to_string()))?;
        let orig_len = repo.cpvs.len();
        for s in iter {
            match s.try_into() {
                Ok(cpv) => {
                    repo.cpvs.insert(cpv);
                }
                Err(e) => error!("{e}"),
            }
        }

        if orig_len != repo.cpvs.len() {
            repo.cpvs.sort();

            // recreate entire PkgMap structure to preserve correct ordering
            let mut pkgmap = PkgMap::new();
            for cpv in &repo.cpvs {
                pkgmap
                    .entry(cpv.category().into())
                    .or_default()
                    .entry(cpv.package().into())
                    .or_default()
                    .insert(cpv.version().clone());
            }
            repo.pkgmap = pkgmap;
        }

        Ok(())
    }
}

impl fmt::Display for FakeRepo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0.id)
    }
}

impl PkgRepository for FakeRepo {
    type Pkg = Pkg;
    type IterCpv = IterCpv;
    type IterCpvRestrict = IterCpvRestrict;
    type Iter = Iter;
    type IterRestrict = IterRestrict;

    // TODO: cache categories/packages/versions values in OnceCell fields?
    fn categories(&self) -> IndexSet<String> {
        self.0.pkgmap.keys().cloned().collect()
    }

    fn packages(&self, cat: &str) -> IndexSet<String> {
        self.0
            .pkgmap
            .get(cat)
            .map(|pkgs| pkgs.keys().cloned().collect())
            .unwrap_or_default()
    }

    fn versions(&self, cat: &str, pkg: &str) -> IndexSet<Version> {
        self.0
            .pkgmap
            .get(cat)
            .and_then(|pkgs| pkgs.get(pkg))
            .cloned()
            .unwrap_or_default()
    }

    fn len(&self) -> usize {
        self.0.cpvs.len()
    }

    fn iter_cpv(&self) -> Self::IterCpv {
        IterCpv {
            iter: self.0.cpvs.clone().into_iter(),
        }
    }

    fn iter_cpv_restrict<R: Into<Restrict>>(&self, value: R) -> Self::IterCpvRestrict {
        IterCpvRestrict {
            iter: self.iter_cpv(),
            restrict: value.into(),
        }
    }

    fn iter(&self) -> Self::Iter {
        self.into_iter()
    }

    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> Self::IterRestrict {
        IterRestrict {
            iter: self.into_iter(),
            restrict: val.into(),
        }
    }
}

impl Contains<&Cpn> for FakeRepo {
    fn contains(&self, cpn: &Cpn) -> bool {
        self.iter_restrict(cpn).next().is_some()
    }
}

impl Contains<&Cpv> for FakeRepo {
    fn contains(&self, cpv: &Cpv) -> bool {
        self.0.cpvs.contains(cpv)
    }
}

impl Contains<&Dep> for FakeRepo {
    fn contains(&self, dep: &Dep) -> bool {
        self.iter_restrict(dep).next().is_some()
    }
}

impl Repository for FakeRepo {
    fn format(&self) -> RepoFormat {
        self.0.repo_config.format
    }

    fn id(&self) -> &str {
        &self.0.id
    }

    fn priority(&self) -> i32 {
        self.0.repo_config.priority
    }

    fn path(&self) -> &Utf8Path {
        &self.0.repo_config.location
    }

    fn sync(&self) -> crate::Result<()> {
        self.0.repo_config.sync()
    }
}

impl IntoIterator for &FakeRepo {
    type Item = Pkg;
    type IntoIter = Iter;

    fn into_iter(self) -> Self::IntoIter {
        Iter {
            iter: self.0.cpvs.clone().into_iter(),
            repo: self.clone(),
        }
    }
}

#[derive(Debug)]
pub struct IterCpv {
    iter: indexmap::set::IntoIter<Cpv>,
}

impl Iterator for IterCpv {
    type Item = Cpv;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

#[derive(Debug)]
pub struct IterCpvRestrict {
    iter: IterCpv,
    restrict: Restrict,
}

impl Iterator for IterCpvRestrict {
    type Item = Cpv;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find(|cpv| self.restrict.matches(cpv))
    }
}

#[derive(Debug)]
pub struct Iter {
    iter: indexmap::set::IntoIter<Cpv>,
    repo: FakeRepo,
}

impl Iterator for Iter {
    type Item = Pkg;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|cpv| Pkg::new(cpv, self.repo.clone()))
    }
}

#[derive(Debug)]
pub struct IterRestrict {
    iter: Iter,
    restrict: Restrict,
}

impl Iterator for IterRestrict {
    type Item = Pkg;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find(|pkg| self.restrict.matches(pkg))
    }
}

#[cfg(test)]
mod tests {
    use tracing_test::traced_test;

    use crate::dep::Dep;
    use crate::macros::assert_logs_re;
    use crate::pkg::Package;
    use crate::repo::Contains;
    use crate::test::assert_ordered_eq;

    use super::*;

    #[test]
    fn id() {
        let repo = FakeRepo::new("fake", 0);
        assert_eq!(repo.id(), "fake");
    }

    #[test]
    fn categories() {
        let mut repo = FakeRepo::new("fake", 0);
        // empty repo
        assert!(repo.categories().is_empty());
        // existing pkgs
        repo.extend(["cat1/pkg-a-1", "cat1/pkg-a-2", "cat2/pkg-b-3"])
            .unwrap();
        assert_ordered_eq!(repo.categories(), ["cat1", "cat2"])
    }

    #[test]
    fn packages() {
        let mut repo: FakeRepo;
        // empty repo
        repo = FakeRepo::new("fake", 0);
        assert!(repo.packages("cat").is_empty());
        // existing pkgs
        repo.extend(["cat1/pkg-a-1", "cat1/pkg-a-2", "cat2/pkg-b-3"])
            .unwrap();
        assert!(repo.packages("cat").is_empty());
        assert_ordered_eq!(repo.packages("cat1"), ["pkg-a"]);
        assert_ordered_eq!(repo.packages("cat2"), ["pkg-b"]);
    }

    #[test]
    fn versions() {
        let ver = |s: &str| Version::try_new(s).unwrap();
        let mut repo: FakeRepo;
        // empty repo
        repo = FakeRepo::new("fake", 0);
        assert!(repo.versions("cat", "pkg").is_empty());
        // existing pkgs
        repo.extend(["cat1/pkg-a-1", "cat1/pkg-a-2", "cat2/pkg-b-3"])
            .unwrap();
        assert!(repo.versions("cat", "pkg").is_empty());
        assert_ordered_eq!(repo.versions("cat1", "pkg-a"), [ver("1"), ver("2")]);
        assert_ordered_eq!(repo.versions("cat2", "pkg-b"), [ver("3")]);
    }

    #[test]
    fn len() {
        let mut repo = FakeRepo::new("fake", 0);
        assert_eq!(repo.len(), 0);
        repo.extend(["cat/pkg-0"]).unwrap();
        assert_eq!(repo.len(), 1);
        repo.extend(["cat/pkg-0", "cat1/pkg1-1", "cat2/pkg2-2"])
            .unwrap();
        assert_eq!(repo.len(), 3);
    }

    #[traced_test]
    #[test]
    fn extend() {
        let mut repo = FakeRepo::new("fake", 0).pkgs(["cat/pkg-2"]).unwrap();
        assert_ordered_eq!(repo.iter().map(|x| x.cpv().to_string()), ["cat/pkg-2"]);

        // add valid cpv
        repo.extend(["cat/pkg-0"]).unwrap();
        assert_ordered_eq!(repo.iter().map(|x| x.cpv().to_string()), ["cat/pkg-0", "cat/pkg-2"]);

        // add multiple cpvs, invalid cpvs logged and ignored
        repo.extend(["cat/pkg-3", "cat/pkg", "cat/pkg-1", "a/b-0"])
            .unwrap();
        assert_ordered_eq!(
            repo.iter().map(|x| x.cpv().to_string()),
            ["a/b-0", "cat/pkg-0", "cat/pkg-1", "cat/pkg-2", "cat/pkg-3"]
        );
        assert_logs_re!("invalid cpv: cat/pkg");

        // re-add existing cpvs
        repo.extend(["cat/pkg-3", "cat/pkg-1", "a/b-0"]).unwrap();
        assert_ordered_eq!(
            repo.iter().map(|x| x.cpv().to_string()),
            ["a/b-0", "cat/pkg-0", "cat/pkg-1", "cat/pkg-2", "cat/pkg-3"]
        );
    }

    #[test]
    fn contains() {
        let repo = FakeRepo::new("fake", 0).pkgs(["cat/pkg-0"]).unwrap();

        // path is always false due to fake repo
        assert!(!repo.contains("cat/pkg"));

        // cpv
        let cpv = Cpv::try_new("cat/pkg-0").unwrap();
        assert!(repo.contains(&cpv));
        let cpv = Cpv::try_new("cat/pkg-1").unwrap();
        assert!(!repo.contains(&cpv));

        // unversioned dep
        let a = Dep::try_new("cat/pkg").unwrap();
        assert!(repo.contains(&a));
        let a = Dep::try_new("cat/pkg-a").unwrap();
        assert!(!repo.contains(&a));
    }

    #[test]
    fn iter() {
        let repo = FakeRepo::new("fake", 0)
            .pkgs(["cat/pkg-0", "acat/bpkg-1"])
            .unwrap();
        let cpvs: Vec<_> = repo.iter().map(|pkg| pkg.cpv().to_string()).collect();
        assert_eq!(cpvs, ["acat/bpkg-1", "cat/pkg-0"]);
    }
}
