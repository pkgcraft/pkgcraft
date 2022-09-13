use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt::{self, Write};
use std::str::FromStr;

use cached::{proc_macro::cached, SizedCache};

pub use self::version::Version;
use self::version::{Operator, ParsedVersion};
use crate::eapi::{IntoEapi, EAPI_PKGCRAFT};
use crate::macros::{cmp_not_equal, vec_str};
use crate::restrict::{self, HashSetRestrict, Restriction, SetRestrict, Str};
use crate::Error;
// export parser functionality
pub use parser::parse;

mod parser;
pub(crate) mod version;

type BaseRestrict = restrict::Restrict;

#[repr(C)]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
pub enum Blocker {
    Strong, // !!cat/pkg
    Weak,   // !cat/pkg
}

impl fmt::Display for Blocker {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Blocker::Weak => write!(f, "!"),
            Blocker::Strong => write!(f, "!!"),
        }
    }
}

#[repr(C)]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
pub enum SlotOperator {
    Equal,
    Star,
}

impl fmt::Display for SlotOperator {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Equal => write!(f, "="),
            Self::Star => write!(f, "*"),
        }
    }
}

impl FromStr for SlotOperator {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        match s {
            "*" => Ok(SlotOperator::Star),
            "=" => Ok(SlotOperator::Equal),
            _ => Err(Error::InvalidValue("invalid slot operator".to_string())),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct ParsedAtom<'a> {
    pub(crate) category: &'a str,
    pub(crate) package: &'a str,
    pub(crate) blocker: Option<Blocker>,
    pub(crate) version: Option<ParsedVersion<'a>>,
    pub(crate) version_str: Option<&'a str>,
    pub(crate) slot: Option<&'a str>,
    pub(crate) subslot: Option<&'a str>,
    pub(crate) slot_op: Option<SlotOperator>,
    pub(crate) use_deps: Option<Vec<&'a str>>,
    pub(crate) repo: Option<&'a str>,
}

impl ParsedAtom<'_> {
    pub(crate) fn to_owned(&self) -> crate::Result<Atom> {
        let version = match (self.version.as_ref(), self.version_str) {
            (Some(v), Some(s)) => Some(v.to_owned(s)?),
            _ => None,
        };

        Ok(Atom {
            category: self.category.to_string(),
            package: self.package.to_string(),
            blocker: self.blocker,
            version,
            slot: self.slot.map(|s| s.to_string()),
            subslot: self.subslot.map(|s| s.to_string()),
            slot_op: self.slot_op,
            use_deps: self.use_deps.as_ref().map(|u| vec_str!(u)),
            repo: self.repo.map(|s| s.to_string()),
        })
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Atom {
    category: String,
    package: String,
    blocker: Option<Blocker>,
    version: Option<Version>,
    slot: Option<String>,
    subslot: Option<String>,
    slot_op: Option<SlotOperator>,
    use_deps: Option<Vec<String>>,
    repo: Option<String>,
}

#[cached(
    type = "SizedCache<String, crate::Result<Atom>>",
    create = "{ SizedCache::with_size(1000) }",
    convert = r#"{ s.to_string() }"#
)]
/// Create a new Atom from a given CPV string (e.g. cat/pkg-1).
pub fn cpv(s: &str) -> crate::Result<Atom> {
    let mut atom = parse::cpv(s)?;
    atom.version_str = Some(s);
    atom.to_owned()
}

impl Atom {
    /// Verify a string represents a valid atom.
    pub fn valid<E: IntoEapi>(s: &str, eapi: E) -> crate::Result<()> {
        parse::dep_str(s, eapi.into_eapi()?)?;
        Ok(())
    }

    /// Verify a string represents a valid atom.
    pub fn valid_cpv(s: &str) -> crate::Result<()> {
        parse::cpv(s)?;
        Ok(())
    }

