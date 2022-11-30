use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::iter::zip;
use std::str::FromStr;
use std::{fmt, str};

use super::parse;
use crate::macros::cmp_not_equal;
use crate::Error;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Suffix {
    Alpha, // _alpha
    Beta,  // _beta
    Pre,   // _pre
    Rc,    // _rc
    P,     // _p
}

impl FromStr for Suffix {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Suffix> {
        match s {
            "alpha" => Ok(Suffix::Alpha),
            "beta" => Ok(Suffix::Beta),
            "pre" => Ok(Suffix::Pre),
            "rc" => Ok(Suffix::Rc),
            "p" => Ok(Suffix::P),
            _ => Err(Error::InvalidValue(format!("invalid suffix: {s}"))),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Revision {
    value: Option<String>,
    int: u64,
}

impl FromStr for Revision {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        let int = s
            .parse()
            .map_err(|e| Error::InvalidValue(format!("invalid revision: {e}: {s}")))?;
        Ok(Revision {
            value: Some(s.to_string()),
            int,
        })
    }
}

impl Revision {
    fn new(rev: Option<&str>) -> crate::Result<Self> {
        match &rev {
            Some(s) => Revision::from_str(s),
            None => Ok(Revision::default()),
        }
    }

    pub fn as_str(&self) -> &str {
        self.value.as_deref().unwrap_or("0")
    }
}

impl PartialEq for Revision {
    fn eq(&self, other: &Self) -> bool {
        self.int == other.int
    }
}

impl Eq for Revision {}

impl Hash for Revision {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.int.hash(state);
    }
}

impl PartialEq<str> for Revision {
    fn eq(&self, other: &str) -> bool {
        match &self.value {
            Some(s) => s == other,
            None => "0" == other,
        }
    }
}

impl Ord for Revision {
    fn cmp(&self, other: &Self) -> Ordering {
        self.int.cmp(&other.int)
    }
}

impl PartialOrd for Revision {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.int.partial_cmp(&other.int)
    }
}

impl fmt::Display for Revision {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<&Revision> for String {
    fn from(rev: &Revision) -> Self {
        rev.as_str().into()
    }
}

#[derive(Debug)]
pub(crate) struct ParsedVersion<'a> {
    pub(crate) start: usize,
    pub(crate) start_base: usize,
    pub(crate) end_base: usize,
    pub(crate) end: usize,
    pub(crate) op: Option<Operator>,
    pub(crate) numbers: Vec<&'a str>,
    pub(crate) letter: Option<char>,
    pub(crate) suffixes: Option<Vec<(&'a str, Option<&'a str>)>>,
    pub(crate) revision: Option<&'a str>,
}

impl<'a> ParsedVersion<'a> {
    // Used by the parser to inject the version operator value.
    pub(crate) fn with_op(
        mut self,
        op: &'a str,
        glob: Option<&'a str>,
    ) -> Result<Self, &'static str> {
        use Operator::*;
        let op = match (op, glob) {
            ("<", None) => Ok(Less),
            ("<=", None) => Ok(LessOrEqual),
            ("=", None) => Ok(Equal),
            ("=", Some(_)) => Ok(EqualGlob),
            ("~", None) => match self.revision {
                None => Ok(Approximate),
                Some(_) => Err("~ version operator can't be used with a revision"),
            },
            (">=", None) => Ok(GreaterOrEqual),
            (">", None) => Ok(Greater),
            _ => Err("invalid version operator"),
        }?;

