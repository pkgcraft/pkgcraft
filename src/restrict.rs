use crate::atom;

#[derive(Debug)]
pub enum Restrict {
    Category(String),
    Package(String),
    Version(Option<atom::Version>),
    Slot(Option<String>),
    SubSlot(Option<String>),
    Use(Vec<String>, Vec<String>),
    Repo(Option<String>),
}

pub(crate) trait Restriction<T> {
    fn matches(&self, object: T) -> bool;
    fn len(&self) -> usize;
}

impl Restriction<&atom::Atom> for Restrict {
    fn matches(&self, atom: &atom::Atom) -> bool {
        match self {
            Restrict::Category(s) => s.as_str() == atom.category(),
            Restrict::Package(s) => s.as_str() == atom.package(),
            Restrict::Version(v) => v.as_ref() == atom.version(),
            Restrict::Slot(s) => s.as_deref() == atom.slot(),
            Restrict::SubSlot(s) => s.as_deref() == atom.subslot(),
            Restrict::Use(_enabled, _disabled) => unimplemented!(),
            Restrict::Repo(s) => s.as_deref() == atom.repo(),
        }
    }

    fn len(&self) -> usize {
        1
    }
}

struct AndRestrict {
    restricts: Vec<Restrict>,
}

impl Restriction<&atom::Atom> for AndRestrict {
    fn matches(&self, atom: &atom::Atom) -> bool {
        self.restricts.iter().all(|r| r.matches(atom))
    }

    fn len(&self) -> usize {
        self.restricts.len()
    }
}

struct OrRestrict {
    restricts: Vec<Restrict>,
}

impl Restriction<&atom::Atom> for OrRestrict {
    fn matches(&self, atom: &atom::Atom) -> bool {
        self.restricts.iter().any(|r| r.matches(atom))
    }

    fn len(&self) -> usize {
        self.restricts.len()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::atom::parse;
    use crate::atom::{Atom, Version};

    use super::*;

    #[test]
    fn test_atom_restricts() {
        // unversioned
        let a = Atom::from_str("cat/pkg").unwrap();
        let r = AndRestrict {
            restricts: vec![
                Restrict::Category("cat".into()),
                Restrict::Package("pkg".into()),
                Restrict::Version(None),
                Restrict::Slot(None),
                Restrict::SubSlot(None),
                Restrict::Repo(None),
            ],
        };
        assert!(r.matches(&a));

        // cpv
        let a = parse::cpv("cat/pkg-1").unwrap();
        let r = AndRestrict {
            restricts: vec![
                Restrict::Category("cat".into()),
                Restrict::Package("pkg".into()),
                Restrict::Version(Some(Version::from_str("1").unwrap())),
                Restrict::Slot(None),
                Restrict::SubSlot(None),
                Restrict::Repo(None),
            ],
        };
        assert!(r.matches(&a));

        // full atom
        let a = Atom::from_str("=cat/pkg-1:2/3::repo").unwrap();
        let r = AndRestrict {
            restricts: vec![
                Restrict::Category("cat".into()),
                Restrict::Package("pkg".into()),
                Restrict::Version(Some(Version::from_str("1").unwrap())),
                Restrict::Slot(Some("2".into())),
                Restrict::SubSlot(Some("3".into())),
                Restrict::Repo(Some("repo".into())),
            ],
        };
        assert!(r.matches(&a));
    }

    #[test]
    fn test_and_restrict() {
        let a = Atom::from_str("cat/pkg").unwrap();
        let cat = Restrict::Category("cat".into());
        let pkg = Restrict::Package("pkg".into());
        let r = AndRestrict {
            restricts: vec![cat, pkg],
        };
        assert!(r.matches(&a));

        // one matched and one unmatched restriction
        let cat = Restrict::Category("cat".into());
        let pkg = Restrict::Package("pkg2".into());
        let r = AndRestrict {
            restricts: vec![cat, pkg],
        };
        assert!(!r.matches(&a));
    }

    #[test]
    fn test_or_restrict() {
        let a = Atom::from_str("cat/pkg").unwrap();
        let cat = Restrict::Category("cat".into());
        let pkg = Restrict::Package("pkg".into());
        let r = OrRestrict {
            restricts: vec![cat, pkg],
        };
        assert!(r.matches(&a));

        // one matched and one unmatched restriction
        let cat = Restrict::Category("cat".into());
        let pkg = Restrict::Package("pkg2".into());
        let r = OrRestrict {
            restricts: vec![cat, pkg],
        };
        assert!(r.matches(&a));
    }
}
