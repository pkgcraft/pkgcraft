use crate::pkg::ebuild::{EbuildPackage, EbuildPkg, EbuildRawPkg};
use crate::pkg::{self, Package, RepoPackage};
use crate::repo::Repository;
use crate::restrict::boolean::*;
use crate::restrict::dep::Restrict as DepRestrict;
use crate::restrict::depset::Restrict as DepSetRestrict;
use crate::restrict::ordered::{make_ordered_restrictions, Restrict as OrderedRestrict};
use crate::restrict::set::OrderedSetRestrict;
use crate::restrict::str::Restrict as StrRestrict;
use crate::restrict::{Restrict as BaseRestrict, Restriction};

use super::xml::Maintainer;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Restrict {
    Ebuild(StrRestrict),
    Description(StrRestrict),
    Slot(StrRestrict),
    Subslot(StrRestrict),
    RawSubslot(Option<StrRestrict>),
    Dependencies(Option<DepSetRestrict<DepRestrict>>),
    Depend(Option<DepSetRestrict<DepRestrict>>),
    Bdepend(Option<DepSetRestrict<DepRestrict>>),
    Idepend(Option<DepSetRestrict<DepRestrict>>),
    Pdepend(Option<DepSetRestrict<DepRestrict>>),
    Rdepend(Option<DepSetRestrict<DepRestrict>>),
    License(Option<DepSetRestrict<StrRestrict>>),
    Properties(Option<DepSetRestrict<StrRestrict>>),
    RequiredUse(Option<DepSetRestrict<StrRestrict>>),
    Restrict(Option<DepSetRestrict<StrRestrict>>),
    SrcUri(Option<DepSetRestrict<StrRestrict>>),
    Homepage(Option<OrderedSetRestrict<String, StrRestrict>>),
    Iuse(Option<OrderedSetRestrict<String, StrRestrict>>),
    Inherit(Option<OrderedSetRestrict<String, StrRestrict>>),
    Inherited(Option<OrderedSetRestrict<String, StrRestrict>>),
    Keywords(Option<OrderedSetRestrict<String, StrRestrict>>),
    LongDescription(Option<StrRestrict>),
    Maintainers(Option<OrderedRestrict<MaintainerRestrict>>),
}

impl From<Restrict> for BaseRestrict {
    fn from(r: Restrict) -> Self {
        Self::Pkg(pkg::Restrict::Ebuild(r))
    }
}

impl Restriction<&EbuildRawPkg> for BaseRestrict {
    fn matches(&self, pkg: &EbuildRawPkg) -> bool {
        crate::restrict::restrict_match! {self, pkg,
            Self::Dep(r) => r.matches(pkg),
            Self::Pkg(r) => r.matches(pkg),
        }
    }
}

impl Restriction<&EbuildRawPkg> for DepRestrict {
    fn matches(&self, pkg: &EbuildRawPkg) -> bool {
        match self {
            Self::Repo(Some(r)) => r.matches(pkg.repo().id()),
            r => r.matches(pkg.cpv()),
        }
    }
}

impl Restriction<&EbuildRawPkg> for pkg::Restrict {
    fn matches(&self, pkg: &EbuildRawPkg) -> bool {
        match self {
            Self::Repo(r) => r.matches(pkg.repo().id()),
            _ => false,
        }
    }
}

impl Restriction<&EbuildPkg> for BaseRestrict {
    fn matches(&self, pkg: &EbuildPkg) -> bool {
        crate::restrict::restrict_match! {self, pkg,
            Self::Dep(r) => r.matches(pkg),
            Self::Pkg(r) => r.matches(pkg),
        }
    }
}

impl Restriction<&EbuildPkg> for DepRestrict {
    fn matches(&self, pkg: &EbuildPkg) -> bool {
        match self {
            Self::Slot(Some(r)) => r.matches(pkg.slot()),
            Self::Subslot(Some(r)) => r.matches(pkg.subslot()),
            Self::Repo(Some(r)) => r.matches(pkg.repo().id()),
            r => r.matches(pkg.cpv()),
        }
    }
}

