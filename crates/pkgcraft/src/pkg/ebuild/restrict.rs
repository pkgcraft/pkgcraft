use crate::pkg::{self, Package};
use crate::repo::Repository;
use crate::restrict::boolean::*;
use crate::restrict::dep::Restrict as DepRestrict;
use crate::restrict::depset::Restrict as DepSetRestrict;
use crate::restrict::ordered::{make_ordered_restrictions, Restrict as OrderedRestrict};
use crate::restrict::set::OrderedSetRestrict;
use crate::restrict::str::Restrict as StrRestrict;
use crate::restrict::{Restrict as BaseRestrict, Restriction};

use super::metadata::Maintainer;
use super::Pkg;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Restrict {
    Ebuild(StrRestrict),
    Description(StrRestrict),
    Slot(StrRestrict),
    Subslot(StrRestrict),
    RawSubslot(Option<StrRestrict>),
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
    DefinedPhases(Option<OrderedSetRestrict<String, StrRestrict>>),
    Keywords(Option<OrderedSetRestrict<String, StrRestrict>>),
    Iuse(Option<OrderedSetRestrict<String, StrRestrict>>),
    Inherit(Option<OrderedSetRestrict<String, StrRestrict>>),
    Inherited(Option<OrderedSetRestrict<String, StrRestrict>>),
    LongDescription(Option<StrRestrict>),
    Maintainers(Option<OrderedRestrict<MaintainerRestrict>>),
}

impl From<Restrict> for BaseRestrict {
    fn from(r: Restrict) -> Self {
        Self::Pkg(pkg::Restrict::Ebuild(r))
    }
}

impl<'a> Restriction<&'a Pkg<'a>> for BaseRestrict {
    fn matches(&self, pkg: &'a Pkg<'a>) -> bool {
        crate::restrict::restrict_match! {self, pkg,
            Self::Dep(r) => r.matches(pkg),
            Self::Pkg(r) => r.matches(pkg),
        }
    }
}

impl<'a> Restriction<&'a Pkg<'a>> for DepRestrict {
    fn matches(&self, pkg: &'a Pkg<'a>) -> bool {
        use DepRestrict::*;
        match self {
            Slot(Some(r)) => r.matches(pkg.slot()),
            Subslot(Some(r)) => r.matches(pkg.subslot()),
            Repo(Some(r)) => r.matches(pkg.repo().id()),
            r => r.matches(pkg.cpv()),
        }
    }
}

impl<'a> Restriction<&'a Pkg<'a>> for pkg::Restrict {
    fn matches(&self, pkg: &'a Pkg<'a>) -> bool {
        use pkg::Restrict::*;
        match self {
            Ebuild(r) => r.matches(pkg),
            Eapi(r) => r.matches(pkg.eapi().as_str()),
            Repo(r) => r.matches(pkg.repo().id()),
        }
    }
}

