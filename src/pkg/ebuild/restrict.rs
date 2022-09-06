use std::{fmt, ptr};

use crate::metadata::ebuild::{MaintainerRestrict, UpstreamRestrict};
use crate::pkg::{self, Package};
use crate::repo::Repository;
use crate::restrict::{self, *};

use super::Pkg;

#[derive(Clone)]
pub enum Restrict {
    Custom(fn(&Pkg) -> bool),
    Ebuild(Str),
    Category(Str),
    Description(Str),
    Slot(Str),
    Subslot(Str),
    RawSubslot(Option<Str>),
    Homepage(Option<SliceRestrict<Str>>),
    DefinedPhases(Option<HashSetRestrict<String>>),
    Keywords(Option<IndexSetRestrict<String, Str>>),
    Iuse(Option<IndexSetRestrict<String, Str>>),
    Inherit(Option<IndexSetRestrict<String, Str>>),
    Inherited(Option<IndexSetRestrict<String, Str>>),
    LongDescription(Option<Str>),
    Maintainers(Option<SliceRestrict<MaintainerRestrict>>),
    Upstreams(Option<SliceRestrict<UpstreamRestrict>>),
}

impl fmt::Debug for Restrict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Custom(func) => write!(f, "Custom(func: {:?})", ptr::addr_of!(func)),
            r => write!(f, "{r:?}"),
        }
    }
}

impl From<Restrict> for restrict::Restrict {
    fn from(r: Restrict) -> Self {
        Self::Pkg(pkg::Restrict::Ebuild(r))
    }
}

impl<'a> Restriction<&'a Pkg<'a>> for restrict::Restrict {
    fn matches(&self, pkg: &'a Pkg<'a>) -> bool {
        use crate::atom::Restrict as AtomRestrict;
        use crate::pkg::Restrict as PkgRestrict;
        restrict::restrict_match! {
            self, pkg,
            Self::Atom(AtomRestrict::Slot(Some(r))) => r.matches(pkg.slot()),
            Self::Atom(AtomRestrict::Subslot(Some(r))) => r.matches(pkg.subslot()),
            Self::Atom(AtomRestrict::Repo(Some(r))) => r.matches(pkg.repo().id()),
            Self::Atom(r) => r.matches(pkg.atom()),
            Self::Pkg(PkgRestrict::Ebuild(r)) => r.matches(pkg),
            Self::Pkg(PkgRestrict::Eapi(r)) => r.matches(pkg.eapi().as_str()),
            Self::Pkg(PkgRestrict::Repo(r)) => r.matches(pkg.repo().id())
        }
    }
}

