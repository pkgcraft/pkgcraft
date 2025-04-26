use std::fmt;
use std::sync::Arc;

use crate::config::Settings;
use crate::dep::{Cpv, Dep, DependencySet, Evaluate, Uri};
use crate::eapi::Eapi;
use crate::macros::bool_not_equal;
use crate::pkg::{Package, RepoPackage, make_pkg_traits};
use crate::repo::Repository;
use crate::repo::ebuild::configured::ConfiguredRepo;
use crate::restrict::{Restrict as BaseRestrict, Restriction};
use crate::traits::Intersects;
use crate::types::OrderedSet;

use super::EbuildPkg;
use super::metadata::Key;

#[derive(Clone)]
pub struct EbuildConfiguredPkg {
    repo: ConfiguredRepo,
    settings: Arc<Settings>,
    raw: EbuildPkg,
}

impl<'a> From<&'a EbuildConfiguredPkg> for &'a EbuildPkg {
    fn from(pkg: &'a EbuildConfiguredPkg) -> Self {
        &pkg.raw
    }
}

make_pkg_traits!(EbuildConfiguredPkg);

impl fmt::Debug for EbuildConfiguredPkg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "EbuildConfiguredPkg {{ {self} }}")
    }
}

impl EbuildConfiguredPkg {
    pub(crate) fn new(repo: ConfiguredRepo, settings: Arc<Settings>, raw: EbuildPkg) -> Self {
        Self { repo, settings, raw }
    }

    /// Return a package's evaluated dependencies for a given iterable of descriptors.
    pub fn dependencies<I>(&self, keys: I) -> DependencySet<&Dep>
    where
        I: IntoIterator<Item = Key>,
    {
        self.raw
            .dependencies(keys)
            .evaluate(self.settings.options())
    }

    /// Return a configured package's evaluated BDEPEND.
    pub fn bdepend(&self) -> DependencySet<&Dep> {
        self.raw.0.meta.bdepend.evaluate(self.settings.options())
    }

    /// Return a configured package's evaluated DEPEND.
    pub fn depend(&self) -> DependencySet<&Dep> {
        self.raw.0.meta.depend.evaluate(self.settings.options())
    }

    /// Return a configured package's evaluated IDEPEND.
    pub fn idepend(&self) -> DependencySet<&Dep> {
        self.raw.0.meta.idepend.evaluate(self.settings.options())
    }

    /// Return a configured package's evaluated PDEPEND.
    pub fn pdepend(&self) -> DependencySet<&Dep> {
        self.raw.0.meta.pdepend.evaluate(self.settings.options())
    }

    /// Return a configured package's evaluated RDEPEND.
    pub fn rdepend(&self) -> DependencySet<&Dep> {
        self.raw.0.meta.rdepend.evaluate(self.settings.options())
    }

    /// Return a configured package's evaluated LICENSE.
    pub fn license(&self) -> DependencySet<&String> {
        self.raw.0.meta.license.evaluate(self.settings.options())
    }

    /// Return a configured package's evaluated PROPERTIES.
    pub fn properties(&self) -> DependencySet<&String> {
        self.raw.0.meta.properties.evaluate(self.settings.options())
    }

    /// Return a configured package's evaluated RESTRICT.
    pub fn required_use(&self) -> DependencySet<&String> {
        self.raw
            .0
            .meta
            .required_use
            .evaluate(self.settings.options())
    }

    /// Return a configured package's evaluated RESTRICT.
    pub fn restrict(&self) -> DependencySet<&String> {
        self.raw.0.meta.restrict.evaluate(self.settings.options())
    }

    /// Return a configured package's evaluated SRC_URI.
    pub fn src_uri(&self) -> DependencySet<&Uri> {
        self.raw.0.meta.src_uri.evaluate(self.settings.options())
    }

    // TODO: combine this with profile and config settings
    pub fn iuse_effective(&self) -> &OrderedSet<String> {
        self.raw.iuse_effective()
    }

    pub fn slot(&self) -> &str {
        self.raw.slot()
    }
}

impl Package for EbuildConfiguredPkg {
    fn eapi(&self) -> &'static Eapi {
        self.raw.eapi()
    }

    fn cpv(&self) -> &Cpv {
        self.raw.cpv()
    }
}

impl RepoPackage for EbuildConfiguredPkg {
    type Repo = ConfiguredRepo;

    fn repo(&self) -> Self::Repo {
        self.repo.clone()
    }
}

impl Restriction<&EbuildConfiguredPkg> for BaseRestrict {
    fn matches(&self, pkg: &EbuildConfiguredPkg) -> bool {
        self.matches(&pkg.raw)
    }
}

impl Intersects<Dep> for EbuildConfiguredPkg {
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