    /// Create a new Atom from a given string.
    pub fn new<E: IntoEapi>(s: &str, eapi: E) -> crate::Result<Self> {
        parse::dep(s, eapi.into_eapi()?)
    }

    /// Return an atom's category.
    pub fn category(&self) -> &str {
        &self.category
    }

    /// Return an atom's package.
    pub fn package(&self) -> &str {
        &self.package
    }

    /// Return an atom's blocker.
    pub fn blocker(&self) -> Option<Blocker> {
        self.blocker
    }

    /// Return the set of an atom's USE flag dependencies.
    fn use_deps_set(&self) -> HashSet<String> {
        match self.use_deps() {
            None => HashSet::<String>::new(),
            Some(u) => u.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Return an atom's USE flag dependencies.
    pub fn use_deps(&self) -> Option<&[String]> {
        self.use_deps.as_deref()
    }

    /// Return an atom's version.
    pub fn version(&self) -> Option<&Version> {
        self.version.as_ref()
    }

    /// Return an atom's revision.
    pub fn revision(&self) -> Option<&version::Revision> {
        self.version.as_ref().map(|v| v.revision())
    }

    /// Return an atom's CAT/PN value, e.g. `>=cat/pkg-1-r2:3` -> `cat/pkg`.
    pub fn key(&self) -> String {
        format!("{}/{}", self.category, self.package)
    }

    /// Return an atom's CPV, e.g. `>=cat/pkg-1-r2:3` -> `cat/pkg-1-r2`.
    pub fn cpv(&self) -> String {
        match &self.version {
            Some(ver) => format!("{}/{}-{ver}", self.category, self.package),
            None => format!("{}/{}", self.category, self.package),
        }
    }

    /// Return an atom's slot.
    pub fn slot(&self) -> Option<&str> {
        self.slot.as_deref()
    }

    /// Return an atom's subslot.
    pub fn subslot(&self) -> Option<&str> {
        self.subslot.as_deref()
    }

    /// Return an atom's slot operator.
    pub fn slot_op(&self) -> Option<SlotOperator> {
        self.slot_op
    }

    /// Return an atom's repository.
    pub fn repo(&self) -> Option<&str> {
        self.repo.as_deref()
    }
}

impl fmt::Display for Atom {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut s = String::new();

        // append blocker
        if let Some(blocker) = self.blocker {
            write!(s, "{}", blocker)?;
        }

        // append version operator with cpv
        let cpv = self.cpv();
        match self.version.as_ref().and_then(|v| v.op()) {
            Some(Operator::Less) => write!(s, "<{cpv}")?,
            Some(Operator::LessOrEqual) => write!(s, "<={cpv}")?,
            Some(Operator::Equal) => write!(s, "={cpv}")?,
            Some(Operator::EqualGlob) => write!(s, "={cpv}*")?,
            Some(Operator::Approximate) => write!(s, "~{cpv}")?,
            Some(Operator::GreaterOrEqual) => write!(s, ">={cpv}")?,
            Some(Operator::Greater) => write!(s, ">{cpv}")?,
            None => s.push_str(&cpv),
        }

        // append slot data
        match (self.slot(), self.subslot(), self.slot_op()) {
            (Some(slot), Some(subslot), Some(op)) => write!(s, ":{slot}/{subslot}{op}")?,
            (Some(slot), Some(subslot), None) => write!(s, ":{slot}/{subslot}")?,
            (Some(slot), None, Some(op)) => write!(s, ":{slot}{op}")?,
            (Some(x), None, None) => write!(s, ":{x}")?,
            (None, None, Some(x)) => write!(s, ":{x}")?,
            _ => (),
        }

        // append use deps
        if let Some(x) = &self.use_deps {
            write!(s, "[{}]", &x.join(","))?;
        }

        // append repo
        if let Some(repo) = &self.repo {
            write!(s, "::{repo}")?;
        }

        write!(f, "{s}")
    }
}

impl Ord for Atom {
    fn cmp(&self, other: &Self) -> Ordering {
        cmp_not_equal!(&self.category, &other.category);
        cmp_not_equal!(&self.package, &other.package);
        cmp_not_equal!(&self.version, &other.version);
        cmp_not_equal!(&self.blocker, &other.blocker);
        cmp_not_equal!(&self.slot, &other.slot);
        cmp_not_equal!(&self.subslot, &other.subslot);
        cmp_not_equal!(&self.use_deps, &other.use_deps);
        self.repo.cmp(&other.repo)
    }
}

impl PartialOrd for Atom {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl FromStr for Atom {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Atom::new(s, &*EAPI_PKGCRAFT)
    }
}

#[derive(Debug, Clone)]
pub enum Restrict {
    Category(Str),
    Package(Str),
    Blocker(Option<Blocker>),
    Version(Option<Version>),
    VersionStr(Str),
    Slot(Option<Str>),
    Subslot(Option<Str>),
    UseDeps(HashSetRestrict<String>),
    Repo(Option<Str>),