        self.op = Some(op);
        Ok(self)
    }

    pub(crate) fn to_owned(&self, input: &str) -> crate::Result<Version> {
        let mut numbers = Vec::<(String, u64)>::new();
        for s in self.numbers.iter() {
            let num = s
                .parse()
                .map_err(|e| Error::InvalidValue(format!("invalid version: {e}: {s}")))?;
            numbers.push((s.to_string(), num));
        }

        let mut suffixes = Vec::<(Suffix, Option<u64>)>::new();
        if let Some(vals) = self.suffixes.as_ref() {
            for (s, v) in vals.iter() {
                let suffix = Suffix::from_str(s)?;
                let num =
                    match v {
                        None => None,
                        Some(x) => Some(x.parse().map_err(|e| {
                            Error::InvalidValue(format!("invalid version: {e}: {x}"))
                        })?),
                    };
                suffixes.push((suffix, num));
            }
        }

        Ok(Version {
            start_base: self.start_base - self.start,
            end_base: self.end_base - self.start,
            full: input[self.start..self.end].to_string(),
            op: self.op,
            numbers,
            letter: self.letter,
            suffixes,
            revision: Revision::new(self.revision)?,
        })
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
pub(crate) enum Operator {
    Less,           // <1
    LessOrEqual,    // <=1
    Equal,          // =1
    EqualGlob,      // =1*
    Approximate,    // ~1
    GreaterOrEqual, // >=1
    Greater,        // >1
}

#[derive(Debug, Clone)]
pub struct Version {
    start_base: usize,
    end_base: usize,
    full: String,
    op: Option<Operator>,
    numbers: Vec<(String, u64)>,
    letter: Option<char>,
    suffixes: Vec<(Suffix, Option<u64>)>,
    revision: Revision,
}

impl Version {
    /// Verify a string represents a valid version.
    pub fn valid(s: &str) -> crate::Result<()> {
        parse::version_str(s)?;
        Ok(())
    }

    /// Create a new Version from a given string.
    pub fn new(s: &str) -> crate::Result<Self> {
        parse::version(s)
    }

    /// Create a new Version from a given string with operators.
    pub fn new_with_op(s: &str) -> crate::Result<Self> {
        parse::version_with_op(s)
    }

    /// Return a version's string value.
    pub fn as_str(&self) -> &str {
        &self.full
    }

    /// Return a version's revision.
    pub fn revision(&self) -> &Revision {
        &self.revision
    }

    /// Return a version's operator, if one exists.
    pub(crate) fn op(&self) -> Option<Operator> {
        self.op
    }

    /// Return a version's base -- all components except the revision.
    pub(crate) fn base(&self) -> &str {
        &self.full[self.start_base..self.end_base]
    }

    /// Compare two versions for restrictions.
    pub(crate) fn op_cmp(&self, other: &Self) -> bool {
        use Operator::*;
        match self.op() {
            Some(Less) => NonOpVersion(other) < NonOpVersion(self),
            Some(LessOrEqual) => NonOpVersion(other) <= NonOpVersion(self),
            Some(Equal) | None => NonOpVersion(other) == NonOpVersion(self),
            Some(EqualGlob) => other.as_str().starts_with(self.as_str()),
            Some(Approximate) => NonRevisionVersion(other) == NonRevisionVersion(self),
            Some(GreaterOrEqual) => NonOpVersion(other) >= NonOpVersion(self),
            Some(Greater) => NonOpVersion(other) > NonOpVersion(self),
        }
    }
}

impl AsRef<Version> for Version {
    fn as_ref(&self) -> &Version {
        self
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<&Version> for String {
    fn from(ver: &Version) -> Self {
        ver.as_str().into()
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Version {}

impl Hash for Version {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.numbers[0].1.hash(state);
        for (v1, n1) in &self.numbers[1..] {
            if v1.starts_with('0') {
                v1.trim_end_matches('0').hash(state);
            } else {
                n1.hash(state);
            }
        }
        self.letter.hash(state);
        self.suffixes.hash(state);
        self.revision.hash(state);
    }
}

fn ver_cmp<V: AsRef<Version>>(v1: V, v2: V, cmp_revs: bool, cmp_ops: bool) -> Ordering {
    let (v1, v2) = (v1.as_ref(), v2.as_ref());

    if v1.base() != v2.base() {
        // compare major versions
        cmp_not_equal!(&v1.numbers[0].1, &v2.numbers[0].1);

        // iterate through the remaining version components
        for ((s1, n1), (s2, n2)) in zip(&v1.numbers[1..], &v2.numbers[1..]) {
            // if string is lexically equal, it is numerically equal too
            if s1 == s2 {
                continue;
            }

            // If one of the components starts with a "0" then they are compared as strings
            // with trailing 0's stripped, otherwise they are compared as integers.
            if s1.starts_with('0') || s2.starts_with('0') {
                cmp_not_equal!(s1.trim_end_matches('0'), s2.trim_end_matches('0'));
            } else {
                cmp_not_equal!(&n1, &n2);
            }
        }

        // compare the number of version components
        cmp_not_equal!(&v1.numbers.len(), &v2.numbers.len());

        // dotted components were equal so compare letter suffixes
        cmp_not_equal!(&v1.letter, &v2.letter);

        for ((s1, n1), (s2, n2)) in zip(&v1.suffixes, &v2.suffixes) {
            // if suffixes differ, use them for comparison
            cmp_not_equal!(s1, s2);
            // otherwise use the suffix versions for comparison
            cmp_not_equal!(n1, n2);
        }

        // If one version has more suffixes, use the last suffix to determine ordering.
        match v1.suffixes.cmp(&v2.suffixes) {
            Ordering::Equal => (),
            Ordering::Greater => match v1.suffixes.last() {
                Some((Suffix::P, _)) => return Ordering::Greater,
                _ => return Ordering::Less,
            },
            Ordering::Less => match v2.suffixes.last() {
                Some((Suffix::P, _)) => return Ordering::Less,
                _ => return Ordering::Greater,
            },
        }
    }

    // compare the revisions
    if cmp_revs {
        cmp_not_equal!(&v1.revision, &v2.revision);
    }

    // compare the operators
    if cmp_ops {
        cmp_not_equal!(&v1.op, &v2.op);
    }

    Ordering::Equal
}

impl Ord for Version {
    fn cmp<'a>(&'a self, other: &'a Self) -> Ordering {
        ver_cmp(self, other, true, true)
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl FromStr for Version {
    type Err = Error;

    #[inline]
    fn from_str(s: &str) -> crate::Result<Self> {
        parse::version(s)
    }
}

/// Version wrapper that ignores revisions and operators during comparisons.
#[derive(Debug, Eq, Hash, Clone)]
struct NonRevisionVersion<'a>(&'a Version);

impl AsRef<Version> for NonRevisionVersion<'_> {
    fn as_ref(&self) -> &Version {
        self.0
    }
}

impl PartialEq for NonRevisionVersion<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Ord for NonRevisionVersion<'_> {
    fn cmp<'a>(&'a self, other: &'a Self) -> Ordering {
        ver_cmp(self, other, false, false)
    }
}

impl PartialOrd for NonRevisionVersion<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Version wrapper that ignores operators during comparisons.
#[derive(Debug, Eq, Hash, Clone)]
struct NonOpVersion<'a>(&'a Version);

