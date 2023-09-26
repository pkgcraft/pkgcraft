use std::sync::OnceLock;

use crate::config::Settings;
use crate::dep::{Cpv, Dep, DepSet, Evaluate, Uri};
use crate::eapi::Eapi;
use crate::pkg::{make_pkg_traits, Package};
use crate::repo::ebuild::configured::Repo;
use crate::restrict::{Restrict as BaseRestrict, Restriction};

#[derive(Debug)]
pub struct Pkg<'a> {
    repo: &'a Repo,
    settings: &'a Settings,
    raw: super::Pkg<'a>,
    bdepend: OnceLock<Option<DepSet<&'a String, &'a Dep>>>,
    depend: OnceLock<Option<DepSet<&'a String, &'a Dep>>>,
    idepend: OnceLock<Option<DepSet<&'a String, &'a Dep>>>,
    pdepend: OnceLock<Option<DepSet<&'a String, &'a Dep>>>,
    rdepend: OnceLock<Option<DepSet<&'a String, &'a Dep>>>,
    license: OnceLock<Option<DepSet<&'a String, &'a String>>>,
    properties: OnceLock<Option<DepSet<&'a String, &'a String>>>,
    required_use: OnceLock<Option<DepSet<&'a String, &'a String>>>,
    restrict: OnceLock<Option<DepSet<&'a String, &'a String>>>,
    uris: OnceLock<Option<DepSet<&'a String, &'a Uri>>>,
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
        }
    }

    /// Return a configured package's evaluated BDEPEND.
    pub fn bdepend(&'a self) -> Option<&DepSet<&'a String, &'a Dep>> {
        self.bdepend
            .get_or_init(|| {
                self.raw
                    .bdepend()
                    .map(|d| d.evaluate(self.settings.options()))
            })
            .as_ref()
    }

    /// Return a configured package's evaluated DEPEND.
    pub fn depend(&'a self) -> Option<&DepSet<&'a String, &'a Dep>> {
        self.depend
            .get_or_init(|| {
                self.raw
                    .depend()
                    .map(|d| d.evaluate(self.settings.options()))
            })
            .as_ref()
    }

    /// Return a configured package's evaluated IDEPEND.
    pub fn idepend(&'a self) -> Option<&DepSet<&'a String, &'a Dep>> {
        self.idepend
            .get_or_init(|| {
                self.raw
                    .idepend()
                    .map(|d| d.evaluate(self.settings.options()))
            })
            .as_ref()
    }

    /// Return a configured package's evaluated PDEPEND.
    pub fn pdepend(&'a self) -> Option<&DepSet<&'a String, &'a Dep>> {
        self.pdepend
            .get_or_init(|| {
                self.raw
                    .pdepend()
                    .map(|d| d.evaluate(self.settings.options()))
            })
            .as_ref()
    }

    /// Return a configured package's evaluated RDEPEND.
    pub fn rdepend(&'a self) -> Option<&DepSet<&'a String, &'a Dep>> {
        self.rdepend
            .get_or_init(|| {
                self.raw
                    .rdepend()
                    .map(|d| d.evaluate(self.settings.options()))
            })
            .as_ref()
    }

    /// Return a configured package's evaluated LICENSE.
    pub fn license(&'a self) -> Option<&DepSet<&'a String, &'a String>> {
        self.license
            .get_or_init(|| {
                self.raw
                    .license()
                    .map(|d| d.evaluate(self.settings.options()))
            })
            .as_ref()
    }

    /// Return a configured package's evaluated PROPERTIES.
    pub fn properties(&'a self) -> Option<&DepSet<&'a String, &'a String>> {
        self.properties
            .get_or_init(|| {
                self.raw
                    .properties()
                    .map(|d| d.evaluate(self.settings.options()))
            })
            .as_ref()
    }

    /// Return a configured package's evaluated RESTRICT.
    pub fn required_use(&'a self) -> Option<&DepSet<&'a String, &'a String>> {
        self.required_use
            .get_or_init(|| {
                self.raw
                    .required_use()
                    .map(|d| d.evaluate(self.settings.options()))
            })
            .as_ref()
    }

    /// Return a configured package's evaluated RESTRICT.
    pub fn restrict(&'a self) -> Option<&DepSet<&'a String, &'a String>> {
        self.restrict
            .get_or_init(|| {
                self.raw
                    .restrict()
                    .map(|d| d.evaluate(self.settings.options()))
            })
            .as_ref()
    }

    /// Return a configured package's evaluated SRC_URI.
    pub fn src_uri(&'a self) -> Option<&DepSet<&'a String, &'a Uri>> {
        self.uris
            .get_or_init(|| {
                self.raw
                    .src_uri()
                    .map(|d| d.evaluate(self.settings.options()))
            })
            .as_ref()
    }
}

impl<'a> Package for Pkg<'a> {
    type Repo = &'a Repo;

    fn cpv(&self) -> &Cpv {
        self.raw.cpv()
    }

    fn eapi(&self) -> &'static Eapi {
        self.raw.eapi()
    }

    fn repo(&self) -> Self::Repo {
        self.repo
    }
}

impl<'a, 'b> Restriction<&'a Pkg<'b>> for BaseRestrict {
    fn matches(&self, pkg: &'a Pkg<'b>) -> bool {
        self.matches(&pkg.raw)
    }
}