impl<'a> Restriction<&'a Pkg<'a>> for Restrict {
    fn matches(&self, pkg: &'a Pkg<'a>) -> bool {
        use self::Restrict::*;
        match self {
            Ebuild(r) => match pkg.ebuild() {
                Ok(s) => r.matches(&s),
                Err(_) => false,
            },
            Description(r) => r.matches(pkg.description()),
            Slot(r) => r.matches(pkg.slot()),
            Subslot(r) => r.matches(pkg.subslot()),
            RawSubslot(r) => match (r, pkg.meta.subslot()) {
                (Some(r), Some(s)) => r.matches(s),
                (None, None) => true,
                _ => false,
            },
            Depend(r) => match (r, pkg.depend()) {
                (Some(r), Some(val)) => r.matches(val),
                (None, None) => true,
                _ => false,
            },
            Bdepend(r) => match (r, pkg.bdepend()) {
                (Some(r), Some(val)) => r.matches(val),
                (None, None) => true,
                _ => false,
            },
            Idepend(r) => match (r, pkg.idepend()) {
                (Some(r), Some(val)) => r.matches(val),
                (None, None) => true,
                _ => false,
            },
            Pdepend(r) => match (r, pkg.pdepend()) {
                (Some(r), Some(val)) => r.matches(val),
                (None, None) => true,
                _ => false,
            },
            Rdepend(r) => match (r, pkg.rdepend()) {
                (Some(r), Some(val)) => r.matches(val),
                (None, None) => true,
                _ => false,
            },
            License(r) => match (r, pkg.license()) {
                (Some(r), Some(val)) => r.matches(val),
                (None, None) => true,
                _ => false,
            },
            Properties(r) => match (r, pkg.properties()) {
                (Some(r), Some(val)) => r.matches(val),
                (None, None) => true,
                _ => false,
            },
            RequiredUse(r) => match (r, pkg.required_use()) {
                (Some(r), Some(val)) => r.matches(val),
                (None, None) => true,
                _ => false,
            },
            Restrict(r) => match (r, pkg.restrict()) {
                (Some(r), Some(val)) => r.matches(val),
                (None, None) => true,
                _ => false,
            },
            SrcUri(r) => match (r, pkg.src_uri()) {
                (Some(r), Some(val)) => r.matches(val),
                (None, None) => true,
                _ => false,
            },
            Homepage(r) => match (r, pkg.homepage()) {
                (Some(r), strings) => r.matches(strings),
                (None, strings) => strings.is_empty(),
            },
            DefinedPhases(r) => match (r, pkg.defined_phases()) {
                (Some(r), strings) => r.matches(strings),
                (None, strings) => strings.is_empty(),
            },
            Keywords(r) => match (r, pkg.keywords()) {
                (Some(r), strings) => r.matches(strings),
                (None, strings) => strings.is_empty(),
            },
            Iuse(r) => match (r, pkg.iuse()) {
                (Some(r), strings) => r.matches(strings),
                (None, strings) => strings.is_empty(),
            },
            Inherit(r) => match (r, pkg.inherit()) {
                (Some(r), strings) => r.matches(strings),
                (None, strings) => strings.is_empty(),
            },
            Inherited(r) => match (r, pkg.inherited()) {
                (Some(r), strings) => r.matches(strings),
                (None, strings) => strings.is_empty(),
            },
            LongDescription(r) => match (r, pkg.xml().long_description()) {
                (Some(r), Some(s)) => r.matches(s),
                (None, None) => true,
                _ => false,
            },
            Maintainers(r) => match r {
                Some(r) => r.matches(pkg.xml().maintainers()),
                None => pkg.xml().maintainers().is_empty(),
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

    use crate::config::Config;
    use crate::repo::PkgRepository;

    use super::*;

    #[test]
    fn test_ebuild() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();

        // single
        let data = indoc::indoc! {r#"
            DESCRIPTION="testing ebuild restrict"
            SLOT=0
        "#};
        t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let data = indoc::indoc! {r#"
            DESCRIPTION="testing ebuild restrict"
            SLOT=0
            VAR="a b c"
        "#};
        let (path, cpv) = t.create_ebuild_raw("cat/pkg-2", data).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();

        // verify pkg restrictions
        let r = Restrict::Ebuild(StrRestrict::equal("no match"));
        assert!(!r.matches(&pkg));
        let r = Restrict::Ebuild(StrRestrict::regex("VAR=").unwrap());
        assert!(r.matches(&pkg));

        // verify repo restrictions
        let iter = repo.iter_restrict(r);
        let cpvs: Vec<_> = iter.map(|p| p.cpv().to_string()).collect();
        assert_eq!(cpvs, ["cat/pkg-2"]);

        let r = Restrict::Ebuild(StrRestrict::regex("SLOT=").unwrap());
        let iter = repo.iter_restrict(r);
        let cpvs: Vec<_> = iter.map(|p| p.cpv().to_string()).collect();
        assert_eq!(cpvs, ["cat/pkg-1", "cat/pkg-2"]);
    }

    #[test]
    fn test_description() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();

        t.create_ebuild("cat/pkg-1", &["DESCRIPTION=desc1"])
            .unwrap();
        let (path, cpv) = t
            .create_ebuild("cat/pkg-2", &["DESCRIPTION=desc2"])
            .unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();

        // verify pkg restrictions
        let r = Restrict::Description(StrRestrict::equal("no match"));
        assert!(!r.matches(&pkg));
        let r = Restrict::Description(StrRestrict::equal("desc2"));
        assert!(r.matches(&pkg));

        // verify repo restrictions
        let iter = repo.iter_restrict(r);
        let cpvs: Vec<_> = iter.map(|p| p.cpv().to_string()).collect();
        assert_eq!(cpvs, ["cat/pkg-2"]);

        let r = Restrict::Description(StrRestrict::regex("desc").unwrap());
        let iter = repo.iter_restrict(r);
        let cpvs: Vec<_> = iter.map(|p| p.cpv().to_string()).collect();
        assert_eq!(cpvs, ["cat/pkg-1", "cat/pkg-2"]);
    }

    #[test]
    fn test_slot() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();

        t.create_ebuild("cat/pkg-0", &["SLOT=0"]).unwrap();
        let (path, cpv) = t.create_ebuild("cat/pkg-1", &["SLOT=1/2"]).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();

        // verify pkg restrictions
        let r = Restrict::Slot(StrRestrict::equal("2"));
        assert!(!r.matches(&pkg));
        let r = Restrict::Slot(StrRestrict::equal("1"));
        assert!(r.matches(&pkg));

        // verify repo restrictions
        let iter = repo.iter_restrict(r);
        let cpvs: Vec<_> = iter.map(|p| p.cpv().to_string()).collect();
        assert_eq!(cpvs, ["cat/pkg-1"]);

        let r = Restrict::Slot(StrRestrict::regex("0|1").unwrap());
        let iter = repo.iter_restrict(r);
        let cpvs: Vec<_> = iter.map(|p| p.cpv().to_string()).collect();
        assert_eq!(cpvs, ["cat/pkg-0", "cat/pkg-1"]);
    }

    #[test]
    fn test_subslot() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();

        // no explicit subslot
        let (path, cpv) = t.create_ebuild("cat/pkg-0", &["SLOT=0"]).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        let r = Restrict::RawSubslot(None);
        assert!(r.matches(&pkg));

        let (path, cpv) = t.create_ebuild("cat/pkg-1", &["SLOT=1/2"]).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert!(!r.matches(&pkg));

        // verify pkg restrictions
        let r = Restrict::Subslot(StrRestrict::equal("1"));
        assert!(!r.matches(&pkg));
        let r = Restrict::Subslot(StrRestrict::equal("2"));
        assert!(r.matches(&pkg));

        // verify repo restrictions
        let iter = repo.iter_restrict(r);
        let cpvs: Vec<_> = iter.map(|p| p.cpv().to_string()).collect();
        assert_eq!(cpvs, ["cat/pkg-1"]);

        let r = Restrict::Subslot(StrRestrict::regex("0|2").unwrap());
        let iter = repo.iter_restrict(r);
        let cpvs: Vec<_> = iter.map(|p| p.cpv().to_string()).collect();
        assert_eq!(cpvs, ["cat/pkg-0", "cat/pkg-1"]);
    }

    #[test]
    fn test_long_description() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();

        let (path, cpv) = t.create_ebuild("cat/pkg-a-1", &[]).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();

        // pkg lacking long description
        let r = Restrict::LongDescription(None);
        assert!(r.matches(&pkg));

        let (path, cpv) = t.create_ebuild("cat/pkg-b-1", &[]).unwrap();
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
        let pkg = Pkg::new(path, cpv, &repo).unwrap();

        // pkg with long description
        let r = Restrict::LongDescription(Some(StrRestrict::regex(".").unwrap()));
        assert!(r.matches(&pkg));

        // single repo match
        let iter = repo.iter_restrict(r);
        let cpvs: Vec<_> = iter.map(|p| p.cpv().to_string()).collect();
        assert_eq!(cpvs, ["cat/pkg-b-1"]);

        let (path, _) = t.create_ebuild("cat/pkg-c-1", &[]).unwrap();
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

        // multiple repo matches
        let r = Restrict::LongDescription(Some(StrRestrict::regex("desc").unwrap()));
        let iter = repo.iter_restrict(r);
        let cpvs: Vec<_> = iter.map(|p| p.cpv().to_string()).collect();
        assert_eq!(cpvs, ["cat/pkg-b-1", "cat/pkg-c-1"]);
    }

    #[test]
    fn test_maintainers() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();

        // none
        t.create_ebuild("noxml/pkg-1", &[]).unwrap();

        // single
        let (path, _) = t.create_ebuild("cat/pkg-a-1", &[]).unwrap();
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
        fs::write(path.parent().unwrap().join("metadata.xml"), data).unwrap();

        // multiple
        let (path, _) = t.create_ebuild("cat/pkg-b-1", &[]).unwrap();
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
        fs::write(path.parent().unwrap().join("metadata.xml"), data).unwrap();

        // pkgs with no maintainers
        let r: BaseRestrict = Restrict::Maintainers(None).into();
        let iter = repo.iter_restrict(r.clone());
        let cpvs: Vec<_> = iter.map(|p| p.cpv().to_string()).collect();
        assert_eq!(cpvs, ["noxml/pkg-1"]);

        // pkgs with maintainers
        let iter = repo.iter_restrict(!r);
        let cpvs: Vec<_> = iter.map(|p| p.cpv().to_string()).collect();
        assert_eq!(cpvs, ["cat/pkg-a-1", "cat/pkg-b-1"]);
    }
}