impl AsRef<Version> for NonOpVersion<'_> {
    fn as_ref(&self) -> &Version {
        self.0
    }
}

impl PartialEq for NonOpVersion<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Ord for NonOpVersion<'_> {
    fn cmp<'a>(&'a self, other: &'a Self) -> Ordering {
        ver_cmp(self, other, true, false)
    }
}

impl PartialOrd for NonOpVersion<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::*;
    use crate::macros::*;
    use crate::test::Versions;
    use crate::Error;

    #[test]
    fn test_overflow_version() {
        let u64_max: u128 = u64::MAX as u128;

        for (s1, s2) in [
            // major version
            (format!("{u64_max}"), format!("{}", u64_max + 1)),
            // minor version
            (format!("1.{u64_max}"), format!("1.{}", u64_max + 1)),
            // suffix version
            (format!("1_p{u64_max}"), format!("1_p{}", u64_max + 1)),
            // revision
            (format!("1-r{u64_max}"), format!("1-r{}", u64_max + 1)),
        ] {
            // at bounds limit
            let v1 = Version::from_str(&s1);
            assert!(v1.is_ok());
            // above bounds limit
            let v2 = Version::from_str(&s2);
            assert_err!(&v2, Err(Error::InvalidValue(_)));
            assert_err_re!(v2, format!("^.*: {}$", u64_max + 1));
        }
    }

    #[test]
    fn test_cmp() {
        let op_map: HashMap<&str, Ordering> =
            [("<", Ordering::Less), ("==", Ordering::Equal), (">", Ordering::Greater)]
                .into_iter()
                .collect();

        let data = Versions::load().unwrap();
        for (expr, (v1, op, v2)) in data.compares() {
            let v1 = Version::from_str(v1).unwrap();
            let v2 = Version::from_str(v2).unwrap();
            match op {
                "!=" => {
                    assert_ne!(v1, v2, "failed comparing {expr}");
                    assert_ne!(v2, v1, "failed comparing {expr}");

                    // verify version hashes aren't equal
                    let set = HashSet::from([v1, v2]);
                    assert_eq!(set.len(), 2, "failed hash {expr}");
                }
                _ => {
                    let op = op_map[op];
                    assert_eq!(v1.cmp(&v2), op, "failed comparing {expr}");
                    assert_eq!(v2.cmp(&v1), op.reverse(), "failed comparing {expr}");

                    // verify version hashes
                    let set = HashSet::from([v1, v2]);
                    match op {
                        Ordering::Equal => assert_eq!(set.len(), 1, "failed hash {expr}"),
                        _ => assert_eq!(set.len(), 2, "failed hash {expr}"),
                    }
                }
            }
        }
    }

    #[test]
    fn test_sorting() {
        let data = Versions::load().unwrap();
        for (unsorted, expected) in data.sorting.iter() {
            let mut versions: Vec<_> = unsorted
                .iter()
                .map(|s| Version::from_str(s).unwrap())
                .collect();
            versions.sort();
            let sorted: Vec<_> = versions.iter().map(|x| format!("{x}")).collect();
            assert_eq!(&sorted, expected);
        }
    }

    #[test]
    fn test_hashing() {
        let data = Versions::load().unwrap();
        for (versions, size) in data.hashing.iter() {
            let versions: HashSet<_> = versions
                .iter()
                .map(|s| Version::from_str(s).unwrap())
                .collect();
            assert_eq!(versions.len(), *size);
        }
    }
}
