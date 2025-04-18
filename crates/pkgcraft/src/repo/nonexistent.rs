use std::hash::{Hash, Hasher};
use std::{fmt, iter};

use camino::Utf8Path;
use indexmap::IndexSet;

use crate::config::RepoConfig;
use crate::dep::{Cpn, Cpv, Dep, Version};
use crate::pkg::Pkg;
use crate::restrict::Restrict;
use crate::traits::Contains;
use crate::Error;

use super::{make_repo_traits, PkgRepository, RepoFormat, Repository};

/// Nonexistent repo only defined via the config.
pub struct NonexistentRepo {
    id: String,
    repo_config: RepoConfig,
}

impl fmt::Debug for NonexistentRepo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("NonexistentRepo")
            .field("id", &self.id())
            .finish()
    }
}

impl PartialEq for NonexistentRepo {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for NonexistentRepo {}

impl Hash for NonexistentRepo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

make_repo_traits!(NonexistentRepo);

impl NonexistentRepo {
    pub(crate) fn new(id: &str, priority: i32) -> Self {
        let repo_config = RepoConfig {
            priority,
            ..RepoFormat::Nonexistent.into()
        };

        Self {
            id: id.to_string(),
            repo_config,
        }
    }

    pub(super) fn from_path<P: AsRef<Utf8Path>, S: AsRef<str>>(
        id: S,
        priority: i32,
        path: P,
    ) -> crate::Result<Self> {
        let id = id.as_ref();
        let path = path.as_ref();
        if !path.exists() {
            Ok(Self::new(id, priority))
        } else {
            Err(Error::NotARepo {
                kind: RepoFormat::Nonexistent,
                id: id.to_string(),
                err: "repo dir exists".to_string(),
            })
        }
    }

    pub(super) fn repo_config(&self) -> &RepoConfig {
        &self.repo_config
    }
}

impl fmt::Display for NonexistentRepo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

impl PkgRepository for NonexistentRepo {
    type Pkg = Pkg;
    type IterCpn = iter::Empty<Cpn>;
    type IterCpnRestrict = iter::Empty<Cpn>;
    type IterCpv = iter::Empty<Cpv>;
    type IterCpvRestrict = iter::Empty<Cpv>;
    type Iter = iter::Empty<crate::Result<Self::Pkg>>;
    type IterRestrict = iter::Empty<crate::Result<Self::Pkg>>;

    fn categories(&self) -> IndexSet<String> {
        Default::default()
    }

    fn packages(&self, _cat: &str) -> IndexSet<String> {
        Default::default()
    }

    fn versions(&self, _cat: &str, _pkg: &str) -> IndexSet<Version> {
        Default::default()
    }

    fn len(&self) -> usize {
        0
    }

    fn iter_cpn(&self) -> Self::IterCpn {
        iter::empty()
    }

    fn iter_cpn_restrict<R: Into<Restrict>>(&self, _value: R) -> Self::IterCpnRestrict {
        iter::empty()
    }

    fn iter_cpv(&self) -> Self::IterCpv {
        iter::empty()
    }

    fn iter_cpv_restrict<R: Into<Restrict>>(&self, _value: R) -> Self::IterCpvRestrict {
        iter::empty()
    }

    fn iter(&self) -> Self::Iter {
        iter::empty()
    }

    fn iter_restrict<R: Into<Restrict>>(&self, _val: R) -> Self::IterRestrict {
        iter::empty()
    }
}

impl Contains<&Cpn> for NonexistentRepo {
    fn contains(&self, _: &Cpn) -> bool {
        false
    }
}

impl Contains<&Cpv> for NonexistentRepo {
    fn contains(&self, _: &Cpv) -> bool {
        false
    }
}

impl Contains<&Dep> for NonexistentRepo {
    fn contains(&self, _: &Dep) -> bool {
        false
    }
}

impl Repository for NonexistentRepo {
    fn format(&self) -> RepoFormat {
        self.repo_config.format
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn priority(&self) -> i32 {
        self.repo_config.priority
    }

    fn path(&self) -> &Utf8Path {
        &self.repo_config.location
    }

    fn sync(&self) -> crate::Result<()> {
        self.repo_config.sync()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains() {
        let repo = NonexistentRepo::new("nonexistent", 0);

        // path
        assert!(!repo.contains("cat/pkg"));

        // versioned dep
        let cpv = Cpv::try_new("cat/pkg-0").unwrap();
        assert!(!repo.contains(&cpv));

        // unversioned dep
        let a = Dep::try_new("cat/pkg").unwrap();
        assert!(!repo.contains(&a));
    }

    #[test]
    fn iter() {
        let repo = NonexistentRepo::new("nonexistent", 0);
        assert!(repo.iter().next().is_none());
    }
}