impl Restriction<&EbuildPkg> for pkg::Restrict {
    fn matches(&self, pkg: &EbuildPkg) -> bool {
        match self {
            Self::Ebuild(r) => r.matches(pkg),
            Self::Eapi(r) => r.matches(pkg.eapi()),
            Self::Repo(r) => r.matches(pkg.repo().id()),
        }
    }
}

impl Restriction<&EbuildPkg> for Restrict {
    fn matches(&self, pkg: &EbuildPkg) -> bool {
        match self {
            Self::Ebuild(r) => match pkg.ebuild() {
                Ok(s) => r.matches(&s),
                Err(_) => false,
            },
            Self::Description(r) => r.matches(pkg.description()),
            Self::Slot(r) => r.matches(pkg.slot()),
            Self::Subslot(r) => r.matches(pkg.subslot()),
            Self::RawSubslot(r) => match (r, pkg.0.data.slot.subslot()) {
                (Some(r), Some(s)) => r.matches(s),
                (None, None) => true,
                _ => false,
            },
            Self::Dependencies(r) => match (r, pkg.dependencies(&[])) {
                (Some(r), val) => r.matches(val),
                (None, val) if val.is_empty() => true,
                _ => false,
            },
            Self::Depend(r) => match (r, pkg.depend()) {
                (Some(r), val) => r.matches(val),
                (None, val) if val.is_empty() => true,
                _ => false,
            },
            Self::Bdepend(r) => match (r, pkg.bdepend()) {
                (Some(r), val) => r.matches(val),
                (None, val) if val.is_empty() => true,
                _ => false,
            },
            Self::Idepend(r) => match (r, pkg.idepend()) {
                (Some(r), val) => r.matches(val),
                (None, val) if val.is_empty() => true,
                _ => false,
            },
            Self::Pdepend(r) => match (r, pkg.pdepend()) {
                (Some(r), val) => r.matches(val),
                (None, val) if val.is_empty() => true,
                _ => false,
            },
            Self::Rdepend(r) => match (r, pkg.rdepend()) {
                (Some(r), val) => r.matches(val),
                (None, val) if val.is_empty() => true,
                _ => false,
            },
            Self::License(r) => match (r, pkg.license()) {
                (Some(r), val) => r.matches(val),
                (None, val) if val.is_empty() => true,
                _ => false,
            },
            Self::Properties(r) => match (r, pkg.properties()) {
                (Some(r), val) => r.matches(val),
                (None, val) if val.is_empty() => true,
                _ => false,
            },
            Self::RequiredUse(r) => match (r, pkg.required_use()) {
                (Some(r), val) => r.matches(val),
                (None, val) if val.is_empty() => true,
                _ => false,
            },
            Self::Restrict(r) => match (r, pkg.restrict()) {
                (Some(r), val) => r.matches(val),
                (None, val) if val.is_empty() => true,
                _ => false,
            },
            Self::SrcUri(r) => match (r, pkg.src_uri()) {
                (Some(r), val) => r.matches(val),
                (None, val) if val.is_empty() => true,
                _ => false,
            },
            Self::Homepage(r) => match (r, pkg.homepage()) {
                (Some(r), val) => r.matches(val),
                (None, val) if val.is_empty() => true,
                _ => false,
            },
            Self::Iuse(r) => match (r, pkg.iuse_effective()) {
                (Some(r), val) => r.matches(val),
                (None, val) if val.is_empty() => true,
                _ => false,
            },
            Self::Inherit(r) => match (r, pkg.inherit()) {
                (Some(r), val) => {
                    let val = val.into_iter().map(|x| x.to_string()).collect();
                    r.matches(&val)
                }
                (None, val) if val.is_empty() => true,
                _ => false,
            },
            Self::Inherited(r) => match (r, pkg.inherited()) {
                (Some(r), val) => {
                    let val = val.into_iter().map(|x| x.to_string()).collect();
                    r.matches(&val)
                }
                (None, val) if val.is_empty() => true,
                _ => false,
            },
            Self::Keywords(r) => match (r, pkg.keywords()) {
                (Some(r), val) => {
                    let val = val.into_iter().map(|x| x.to_string()).collect();
                    r.matches(&val)
                }
                (None, val) if val.is_empty() => true,
                _ => false,
            },
            Self::LongDescription(r) => match (r, pkg.metadata().description()) {
                (Some(r), Some(s)) => r.matches(s),
                (None, None) => true,
                _ => false,
            },
            Self::Maintainers(r) => match r {
                Some(r) => r.matches(pkg.metadata().maintainers()),
                None => pkg.metadata().maintainers().is_empty(),
            },
        }
    }
}

