use std::{fmt, ptr};

use crate::pkg::{self, Package};
use crate::restrict::{self, Restriction};

use super::Pkg;

#[derive(Clone)]
pub enum Restrict {
    Custom(fn(&Pkg) -> bool),
    Ebuild(restrict::Str),
    Description(restrict::Str),
    Slot(restrict::Str),
    Subslot(restrict::Str),
    Homepage(Option<restrict::SliceStrs>),
    DefinedPhases(Option<restrict::HashSetStrs>),
    Keywords(Option<restrict::IndexSetStrs>),
    Iuse(Option<restrict::IndexSetStrs>),
    Inherit(Option<restrict::IndexSetStrs>),
    Inherited(Option<restrict::IndexSetStrs>),
    LongDescription(Option<restrict::Str>),
}

impl fmt::Debug for Restrict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Custom(func) => write!(f, "Custom(func: {:?})", ptr::addr_of!(func)),
            Self::Ebuild(r) => write!(f, "Ebuild({r:?})"),
            Self::Description(r) => write!(f, "Description({r:?})"),
            Self::Slot(r) => write!(f, "Slot({r:?})"),
            Self::Subslot(r) => write!(f, "Subslot({r:?})"),
            Self::Homepage(r) => write!(f, "Homepage({r:?})"),
            Self::DefinedPhases(r) => write!(f, "DefinedPhases({r:?})"),
            Self::Keywords(r) => write!(f, "Keywords({r:?})"),
            Self::Iuse(r) => write!(f, "Iuse({r:?})"),
            Self::Inherit(r) => write!(f, "Inherit({r:?})"),
            Self::Inherited(r) => write!(f, "Inherited({r:?})"),
            Self::LongDescription(r) => write!(f, "LongDescription({r:?})"),
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
        restrict::restrict_match! {
            self, pkg,
            Self::Atom(r) => r.matches(pkg.atom()),
            Self::Pkg(pkg::Restrict::Ebuild(r)) => r.matches(pkg)
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
            Self::Description(r) => r.matches(pkg.description()),
            Self::Slot(r) => r.matches(pkg.slot()),
            Self::Subslot(r) => r.matches(pkg.subslot()),
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
                (Some(r), Some(long_desc)) => r.matches(long_desc),
                (None, None) => true,
                _ => false,
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
    fn test_description() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        t.create_ebuild("cat/pkg-1", [(Key::Description, "desc1")])
            .unwrap();
        let path = t
            .create_ebuild("cat/pkg-2", [(Key::Description, "desc2")])
            .unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();

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
    fn test_long_description() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        let path = t.create_ebuild("cat/pkg-a-1", []).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();

        // pkg lacking long description
        let r = Restrict::LongDescription(None);
        assert!(r.matches(&pkg));

        let path = t.create_ebuild("cat/pkg-b-1", []).unwrap();
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
        let pkg = Pkg::new(&path, &repo).unwrap();

        // pkg with long description
        let r = Restrict::LongDescription(Some(restrict::Str::regex(".").unwrap()));
        assert!(r.matches(&pkg));

        // single repo match
        let iter = repo.iter_restrict(r);
        let atoms: Vec<_> = iter.map(|p| p.atom().to_string()).collect();
        assert_eq!(atoms, ["cat/pkg-b-1"]);

        let path = t.create_ebuild("cat/pkg-c-1", []).unwrap();
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
}
