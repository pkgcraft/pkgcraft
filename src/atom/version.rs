use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::iter::zip;
use std::str::FromStr;
use std::{fmt, str};

use super::{cmp_not_equal, parse};
use crate::{Error, Result};

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

    fn from_str(s: &str) -> Result<Suffix> {
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

#[derive(Debug, Default, Eq, Clone)]
pub struct Revision {
    value: Option<String>,
    int: u64,
}

impl FromStr for Revision {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let int: u64 = s
            .parse()
            .map_err(|e| Error::InvalidValue(format!("invalid revision: {e}: {s}")))?;
        Ok(Revision {
            value: Some(s.to_string()),
            int,
        })
    }
}

impl Revision {
    fn new(rev: Option<&str>) -> Result<Self> {
        match &rev {
            Some(s) => Revision::from_str(s),
            None => Ok(Revision::default()),
        }
    }

    pub fn as_str(&self) -> &str {
        self.value.as_deref().unwrap_or("")
    }
}

impl PartialEq for Revision {
    fn eq(&self, other: &Self) -> bool {
        self.int == other.int
    }
}

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
    pub(crate) end_base: usize,
    pub(crate) end: usize,
    pub(crate) numbers: Vec<&'a str>,
    pub(crate) letter: Option<char>,
    pub(crate) suffixes: Option<Vec<(&'a str, Option<&'a str>)>>,
    pub(crate) revision: Option<&'a str>,
}

impl ParsedVersion<'_> {
    pub(crate) fn into_owned(self, input: &str) -> Result<Version> {
        let mut numbers = Vec::<(String, u64)>::new();
        for s in self.numbers.iter() {
            let num = s
                .parse()
                .map_err(|e| Error::InvalidValue(format!("invalid version: {e}: {s}")))?;
            numbers.push((s.to_string(), num));
        }

        let mut suffixes = Vec::<(Suffix, Option<u64>)>::new();
        if let Some(vals) = self.suffixes {
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
            end_base: self.end_base - self.start,
            full: input[self.start..self.end].to_string(),
            numbers,
            letter: self.letter,
            suffixes,
            revision: Revision::new(self.revision)?,
        })
    }
}

#[derive(Debug, Eq, Clone)]
pub struct Version {
    end_base: usize,
    full: String,
    numbers: Vec<(String, u64)>,
    letter: Option<char>,
    suffixes: Vec<(Suffix, Option<u64>)>,
    revision: Revision,
}

impl Version {
    pub fn as_str(&self) -> &str {
        &self.full
    }

    pub fn revision(&self) -> Option<&Revision> {
        self.revision.value.as_ref().map(|_| &self.revision)
    }

    pub(crate) fn base(&self) -> &str {
        let base = &self.full.as_bytes()[..self.end_base];
        str::from_utf8(base).unwrap()
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

impl Ord for Version {
    fn cmp<'a>(&'a self, other: &'a Self) -> Ordering {
        if self.base() != other.base() {
            // compare major versions
            cmp_not_equal!(&self.numbers[0].1, &other.numbers[0].1);

            // iterate through the remaining version components
            for ((v1, n1), (v2, n2)) in zip(&self.numbers[1..], &other.numbers[1..]) {
                // if string is lexically equal, it is numerically equal too
                if v1 == v2 {
                    continue;
                }

                // If one of the components starts with a "0" then they are compared as strings
                // with trailing 0's stripped, otherwise they are compared as integers.
                if v1.starts_with('0') || v2.starts_with('0') {
                    cmp_not_equal!(v1.trim_end_matches('0'), v2.trim_end_matches('0'));
                } else {
                    cmp_not_equal!(&n1, &n2);
                }
            }

            // compare the number of version components
            cmp_not_equal!(&self.numbers.len(), &other.numbers.len());

            // dotted components were equal so compare letter suffixes
            cmp_not_equal!(&self.letter, &other.letter);

            for ((s1, n1), (s2, n2)) in zip(&self.suffixes, &other.suffixes) {
                // if suffixes differ, use them for comparison
                cmp_not_equal!(s1, s2);
                // otherwise use the suffix versions for comparison
                cmp_not_equal!(n1, n2);
            }

            // If one version has more suffixes, use the last suffix to determine ordering.
            match self.suffixes.cmp(&other.suffixes) {
                Ordering::Equal => (),
                Ordering::Greater => match self.suffixes.last().unwrap().0 {
                    Suffix::P => return Ordering::Greater,
                    _ => return Ordering::Less,
                },
                Ordering::Less => match other.suffixes.last().unwrap().0 {
                    Suffix::P => return Ordering::Less,
                    _ => return Ordering::Greater,
                },
            }
        }

        // finally compare the revisions
        self.revision.cmp(&other.revision)
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
    fn from_str(s: &str) -> Result<Self> {
        parse::version(s)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::*;
    use crate::macros::*;
    use crate::test::VER_CMP_DATA;
    use crate::Error;

    #[test]
    fn test_from_str() {
        for s in ["0", "0-r0", "1.0-r01", "1_alpha5-r1", "1.001.100r_beta1_p2"] {
            let ver = Version::from_str(s).unwrap();
            assert_eq!(format!("{ver}"), s);
        }
    }

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
                .iter()
                .cloned()
                .collect();

        for expr in VER_CMP_DATA {
            let v: Vec<&str> = expr.split(' ').collect();
            let v1 = Version::from_str(v[0]).unwrap();
            let v2 = Version::from_str(v[2]).unwrap();
            let op = v[1];
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
        for (unsorted, expected) in [
            // all equal versions shouldn't be sorted
            ("0 00 0-r0 0-r00", "0 00 0-r0 0-r00"),
            ("1.0.2 1.0.2-r0 1.000.2", "1.0.2 1.0.2-r0 1.000.2"),
            // simple versions
            ("3 2 1 0", "0 1 2 3"),
            ("1.100 1.10 1.1", "1.1 1.10 1.100"),
            // letter suffixes
            ("1z 1y 1b 1a", "1a 1b 1y 1z"),
            // release suffixes
            ("1_p 1_rc 1_pre 1_beta 1_alpha", "1_alpha 1_beta 1_pre 1_rc 1_p"),
            ("1_p2 1_p1 1_p0", "1_p0 1_p1 1_p2"),
            // revisions
            ("1-r2 1-r1 1-r0", "1-r0 1-r1 1-r2"),
        ] {
            let mut versions: Vec<Version> = unsorted
                .split(' ')
                .map(|s| Version::from_str(s).unwrap())
                .collect();
            versions.sort();
            let sorted: Vec<String> = versions.iter().map(|v| format!("{v}")).collect();
            assert_eq!(sorted.join(" "), expected);
        }
    }
}