restrict_with_boolean! {MaintainerRestrict,
    Email(StrRestrict),
    Name(Option<StrRestrict>),
    Description(Option<StrRestrict>),
    Type(StrRestrict),
    Proxied(StrRestrict),
}

impl MaintainerRestrict {
    restrict_impl_boolean! {Self}
}

impl Restriction<&Maintainer> for MaintainerRestrict {
    fn matches(&self, m: &Maintainer) -> bool {
        restrict_match_boolean! {self, m,
            Self::Email(r) => r.matches(m.email()),
            Self::Name(r) => match (r, m.name()) {
                (Some(r), Some(s)) => r.matches(s),
                (None, None) => true,
                _ => false,
            },
            Self::Description(r) => match (r, m.description()) {
                (Some(r), Some(s)) => r.matches(s),
                (None, None) => true,
                _ => false,
            },
            Self::Type(r) => r.matches(m.maint_type().as_ref()),
            Self::Proxied(r) => r.matches(m.proxied().as_ref()),
        }
    }
}

impl From<OrderedRestrict<MaintainerRestrict>> for BaseRestrict {
    fn from(r: OrderedRestrict<MaintainerRestrict>) -> Self {
        Restrict::Maintainers(Some(r)).into()
    }
}

make_ordered_restrictions!((&[Maintainer], MaintainerRestrict));

#[cfg(test)]
mod tests {
    use std::fs;

    use itertools::Itertools;

    use crate::config::Config;
    use crate::repo::PkgRepository;
    use crate::test::assert_ordered_eq;

    use super::*;

    #[test]
    fn ebuild() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test", 0, None).unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        // single
        let data = indoc::indoc! {r#"
            EAPI=8
            DESCRIPTION="testing ebuild restrict"
            SLOT=0
        "#};
        temp.create_ebuild_from_str("cat/pkg-1", data).unwrap();
        let data = indoc::indoc! {r#"
            EAPI=8
            DESCRIPTION="testing ebuild restrict"
            SLOT=0
            VAR="a b c"
        "#};
        temp.create_ebuild_from_str("cat/pkg-2", data).unwrap();
        let pkg = repo.get_pkg("cat/pkg-2").unwrap();

        // verify pkg restrictions
        let r = Restrict::Ebuild(StrRestrict::equal("no match"));
        assert!(!r.matches(&pkg));
        let r = Restrict::Ebuild(StrRestrict::regex("VAR=").unwrap());
        assert!(r.matches(&pkg));

        // verify repo restrictions
        let pkgs: Vec<_> = repo.iter_restrict(r).try_collect().unwrap();
        assert_ordered_eq!(pkgs.iter().map(|p| p.cpv().to_string()), ["cat/pkg-2"]);

        let r = Restrict::Ebuild(StrRestrict::regex("SLOT=").unwrap());
        let pkgs: Vec<_> = repo.iter_restrict(r).try_collect().unwrap();
        assert_ordered_eq!(pkgs.iter().map(|p| p.cpv().to_string()), ["cat/pkg-1", "cat/pkg-2"],);
    }

    #[test]
    fn description() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test", 0, None).unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        temp.create_ebuild("cat/pkg-1", &["DESCRIPTION=desc1"])
            .unwrap();
        temp.create_ebuild("cat/pkg-2", &["DESCRIPTION=desc2"])
            .unwrap();
        config.finalize().unwrap();

        let pkg = repo.get_pkg("cat/pkg-2").unwrap();

        // verify pkg restrictions
        let r = Restrict::Description(StrRestrict::equal("no match"));
        assert!(!r.matches(&pkg));
        let r = Restrict::Description(StrRestrict::equal("desc2"));
        assert!(r.matches(&pkg));

        // verify repo restrictions
        let pkgs: Vec<_> = repo.iter_restrict(r).try_collect().unwrap();
        assert_ordered_eq!(pkgs.iter().map(|p| p.cpv().to_string()), ["cat/pkg-2"]);

        let r = Restrict::Description(StrRestrict::regex("desc").unwrap());
        let pkgs: Vec<_> = repo.iter_restrict(r).try_collect().unwrap();
        assert_ordered_eq!(pkgs.iter().map(|p| p.cpv().to_string()), ["cat/pkg-1", "cat/pkg-2"],);
    }

