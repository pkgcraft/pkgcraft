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
    // boolean
    And(Vec<Box<Restrict>>),
    Or(Vec<Box<Restrict>>),
}

pub(crate) trait Restriction<T> {
    fn matches(&self, object: T) -> bool;
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
            // boolean
            Restrict::And(vals) => vals.iter().all(|r| r.matches(atom)),
            Restrict::Or(vals) => vals.iter().any(|r| r.matches(atom)),
        }
    }
}

impl From<&atom::Atom> for Restrict {
    fn from(atom: &atom::Atom) -> Self {
        let mut restricts = vec![
            Box::new(Restrict::Category(atom.category().to_string())),
            Box::new(Restrict::Package(atom.package().to_string())),
        ];

        if let Some(v) = atom.version() {
            restricts.push(Box::new(Restrict::Version(Some(v.clone()))));
        }

        if let Some(s) = atom.slot() {
            restricts.push(Box::new(Restrict::Slot(Some(s.to_string()))));
        }

        if let Some(s) = atom.subslot() {
            restricts.push(Box::new(Restrict::SubSlot(Some(s.to_string()))));
        }

        // TODO: add use deps support

        if let Some(s) = atom.repo() {
            restricts.push(Box::new(Restrict::Repo(Some(s.to_string()))));
        }

        Restrict::And(restricts)
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
        let unversioned = Atom::from_str("cat/pkg").unwrap();
        let cpv = parse::cpv("cat/pkg-1").unwrap();
        let full = Atom::from_str("=cat/pkg-1:2/3::repo").unwrap();

        // category
        let r = Restrict::Category("cat".to_string());
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // package
        let r = Restrict::Package("pkg".to_string());
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // no version
        let r = Restrict::Version(None);
        assert!(r.matches(&unversioned));
        assert!(!r.matches(&cpv));
        assert!(!r.matches(&full));

        // version
        let r = Restrict::Version(Some(Version::from_str("1").unwrap()));
        assert!(!r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // no slot
        let r = Restrict::Slot(None);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(!r.matches(&full));

        // slot
        let r = Restrict::Slot(Some("2".to_string()));
        assert!(!r.matches(&unversioned));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&full));

        // no subslot
        let r = Restrict::SubSlot(None);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(!r.matches(&full));

        // subslot
        let r = Restrict::SubSlot(Some("3".to_string()));
        assert!(!r.matches(&unversioned));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&full));

        // no repo
        let r = Restrict::Repo(None);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(!r.matches(&full));

        // repo
        let r = Restrict::Repo(Some("repo".to_string()));
        assert!(!r.matches(&unversioned));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&full));

        // unversioned restriction
        let r = Restrict::from(&unversioned);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // cpv restriction
        let r = Restrict::from(&cpv);
        assert!(!r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // full atom restriction
        let r = Restrict::from(&full);
        assert!(!r.matches(&unversioned));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&full));
    }

    #[test]
    fn test_filtering() {
        let atom_strs = vec!["cat/pkg", ">=cat/pkg-1", "=cat/pkg-1:2/3::repo"];
        let atoms: Vec<Atom> = atom_strs
            .iter()
            .map(|s| Atom::from_str(s).unwrap())
            .collect();

        let filter = |r: Restrict, atoms: Vec<Atom>| -> Vec<String> {
            atoms
                .into_iter()
                .filter(|a| r.matches(a))
                .map(|a| a.to_string())
                .collect()
        };

        let r = Restrict::Category("cat".to_string());
        assert_eq!(filter(r, atoms.clone()), atom_strs);

        let r = Restrict::Version(None);
        assert_eq!(filter(r, atoms.clone()), ["cat/pkg"]);

        let cpv = Atom::from_str("=cat/pkg-1").unwrap();
        let r = Restrict::from(&cpv);
        assert_eq!(filter(r, atoms.clone()), [">=cat/pkg-1", "=cat/pkg-1:2/3::repo"]);
    }

    #[test]
    fn test_and_restrict() {
        let a = Atom::from_str("cat/pkg").unwrap();
        let cat = Restrict::Category("cat".into());
        let pkg = Restrict::Package("pkg".into());
        let r = Restrict::And(vec![Box::new(cat), Box::new(pkg)]);
        assert!(r.matches(&a));

        // one matched and one unmatched restriction
        let cat = Restrict::Category("cat".into());
        let pkg = Restrict::Package("pkga".into());
        let r = Restrict::And(vec![Box::new(cat), Box::new(pkg)]);
        assert!(!r.matches(&a));

        // matching against two atoms
        let a1 = Atom::from_str("cat/pkg1").unwrap();
        let a2 = Atom::from_str("cat/pkg2").unwrap();
        let r = Restrict::And(vec![Box::new(Restrict::from(&a1)), Box::new(Restrict::from(&a2))]);
        assert!(!r.matches(&a1));
        assert!(!r.matches(&a2));
    }

    #[test]
    fn test_or_restrict() {
        let a = Atom::from_str("cat/pkg").unwrap();
        let cat = Restrict::Category("cat".into());
        let pkg = Restrict::Package("pkg".into());
        let r = Restrict::Or(vec![Box::new(cat), Box::new(pkg)]);
        assert!(r.matches(&a));

        // one matched and one unmatched restriction
        let cat = Restrict::Category("cat".into());
        let pkg = Restrict::Package("pkga".into());
        let r = Restrict::Or(vec![Box::new(cat), Box::new(pkg)]);
        assert!(r.matches(&a));

        // matching against two atoms
        let a1 = Atom::from_str("cat/pkg1").unwrap();
        let a2 = Atom::from_str("cat/pkg2").unwrap();
        let r = Restrict::Or(vec![Box::new(Restrict::from(&a1)), Box::new(Restrict::from(&a2))]);
        assert!(r.matches(&a1));
        assert!(r.matches(&a2));
    }
}