    // boolean
    And(Vec<Box<Self>>),
}

impl Restrict {
    pub fn category(s: &str) -> Self {
        Self::Category(Str::equal(s))
    }

    pub fn package(s: &str) -> Self {
        Self::Package(Str::equal(s))
    }

    pub fn version(s: &str) -> crate::Result<Self> {
        let v = Version::from_str(s)?;
        Ok(Self::Version(Some(v)))
    }

    pub fn slot(o: Option<&str>) -> Self {
        Self::Slot(o.map(Str::equal))
    }

    pub fn subslot(o: Option<&str>) -> Self {
        Self::Subslot(o.map(Str::equal))
    }

    pub fn use_deps<I, S>(iter: Option<I>) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let r = match iter {
            None => SetRestrict::Empty,
            Some(i) => SetRestrict::Superset(i.into_iter().map(|s| s.into()).collect()),
        };
        Self::UseDeps(r)
    }

    pub fn repo(o: Option<&str>) -> Self {
        Self::Repo(o.map(Str::equal))
    }
}

impl Restriction<&Atom> for Restrict {
    fn matches(&self, atom: &Atom) -> bool {
        use self::Restrict::*;
        match self {
            Category(r) => r.matches(atom.category()),
            Package(r) => r.matches(atom.package()),
            Blocker(b) => match (b, atom.blocker()) {
                (Some(b), Some(blocker)) => *b == blocker,
                (None, None) => true,
                _ => false,
            },
            Version(v) => match (v, atom.version()) {
                (Some(v), Some(ver)) => v.op_cmp(ver),
                (None, None) => true,
                _ => false,
            },
            VersionStr(r) => r.matches(atom.version().map_or_else(|| "", |v| v.as_str())),
            Slot(r) => match (r, atom.slot()) {
                (Some(r), Some(slot)) => r.matches(slot),
                (None, None) => true,
                _ => false,
            },
            Subslot(r) => match (r, atom.subslot()) {
                (Some(r), Some(subslot)) => r.matches(subslot),
                (None, None) => true,
                _ => false,
            },
            UseDeps(r) => r.matches(&atom.use_deps_set()),
            Repo(r) => match (r, atom.repo()) {
                (Some(r), Some(repo)) => r.matches(repo),
                (None, None) => true,
                _ => false,
            },
            And(vals) => vals.iter().all(|r| r.matches(atom)),
        }
    }
}

impl From<Restrict> for BaseRestrict {
    fn from(r: Restrict) -> Self {
        Self::Atom(r)
    }
}

impl Restriction<&Atom> for BaseRestrict {
    fn matches(&self, atom: &Atom) -> bool {
        crate::restrict::restrict_match! {
            self, atom,
            Self::Atom(r) => r.matches(atom)
        }
    }
}