impl<'a> Restriction<&'a Pkg<'a>> for Restrict {
    fn matches(&self, pkg: &'a Pkg<'a>) -> bool {
        match self {
            Self::Custom(func) => func(pkg),
            Self::Ebuild(r) => match pkg.ebuild() {
                Ok(s) => r.matches(&s),
                Err(_) => false,
            },
            Self::Category(r) => r.matches(pkg.atom().category()),
            Self::Description(r) => r.matches(pkg.description()),
            Self::Slot(r) => r.matches(pkg.slot()),
            Self::Subslot(r) => r.matches(pkg.subslot()),
            Self::RawSubslot(r) => match (r, pkg.meta.subslot()) {
                (Some(r), Some(s)) => r.matches(s),
                (None, None) => true,
                _ => false,
            },
            Self::Homepage(r) => match (r, pkg.homepage()) {
                (Some(r), strings) => r.matches(strings),
                (None, strings) => strings.is_empty(),
            },
            Self::DefinedPhases(r) => match (r, pkg.defined_phases()) {
                (Some(r), strings) => r.matches(strings),
                (None, strings) => strings.is_empty(),
            },
            Self::Keywords(r) => match (r, pkg.keywords()) {
                (Some(r), strings) => r.matches(strings),
                (None, strings) => strings.is_empty(),
            },
            Self::Iuse(r) => match (r, pkg.iuse()) {
                (Some(r), strings) => r.matches(strings),
                (None, strings) => strings.is_empty(),
            },
            Self::Inherit(r) => match (r, pkg.inherit()) {
                (Some(r), strings) => r.matches(strings),
                (None, strings) => strings.is_empty(),
            },
            Self::Inherited(r) => match (r, pkg.inherited()) {
                (Some(r), strings) => r.matches(strings),
                (None, strings) => strings.is_empty(),
            },
            Self::LongDescription(r) => match (r, pkg.long_description()) {
                (Some(r), Some(s)) => r.matches(s),
                (None, None) => true,
                _ => false,
            },
            Self::Maintainers(r) => match r {
                Some(r) => r.matches(pkg.maintainers()),
                None => pkg.maintainers().is_empty(),
            },
            Self::Upstreams(r) => match r {
                Some(r) => r.matches(pkg.upstreams()),
                None => pkg.upstreams().is_empty(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::config::Config;
    use crate::metadata::Key;

    use super::*;

    #[test]
    fn test_ebuild() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

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
        let r = Restrict::Ebuild(restrict::Str::matches("no match"));
        assert!(!r.matches(&pkg));
        let r = Restrict::Ebuild(restrict::Str::regex("VAR=").unwrap());
        assert!(r.matches(&pkg));

        // verify repo restrictions
        let iter = repo.iter_restrict(r);
        let atoms: Vec<_> = iter.map(|p| p.atom().to_string()).collect();
        assert_eq!(atoms, ["cat/pkg-2"]);

        let r = Restrict::Ebuild(restrict::Str::regex("SLOT=").unwrap());
        let iter = repo.iter_restrict(r);
        let atoms: Vec<_> = iter.map(|p| p.atom().to_string()).collect();
        assert_eq!(atoms, ["cat/pkg-1", "cat/pkg-2"]);
    }

    #[test]
    fn test_description() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        t.create_ebuild("cat/pkg-1", [(Key::Description, "desc1")])
            .unwrap();
        let (path, cpv) = t
            .create_ebuild("cat/pkg-2", [(Key::Description, "desc2")])
            .unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();

        // verify pkg restrictions
        let r = Restrict::Description(restrict::Str::matches("no match"));
        assert!(!r.matches(&pkg));
        let r = Restrict::Description(restrict::Str::matches("desc2"));
        assert!(r.matches(&pkg));

        // verify repo restrictions
        let iter = repo.iter_restrict(r);
        let atoms: Vec<_> = iter.map(|p| p.atom().to_string()).collect();
        assert_eq!(atoms, ["cat/pkg-2"]);

        let r = Restrict::Description(restrict::Str::regex("desc").unwrap());
        let iter = repo.iter_restrict(r);
        let atoms: Vec<_> = iter.map(|p| p.atom().to_string()).collect();
        assert_eq!(atoms, ["cat/pkg-1", "cat/pkg-2"]);
    }

    #[test]
    fn test_slot() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        t.create_ebuild("cat/pkg-0", [(Key::Slot, "0")]).unwrap();
        let (path, cpv) = t.create_ebuild("cat/pkg-1", [(Key::Slot, "1/2")]).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();

        // verify pkg restrictions
        let r = Restrict::Slot(restrict::Str::matches("2"));
        assert!(!r.matches(&pkg));
        let r = Restrict::Slot(restrict::Str::matches("1"));
        assert!(r.matches(&pkg));

        // verify repo restrictions
        let iter = repo.iter_restrict(r);
        let atoms: Vec<_> = iter.map(|p| p.atom().to_string()).collect();
        assert_eq!(atoms, ["cat/pkg-1"]);

        let r = Restrict::Slot(restrict::Str::regex("0|1").unwrap());
        let iter = repo.iter_restrict(r);
        let atoms: Vec<_> = iter.map(|p| p.atom().to_string()).collect();
        assert_eq!(atoms, ["cat/pkg-0", "cat/pkg-1"]);
    }

    #[test]
    fn test_subslot() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        // no explicit subslot
        let (path, cpv) = t.create_ebuild("cat/pkg-0", [(Key::Slot, "0")]).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        let r = Restrict::RawSubslot(None);
        assert!(r.matches(&pkg));

        let (path, cpv) = t.create_ebuild("cat/pkg-1", [(Key::Slot, "1/2")]).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert!(!r.matches(&pkg));

        // verify pkg restrictions
        let r = Restrict::Subslot(restrict::Str::matches("1"));
        assert!(!r.matches(&pkg));
        let r = Restrict::Subslot(restrict::Str::matches("2"));
        assert!(r.matches(&pkg));

        // verify repo restrictions
        let iter = repo.iter_restrict(r);
        let atoms: Vec<_> = iter.map(|p| p.atom().to_string()).collect();
        assert_eq!(atoms, ["cat/pkg-1"]);

        let r = Restrict::Subslot(restrict::Str::regex("0|2").unwrap());
        let iter = repo.iter_restrict(r);
        let atoms: Vec<_> = iter.map(|p| p.atom().to_string()).collect();
        assert_eq!(atoms, ["cat/pkg-0", "cat/pkg-1"]);
    }

    #[test]
    fn test_long_description() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        let (path, cpv) = t.create_ebuild("cat/pkg-a-1", []).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();

        // pkg lacking long description
        let r = Restrict::LongDescription(None);
        assert!(r.matches(&pkg));

        let (path, cpv) = t.create_ebuild("cat/pkg-b-1", []).unwrap();
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
        let r = Restrict::LongDescription(Some(restrict::Str::regex(".").unwrap()));
        assert!(r.matches(&pkg));

        // single repo match
        let iter = repo.iter_restrict(r);
        let atoms: Vec<_> = iter.map(|p| p.atom().to_string()).collect();
        assert_eq!(atoms, ["cat/pkg-b-1"]);

        let (path, _) = t.create_ebuild("cat/pkg-c-1", []).unwrap();
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
        let r = Restrict::LongDescription(Some(restrict::Str::regex("desc").unwrap()));
        let iter = repo.iter_restrict(r);
        let atoms: Vec<_> = iter.map(|p| p.atom().to_string()).collect();
        assert_eq!(atoms, ["cat/pkg-b-1", "cat/pkg-c-1"]);
    }

    #[test]
    fn test_maintainers() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("xml", 0).unwrap();

        // none
        t.create_ebuild("noxml/pkg-1", []).unwrap();

        // single
        let (path, _) = t.create_ebuild("cat/pkg-a-1", []).unwrap();
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
        let (path, _) = t.create_ebuild("cat/pkg-b-1", []).unwrap();
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
        let r = Restrict::Maintainers(None);
        let iter = repo.iter_restrict(r.clone());
        let atoms: Vec<_> = iter.map(|p| p.atom().to_string()).collect();
        assert_eq!(atoms, ["noxml/pkg-1"]);

        // pkgs with maintainers
        let iter = repo.iter_restrict(restrict::Restrict::not(r));
        let atoms: Vec<_> = iter.map(|p| p.atom().to_string()).collect();
        assert_eq!(atoms, ["cat/pkg-a-1", "cat/pkg-b-1"]);
    }

    #[test]
    fn test_upstreams() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("xml", 0).unwrap();

        // none
        t.create_ebuild("noxml/pkg-1", []).unwrap();

        // single
        let (path, _) = t.create_ebuild("cat/pkg-a-1", []).unwrap();
        let data = indoc::indoc! {r#"
            <?xml version="1.0" encoding="UTF-8"?>
            <!DOCTYPE pkgmetadata SYSTEM "https://www.gentoo.org/dtd/metadata.dtd">
            <pkgmetadata>
                <upstream>
                    <remote-id type="github">user/project</remote-id>
                </upstream>
            </pkgmetadata>
        "#};
        fs::write(path.parent().unwrap().join("metadata.xml"), data).unwrap();

        // multiple
        let (path, _) = t.create_ebuild("cat/pkg-b-1", []).unwrap();
        let data = indoc::indoc! {r#"
            <?xml version="1.0" encoding="UTF-8"?>
            <!DOCTYPE pkgmetadata SYSTEM "https://www.gentoo.org/dtd/metadata.dtd">
            <pkgmetadata>
                <upstream>
                    <remote-id type="github">pkgcraft/pkgcraft</remote-id>
                    <remote-id type="pypi">pkgcraft</remote-id>
                </upstream>
            </pkgmetadata>
        "#};
        fs::write(path.parent().unwrap().join("metadata.xml"), data).unwrap();

        // pkgs with no upstreams
        let r = Restrict::Upstreams(None);
        let iter = repo.iter_restrict(r.clone());
        let atoms: Vec<_> = iter.map(|p| p.atom().to_string()).collect();
        assert_eq!(atoms, ["noxml/pkg-1"]);

        // pkgs with upstreams
        let iter = repo.iter_restrict(restrict::Restrict::not(r));
        let atoms: Vec<_> = iter.map(|p| p.atom().to_string()).collect();
        assert_eq!(atoms, ["cat/pkg-a-1", "cat/pkg-b-1"]);
    }
}
