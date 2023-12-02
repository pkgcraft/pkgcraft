use std::sync::OnceLock;

use crate::config::Settings;
use crate::dep::{Cpv, Dep, DepSet, Evaluate, Uri};
use crate::eapi::Eapi;
use crate::pkg::{make_pkg_traits, Package, RepoPackage};
use crate::repo::ebuild::configured::Repo;
use crate::restrict::{Restrict as BaseRestrict, Restriction};
use crate::shell::metadata::Key;
use crate::types::OrderedSet;

use super::EbuildPackage;

#[derive(Debug)]
pub struct Pkg<'a> {
    repo: &'a Repo,
    settings: &'a Settings,
    raw: super::Pkg<'a>,
    bdepend: OnceLock<DepSet<&'a String, &'a Dep>>,
    depend: OnceLock<DepSet<&'a String, &'a Dep>>,
    idepend: OnceLock<DepSet<&'a String, &'a Dep>>,
    pdepend: OnceLock<DepSet<&'a String, &'a Dep>>,
    rdepend: OnceLock<DepSet<&'a String, &'a Dep>>,
    license: OnceLock<DepSet<&'a String, &'a String>>,
    properties: OnceLock<DepSet<&'a String, &'a String>>,
    required_use: OnceLock<DepSet<&'a String, &'a String>>,
    restrict: OnceLock<DepSet<&'a String, &'a String>>,
    uris: OnceLock<DepSet<&'a String, &'a Uri>>,
    iuse_effective: OnceLock<OrderedSet<String>>,
}

impl<'a> From<&'a Pkg<'a>> for &'a super::Pkg<'a> {
    fn from(pkg: &'a Pkg<'a>) -> Self {
        &pkg.raw
    }
}

make_pkg_traits!(Pkg<'_>);

impl<'a> Pkg<'a> {
    pub(crate) fn new(repo: &'a Repo, settings: &'a Settings, raw: super::Pkg<'a>) -> Self {
        Self {
            repo,
            settings,
            raw,
            bdepend: OnceLock::new(),
            depend: OnceLock::new(),
            idepend: OnceLock::new(),
            pdepend: OnceLock::new(),
            rdepend: OnceLock::new(),
            license: OnceLock::new(),
            properties: OnceLock::new(),
            required_use: OnceLock::new(),
            restrict: OnceLock::new(),
            uris: OnceLock::new(),
            iuse_effective: OnceLock::new(),
        }
    }

    /// Return a package's evaluated dependencies for a given iterable of descriptors.
    pub fn dependencies(&'a self, keys: &[Key]) -> DepSet<&'a String, &'a Dep> {
        self.raw
            .dependencies(keys)
            .evaluate(self.settings.options())
    }

    /// Return a configured package's evaluated BDEPEND.
    pub fn bdepend(&'a self) -> &DepSet<&'a String, &'a Dep> {
        self.bdepend
            .get_or_init(|| self.raw.bdepend().evaluate(self.settings.options()))
    }

    /// Return a configured package's evaluated DEPEND.
    pub fn depend(&'a self) -> &DepSet<&'a String, &'a Dep> {
        self.depend
            .get_or_init(|| self.raw.depend().evaluate(self.settings.options()))
    }

    /// Return a configured package's evaluated IDEPEND.
    pub fn idepend(&'a self) -> &DepSet<&'a String, &'a Dep> {
        self.idepend
            .get_or_init(|| self.raw.idepend().evaluate(self.settings.options()))
    }

    /// Return a configured package's evaluated PDEPEND.
    pub fn pdepend(&'a self) -> &DepSet<&'a String, &'a Dep> {
        self.pdepend
            .get_or_init(|| self.raw.pdepend().evaluate(self.settings.options()))
    }

    /// Return a configured package's evaluated RDEPEND.
    pub fn rdepend(&'a self) -> &DepSet<&'a String, &'a Dep> {
        self.rdepend
            .get_or_init(|| self.raw.rdepend().evaluate(self.settings.options()))
    }

    /// Return a configured package's evaluated LICENSE.
    pub fn license(&'a self) -> &DepSet<&'a String, &'a String> {
        self.license
            .get_or_init(|| self.raw.license().evaluate(self.settings.options()))
    }

    /// Return a configured package's evaluated PROPERTIES.
    pub fn properties(&'a self) -> &DepSet<&'a String, &'a String> {
        self.properties
            .get_or_init(|| self.raw.properties().evaluate(self.settings.options()))
    }

    /// Return a configured package's evaluated RESTRICT.
    pub fn required_use(&'a self) -> &DepSet<&'a String, &'a String> {
        self.required_use
            .get_or_init(|| self.raw.required_use().evaluate(self.settings.options()))
    }

    /// Return a configured package's evaluated RESTRICT.
    pub fn restrict(&'a self) -> &DepSet<&'a String, &'a String> {
        self.restrict
            .get_or_init(|| self.raw.restrict().evaluate(self.settings.options()))
    }

    /// Return a configured package's evaluated SRC_URI.
    pub fn src_uri(&'a self) -> &DepSet<&'a String, &'a Uri> {
        self.uris
            .get_or_init(|| self.raw.src_uri().evaluate(self.settings.options()))
    }
}

impl<'a> Package for Pkg<'a> {
    fn eapi(&self) -> &'static Eapi {
        self.raw.eapi()
    }

    fn cpv(&self) -> &Cpv {
        self.raw.cpv()
    }
}

impl<'a> RepoPackage for Pkg<'a> {
    type Repo = &'a Repo;

    fn repo(&self) -> Self::Repo {
        self.repo
    }
}

impl<'a> EbuildPackage for Pkg<'a> {
    // TODO: combine this with profile and config settings
    fn iuse_effective(&self) -> &OrderedSet<String> {
        self.iuse_effective
            .get_or_init(|| self.raw.iuse_effective().clone())
    }

    fn slot(&self) -> &str {
        self.raw.slot()
    }
}

impl<'a, 'b> Restriction<&'a Pkg<'b>> for BaseRestrict {
    fn matches(&self, pkg: &'a Pkg<'b>) -> bool {
        self.matches(&pkg.raw)
    }
}