impl From<&Atom> for Restrict {
    fn from(atom: &Atom) -> Self {
        let mut restricts = vec![
            Box::new(Restrict::category(atom.category())),
            Box::new(Restrict::package(atom.package())),
            Box::new(Restrict::Blocker(atom.blocker())),
        ];

        if let Some(v) = atom.version() {
            restricts.push(Box::new(Restrict::Version(Some(v.clone()))));
        }

        if let Some(s) = atom.slot() {
            restricts.push(Box::new(Restrict::slot(Some(s))));
        }

        if let Some(s) = atom.subslot() {
            restricts.push(Box::new(Restrict::subslot(Some(s))));
        }

        if let Some(u) = atom.use_deps() {
            restricts.push(Box::new(Restrict::use_deps(Some(u))));
        }

        if let Some(s) = atom.repo() {
            restricts.push(Box::new(Restrict::repo(Some(s))));
        }

        Restrict::And(restricts)
    }
}

impl From<Atom> for Restrict {
    fn from(atom: Atom) -> Self {
        (&atom).into()
    }
}

impl From<&Atom> for BaseRestrict {
    fn from(atom: &Atom) -> Self {
        BaseRestrict::Atom(atom.into())
    }
}

impl From<Atom> for BaseRestrict {
    fn from(atom: Atom) -> Self {
        BaseRestrict::Atom((&atom).into())
    }
}

#[cfg(test)]
mod tests {
    use crate::test::{Atoms, Versions};

    use super::*;

    #[test]
    fn test_fmt() {
        let mut atom: Atom;
        for s in [
            "cat/pkg",
            "<cat/pkg-4",
            "<=cat/pkg-4-r1",
            "=cat/pkg-4-r0",
            "=cat/pkg-4-r01",
            "=cat/pkg-4*",
            "~cat/pkg-4",
            ">=cat/pkg-r1-2-r3",
            ">cat/pkg-4-r1:0=",
            ">cat/pkg-4-r1:0/2=[use]",
            ">cat/pkg-4-r1:0/2=[use]::repo",
            "!cat/pkg",
            "!!<cat/pkg-4",
        ] {
            atom = Atom::from_str(&s).unwrap();
            assert_eq!(format!("{atom}"), s);
        }
    }

    #[test]
    fn test_atom_key() {
        let mut atom: Atom;
        for (s, key) in [
            ("cat/pkg", "cat/pkg"),
            ("<cat/pkg-4", "cat/pkg"),
            ("<=cat/pkg-4-r1", "cat/pkg"),
            ("=cat/pkg-4", "cat/pkg"),
            ("=cat/pkg-4*", "cat/pkg"),
            ("~cat/pkg-4", "cat/pkg"),
            (">=cat/pkg-r1-2-r3", "cat/pkg-r1"),
            (">cat/pkg-4-r1:0=", "cat/pkg"),
        ] {
            atom = Atom::from_str(&s).unwrap();
            assert_eq!(atom.key(), key);
        }
    }

    #[test]
    fn test_atom_version() {
        let mut atom: Atom;
        for (s, version) in [
            ("cat/pkg", None),
            ("<cat/pkg-4", Some("<4")),
            ("<=cat/pkg-4-r1", Some("<=4-r1")),
            ("=cat/pkg-4", Some("=4")),
            ("=cat/pkg-4*", Some("=4*")),
            ("~cat/pkg-4", Some("~4")),
            (">=cat/pkg-r1-2-r3", Some(">=2-r3")),
            (">cat/pkg-4-r1:0=", Some(">4-r1")),
        ] {
            atom = Atom::from_str(&s).unwrap();
            let version = version.map(|s| parse::version_with_op(s).unwrap());
            assert_eq!(atom.version(), version.as_ref());
        }
    }

