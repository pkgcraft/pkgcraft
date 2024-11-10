use std::fmt;
use std::sync::Arc;

use crate::config::Settings;
use crate::dep::{Cpv, Dep, DependencySet, Evaluate, Uri};
use crate::eapi::Eapi;
use crate::macros::bool_not_equal;
use crate::pkg::{make_pkg_traits, Package, RepoPackage};
use crate::repo::ebuild::configured::ConfiguredRepo;
use crate::repo::Repository;
use crate::restrict::{Restrict as BaseRestrict, Restriction};
use crate::traits::Intersects;
use crate::types::OrderedSet;

use super::metadata::Key;
use super::EbuildPackage;

#[derive(Clone)]
pub struct Pkg {
    repo: ConfiguredRepo,
    settings: Arc<Settings>,
    raw: super::Pkg,
}

impl<'a> From<&'a Pkg> for &'a super::Pkg {
    fn from(pkg: &'a Pkg) -> Self {
        &pkg.raw
    }
}

make_pkg_traits!(Pkg);

impl fmt::Debug for Pkg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Pkg {{ {self} }}")
    }
}

impl Pkg {
    pub(crate) fn new(repo: ConfiguredRepo, settings: Arc<Settings>, raw: super::Pkg) -> Self {
        Self { repo, settings, raw }
    }

    /// Return a package's evaluated dependencies for a given iterable of descriptors.
    pub fn dependencies(&self, keys: &[Key]) -> DependencySet<&Dep> {
        self.raw
            .dependencies(keys)
            .evaluate(self.settings.options())
    }

    /// Return a configured package's evaluated BDEPEND.
    pub fn bdepend(&self) -> DependencySet<&Dep> {
        self.raw.data.bdepend.evaluate(self.settings.options())
    }

    /// Return a configured package's evaluated DEPEND.
    pub fn depend(&self) -> DependencySet<&Dep> {
        self.raw.data.depend.evaluate(self.settings.options())
    }

    /// Return a configured package's evaluated IDEPEND.
    pub fn idepend(&self) -> DependencySet<&Dep> {
        self.raw.data.idepend.evaluate(self.settings.options())
    }

    /// Return a configured package's evaluated PDEPEND.
    pub fn pdepend(&self) -> DependencySet<&Dep> {
        self.raw.data.pdepend.evaluate(self.settings.options())
    }

    /// Return a configured package's evaluated RDEPEND.
    pub fn rdepend(&self) -> DependencySet<&Dep> {
        self.raw.data.rdepend.evaluate(self.settings.options())
    }

    /// Return a configured package's evaluated LICENSE.
    pub fn license(&self) -> DependencySet<&String> {
        self.raw.data.license.evaluate(self.settings.options())
    }

    /// Return a configured package's evaluated PROPERTIES.
    pub fn properties(&self) -> DependencySet<&String> {
        self.raw.data.properties.evaluate(self.settings.options())
    }

    /// Return a configured package's evaluated RESTRICT.
    pub fn required_use(&self) -> DependencySet<&String> {
        self.raw.data.required_use.evaluate(self.settings.options())
    }

    /// Return a configured package's evaluated RESTRICT.
    pub fn restrict(&self) -> DependencySet<&String> {
        self.raw.data.restrict.evaluate(self.settings.options())
    }

    /// Return a configured package's evaluated SRC_URI.
    pub fn src_uri(&self) -> DependencySet<&Uri> {
        self.raw.data.src_uri.evaluate(self.settings.options())
    }
}

impl Package for Pkg {
    fn eapi(&self) -> &'static Eapi {
        self.raw.eapi()
    }

    fn cpv(&self) -> &Cpv {
        self.raw.cpv()
    }
}

impl RepoPackage for Pkg {
    type Repo = ConfiguredRepo;

    fn repo(&self) -> Self::Repo {
        self.repo.clone()
    }
}

impl EbuildPackage for Pkg {
    // TODO: combine this with profile and config settings
    fn iuse_effective(&self) -> &OrderedSet<String> {
        self.raw.iuse_effective()
    }

    fn slot(&self) -> &str {
        self.raw.slot()
    }
}

impl Restriction<&Pkg> for BaseRestrict {
    fn matches(&self, pkg: &Pkg) -> bool {
        self.matches(&pkg.raw)
    }
}

impl Intersects<Dep> for Pkg {
    fn intersects(&self, dep: &Dep) -> bool {
        bool_not_equal!(self.cpn(), dep.cpn());

        if let Some(val) = dep.slot() {
            bool_not_equal!(self.raw.slot(), val);
        }

        if let Some(val) = dep.subslot() {
            bool_not_equal!(self.raw.subslot(), val);
        }

        // TODO: compare usedeps to iuse_effective

        if let Some(val) = dep.repo() {
            bool_not_equal!(self.repo.name(), val);
        }

        if let Some(val) = dep.version() {
            self.cpv().version().intersects(val)
        } else {
            true
        }
    }
}