    #[test]
    fn slot() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test", 0, None).unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        temp.create_ebuild("cat/pkg-0", &["SLOT=0"]).unwrap();
        temp.create_ebuild("cat/pkg-1", &["SLOT=1/2"]).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();

        // verify pkg restrictions
        let r = Restrict::Slot(StrRestrict::equal("2"));
        assert!(!r.matches(&pkg));
        let r = Restrict::Slot(StrRestrict::equal("1"));
        assert!(r.matches(&pkg));

        // verify repo restrictions
        let pkgs: Vec<_> = repo.iter_restrict(r).try_collect().unwrap();
        assert_ordered_eq!(pkgs.iter().map(|p| p.cpv().to_string()), ["cat/pkg-1"]);

        let r = Restrict::Slot(StrRestrict::regex("0|1").unwrap());
        let pkgs: Vec<_> = repo.iter_restrict(r).try_collect().unwrap();
        assert_ordered_eq!(pkgs.iter().map(|p| p.cpv().to_string()), ["cat/pkg-0", "cat/pkg-1"],);
    }

    #[test]
    fn subslot() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test", 0, None).unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        temp.create_ebuild("cat/pkg-0", &["SLOT=0"]).unwrap();
        temp.create_ebuild("cat/pkg-1", &["SLOT=1/2"]).unwrap();
        config.finalize().unwrap();

        // no explicit subslot
        let pkg = repo.get_pkg("cat/pkg-0").unwrap();
        let r = Restrict::RawSubslot(None);
        assert!(r.matches(&pkg));

        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        assert!(!r.matches(&pkg));

        // verify pkg restrictions
        let r = Restrict::Subslot(StrRestrict::equal("1"));
        assert!(!r.matches(&pkg));
        let r = Restrict::Subslot(StrRestrict::equal("2"));
        assert!(r.matches(&pkg));

        // verify repo restrictions
        let pkgs: Vec<_> = repo.iter_restrict(r).try_collect().unwrap();
        assert_ordered_eq!(pkgs.iter().map(|p| p.cpv().to_string()), ["cat/pkg-1"]);

        let r = Restrict::Subslot(StrRestrict::regex("0|2").unwrap());
        let pkgs: Vec<_> = repo.iter_restrict(r).try_collect().unwrap();
        assert_ordered_eq!(pkgs.iter().map(|p| p.cpv().to_string()), ["cat/pkg-0", "cat/pkg-1"],);
    }

    #[test]
    fn long_description() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test", 0, None).unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();

        temp.create_ebuild("cat/pkg-a-1", &[]).unwrap();

        let path = temp.create_ebuild("cat/pkg-b-1", &[]).unwrap();
        let data = indoc::indoc! {r#"
            <?xml version="1.0" encoding="UTF-8"?>
            <!DOCTYPE pkgmetadata SYSTEM "https://www.gentoo.org/dtd/metadata.dtd">
            <pkgmetadata>
                <longdescription>
                    desc1
                </longdescription>
            </pkgmetadata>
        "#};
        fs::write(path.parent().unwrap().join("metadata.xml"), data).unwrap();

        let path = temp.create_ebuild("cat/pkg-c-1", &[]).unwrap();
        let data = indoc::indoc! {r#"
            <?xml version="1.0" encoding="UTF-8"?>
            <!DOCTYPE pkgmetadata SYSTEM "https://www.gentoo.org/dtd/metadata.dtd">
            <pkgmetadata>
                <longdescription>
                    desc2
                </longdescription>
            </pkgmetadata>
        "#};
        fs::write(path.parent().unwrap().join("metadata.xml"), data).unwrap();

        config.finalize().unwrap();

        // pkg lacking long description
        let pkg = repo.get_pkg("cat/pkg-a-1").unwrap();
        let r = Restrict::LongDescription(None);
        assert!(r.matches(&pkg));

        // pkg with long description
        let pkg = repo.get_pkg("cat/pkg-b-1").unwrap();
        let r = Restrict::LongDescription(Some(StrRestrict::regex(".").unwrap()));
        assert!(r.matches(&pkg));

        // single repo match
        let r = Restrict::LongDescription(Some(StrRestrict::regex("desc1").unwrap()));
        let pkgs: Vec<_> = repo.iter_restrict(r).try_collect().unwrap();
        assert_ordered_eq!(pkgs.iter().map(|p| p.cpv().to_string()), ["cat/pkg-b-1"]);

        // multiple repo matches
        let r = Restrict::LongDescription(Some(StrRestrict::regex("desc").unwrap()));
        let pkgs: Vec<_> = repo.iter_restrict(r).try_collect().unwrap();
        assert_ordered_eq!(
            pkgs.iter().map(|p| p.cpv().to_string()),
            ["cat/pkg-b-1", "cat/pkg-c-1"],
        );
    }

    #[test]
    fn maintainers() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test", 0, None).unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        // none
        temp.create_ebuild("noxml/pkg-1", &[]).unwrap();

        // single
        temp.create_ebuild("cat/pkg-a-1", &[]).unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-a-1").unwrap();
        let data = indoc::indoc! {r#"
            <?xml version="1.0" encoding="UTF-8"?>
            <!DOCTYPE pkgmetadata SYSTEM "https://www.gentoo.org/dtd/metadata.dtd">
            <pkgmetadata>
                <maintainer type="project">
                    <email>a.project@email.com</email>
                    <name>A Project</name>
                </maintainer>
            </pkgmetadata>
        "#};
        fs::write(raw_pkg.path().parent().unwrap().join("metadata.xml"), data).unwrap();

        // multiple
        temp.create_ebuild("cat/pkg-b-1", &[]).unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-b-1").unwrap();
        let data = indoc::indoc! {r#"
            <?xml version="1.0" encoding="UTF-8"?>
            <!DOCTYPE pkgmetadata SYSTEM "https://www.gentoo.org/dtd/metadata.dtd">
            <pkgmetadata>
                <maintainer type="person" proxied="yes">
                    <email>a.person@email.com</email>
                    <name>A Person</name>
                </maintainer>
                <maintainer type="person" proxied="proxy">
                    <email>b.person@email.com</email>
                    <name>B Person</name>
                </maintainer>
            </pkgmetadata>
        "#};
        fs::write(raw_pkg.path().parent().unwrap().join("metadata.xml"), data).unwrap();

        // pkgs with no maintainers
        let r: BaseRestrict = Restrict::Maintainers(None).into();
        let pkgs: Vec<_> = repo.iter_restrict(&r).try_collect().unwrap();
        assert_ordered_eq!(pkgs.iter().map(|p| p.cpv().to_string()), ["noxml/pkg-1"]);

        // pkgs with maintainers
        let pkgs: Vec<_> = repo.iter_restrict(!r).try_collect().unwrap();
        assert_ordered_eq!(
            pkgs.iter().map(|p| p.cpv().to_string()),
            ["cat/pkg-a-1", "cat/pkg-b-1"],
        );
    }
}