    #[test]
    fn test_atom_revision() {
        let mut atom: Atom;
        for (s, revision) in [
            ("cat/pkg", None),
            ("<cat/pkg-4", Some("0")),
            ("<=cat/pkg-4-r1", Some("1")),
            (">=cat/pkg-r1-2-r3", Some("3")),
            (">cat/pkg-4-r1:0=", Some("1")),
        ] {
            atom = Atom::from_str(&s).unwrap();
            let revision = revision.map(|s| version::Revision::from_str(s).unwrap());
            assert_eq!(atom.revision(), revision.as_ref(), "{s} failed");
        }
    }

    #[test]
    fn test_atom_cpv() {
        let mut atom: Atom;
        for (s, cpv) in [
            ("cat/pkg", "cat/pkg"),
            ("<cat/pkg-4", "cat/pkg-4"),
            ("<=cat/pkg-4-r1", "cat/pkg-4-r1"),
            ("=cat/pkg-4", "cat/pkg-4"),
            ("=cat/pkg-4*", "cat/pkg-4"),
            ("~cat/pkg-4", "cat/pkg-4"),
            (">=cat/pkg-r1-2-r3", "cat/pkg-r1-2-r3"),
            (">cat/pkg-4-r1:0=", "cat/pkg-4-r1"),
        ] {
            atom = Atom::from_str(&s).unwrap();
            assert_eq!(atom.cpv(), cpv);
        }
    }

    #[test]
    fn test_sorting() {
        let atoms = Atoms::load().unwrap();
        for (unsorted, expected) in atoms.sorting.iter() {
            let mut atoms: Vec<_> = unsorted
                .iter()
                .map(|s| Atom::from_str(s).unwrap())
                .collect();
            atoms.sort();
            let sorted: Vec<_> = atoms.iter().map(|x| format!("{x}")).collect();
            assert_eq!(&sorted, expected);
        }
    }

    #[test]
    fn test_hashing() {
        let data = Versions::load().unwrap();
        for (versions, size) in data.hashing.iter() {
            let atoms: HashSet<_> = versions
                .iter()
                .map(|s| Atom::from_str(&format!("=cat/pkg-{s}")).unwrap())
                .collect();
            assert_eq!(atoms.len(), *size);
        }
    }

    #[test]
    fn test_restrict_methods() {
        let unversioned = Atom::from_str("cat/pkg").unwrap();
        let blocker = Atom::from_str("!cat/pkg").unwrap();
        let cpv = cpv("cat/pkg-1").unwrap();
        let full = Atom::from_str("=cat/pkg-1:2/3[u1,u2]::repo").unwrap();

        // category
        let r = Restrict::category("cat");
        assert!(r.matches(&unversioned));
        assert!(r.matches(&blocker));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // package
        let r = Restrict::package("pkg");
        assert!(r.matches(&unversioned));
        assert!(r.matches(&blocker));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // blocker
        let r = Restrict::Blocker(None);
        assert!(r.matches(&unversioned));
        assert!(!r.matches(&blocker));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));
        let r = Restrict::Blocker(Some(Blocker::Weak));
        assert!(!r.matches(&unversioned));
        assert!(r.matches(&blocker));
        assert!(!r.matches(&cpv));
        assert!(!r.matches(&full));

        // no version
        let r = Restrict::Version(None);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&blocker));
        assert!(!r.matches(&cpv));
        assert!(!r.matches(&full));

        // version
        let r = Restrict::version("1").unwrap();
        assert!(!r.matches(&unversioned));
        assert!(!r.matches(&blocker));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // no slot
        let r = Restrict::slot(None);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&blocker));
        assert!(r.matches(&cpv));
        assert!(!r.matches(&full));

        // slot
        let r = Restrict::slot(Some("2"));
        assert!(!r.matches(&unversioned));
        assert!(!r.matches(&blocker));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&full));

        // no subslot
        let r = Restrict::subslot(None);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&blocker));
        assert!(r.matches(&cpv));
        assert!(!r.matches(&full));

        // subslot
        let r = Restrict::subslot(Some("3"));
        assert!(!r.matches(&unversioned));
        assert!(!r.matches(&blocker));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&full));

        // no use deps specified
        let r = Restrict::use_deps(None::<&[String]>);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&blocker));
        assert!(r.matches(&cpv));
        assert!(!r.matches(&full));

        // use deps specified
        for u in [vec!["u1"], vec!["u1", "u2"]] {
            let r = Restrict::use_deps(Some(u));
            assert!(!r.matches(&unversioned));
            assert!(!r.matches(&blocker));
            assert!(!r.matches(&cpv));
            assert!(r.matches(&full));
        }

        // no repo
        let r = Restrict::repo(None);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&blocker));
        assert!(r.matches(&cpv));
        assert!(!r.matches(&full));

        // repo
        let r = Restrict::repo(Some("repo"));
        assert!(!r.matches(&unversioned));
        assert!(!r.matches(&blocker));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&full));
    }

    #[test]
    fn test_restrict_conversion() {
        let unversioned = Atom::from_str("cat/pkg").unwrap();
        let cpv = cpv("cat/pkg-1").unwrap();
        let full = Atom::from_str("=cat/pkg-1:2/3[u1,u2]::repo").unwrap();

        // unversioned restriction
        let r = BaseRestrict::from(&unversioned);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // cpv restriction
        let r = BaseRestrict::from(&cpv);
        assert!(!r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // full atom restriction
        let r = BaseRestrict::from(&full);
        assert!(!r.matches(&unversioned));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&full));
    }

    #[test]
    fn test_restrict_versions() {
        let lt = Atom::from_str("<cat/pkg-1-r1").unwrap();
        let le = Atom::from_str("<=cat/pkg-1-r1").unwrap();
        let eq = Atom::from_str("=cat/pkg-1-r1").unwrap();
        let eq_glob = Atom::from_str("=cat/pkg-1*").unwrap();
        let approx = Atom::from_str("~cat/pkg-1").unwrap();
        let ge = Atom::from_str(">=cat/pkg-1-r1").unwrap();
        let gt = Atom::from_str(">cat/pkg-1-r1").unwrap();

        let lt_cpv = cpv("cat/pkg-0").unwrap();
        let gt_cpv = cpv("cat/pkg-2").unwrap();

        let r = BaseRestrict::from(&lt);
        assert!(r.matches(&lt_cpv));
        assert!(!r.matches(&lt));
        assert!(!r.matches(&gt_cpv));

        let r = BaseRestrict::from(&le);
        assert!(r.matches(&lt_cpv));
        assert!(r.matches(&le));
        assert!(!r.matches(&gt_cpv));

        let r = BaseRestrict::from(&eq);
        assert!(!r.matches(&lt_cpv));
        assert!(r.matches(&eq));
        assert!(!r.matches(&gt_cpv));

        let r = BaseRestrict::from(&eq_glob);
        assert!(!r.matches(&lt_cpv));
        assert!(r.matches(&eq_glob));
        for s in ["cat/pkg-1-r1", "cat/pkg-10", "cat/pkg-1.0.1"] {
            let cpv = cpv(s).unwrap();
            assert!(r.matches(&cpv));
        }
        assert!(!r.matches(&gt_cpv));
        let r = BaseRestrict::from(&approx);
        assert!(!r.matches(&lt_cpv));
        assert!(r.matches(&approx));
        for s in ["cat/pkg-1-r1", "cat/pkg-1-r999"] {
            let cpv = cpv(s).unwrap();
            assert!(r.matches(&cpv));
        }
        assert!(!r.matches(&gt_cpv));

        let r = BaseRestrict::from(&ge);
        assert!(!r.matches(&lt_cpv));
        assert!(r.matches(&ge));
        assert!(r.matches(&gt_cpv));

        let r = BaseRestrict::from(&gt);
        assert!(!r.matches(&lt_cpv));
        assert!(!r.matches(&gt));
        assert!(r.matches(&gt_cpv));
    }
}
