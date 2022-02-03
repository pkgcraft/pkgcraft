use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use super::{cmp_not_equal, parse};
use crate::error::Error;

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

    fn from_str(s: &str) -> Result<Suffix, Self::Err> {
        match s {
            "alpha" => Ok(Suffix::Alpha),
            "beta" => Ok(Suffix::Beta),
            "pre" => Ok(Suffix::Pre),
            "rc" => Ok(Suffix::Rc),
            "p" => Ok(Suffix::P),
            _ => Err(Error::InvalidValue(format!("invalid suffix: {}", s))),
        }
    }
}

#[derive(Debug, Default, Eq, Clone)]
pub(crate) struct Revision {
    pub(crate) value: Option<String>,
    int: u64,
}

impl FromStr for Revision {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let int: u64 = s
            .parse()
            .map_err(|e| Error::InvalidValue(format!("invalid revision {:?}: {}", s, e)))?;
        Ok(Revision {
            value: Some(s.to_string()),
            int,
        })
    }
}

impl Revision {
    pub(crate) fn new(rev: Option<&str>) -> crate::Result<Self> {
        match &rev {
            Some(s) => Revision::from_str(s),
            None => Ok(Revision::default()),
        }
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
        match &self.value {
            Some(s) => write!(f, "-r{}", s),
            None => Ok(()),
        }
    }
}

#[derive(Debug)]
pub(crate) struct ParsedVersion<'a> {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) numbers: Vec<&'a str>,
    pub(crate) letter: Option<char>,
    pub(crate) suffixes: Option<Vec<(&'a str, Option<&'a str>)>>,
    pub(crate) revision: Option<&'a str>,
}

impl ParsedVersion<'_> {
    pub(crate) fn into_owned(self, s: &str) -> crate::Result<Version> {
        let mut numbers: Vec<(String, u64)> = vec![];
        for s in self.numbers.iter() {
            let num = s
                .parse()
                .map_err(|e| Error::InvalidValue(format!("invalid version: {}: {}", e, s)))?;
            numbers.push((s.to_string(), num));
        }

        let mut suffixes: Vec<(Suffix, Option<u64>)> = vec![];
        if let Some(vals) = self.suffixes {
            for (s, v) in vals.iter() {
                let suffix = Suffix::from_str(s)?;
                let num = match v {
                    None => None,
                    Some(x) => Some(x.parse().map_err(|e| {
                        Error::InvalidValue(format!("invalid version: {}: {}", e, s))
                    })?),
                };
                suffixes.push((suffix, num));
            }
        }

        Ok(Version {
            base: s[self.start..self.end].to_string(),
            numbers,
            letter: self.letter,
            suffixes,
            revision: Revision::new(self.revision)?,
        })
    }
}

#[derive(Debug, Eq, Clone)]
pub struct Version {
    base: String,
    numbers: Vec<(String, u64)>,
    letter: Option<char>,
    suffixes: Vec<(Suffix, Option<u64>)>,
    revision: Revision,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}", self.base, self.revision)
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Ord for Version {
    fn cmp<'a>(&'a self, other: &'a Self) -> Ordering {
        if self.base != other.base {
            // compare major versions
            cmp_not_equal!(self.numbers[0].1.cmp(&other.numbers[0].1));

            // iterate through the remaining version components
            for ((v1, n1), (v2, n2)) in self.numbers[1..].iter().zip(other.numbers[1..].iter()) {
                // if string is lexically equal, it is numerically equal too
                if v1 == v2 {
                    continue;
                }

                // If one of the components starts with a "0" then they are compared as strings
                // with trailing 0's stripped, otherwise they are compared as integers.
                if v1.starts_with('0') || v2.starts_with('0') {
                    cmp_not_equal!(v1.trim_end_matches('0').cmp(v2.trim_end_matches('0')));
                } else {
                    cmp_not_equal!(n1.cmp(n2));
                }
            }

            // compare the number of version components
            cmp_not_equal!(self.numbers.len().cmp(&other.numbers.len()));

            // dotted components were equal so compare letter suffixes
            cmp_not_equal!(self.letter.cmp(&other.letter));

            for ((s1, n1), (s2, n2)) in self.suffixes.iter().zip(other.suffixes.iter()) {
                // if suffixes differ, use them for comparison
                cmp_not_equal!(s1.cmp(s2));
                // otherwise use the suffix versions for comparison
                cmp_not_equal!(n1.cmp(n2));
            }

            // One version has more suffixes than the other, use its last
            // suffix to determine ordering.
            match self.suffixes.len().cmp(&other.suffixes.len()) {
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
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse::version(s)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn test_from_str() {
        for s in ["0", "0-r0", "1_alpha5-r1", "1.001.100r_beta1_p2"] {
            let ver = Version::from_str(s).unwrap();
            assert_eq!(format!("{}", ver), s);
        }
    }

    #[test]
    #[should_panic(expected = "invalid version")]
    fn test_overflow_major_version() {
        let val: u128 = u64::MAX as u128;
        let v1 = Version::from_str(&format!("{}", val)).unwrap();
        let v2 = Version::from_str(&format!("{}", val + 1)).unwrap();
        assert!(v1 != v2);
    }

    #[test]
    #[should_panic(expected = "invalid version")]
    fn test_overflow_version_component() {
        let val: u128 = u64::MAX as u128;
        let v1 = Version::from_str(&format!("1.{}", val)).unwrap();
        let v2 = Version::from_str(&format!("1.{}", val + 1)).unwrap();
        assert!(v1 != v2);
    }

    #[test]
    fn test_cmp() {
        let op_map: HashMap<&str, Ordering> = [
            ("<", Ordering::Less),
            ("==", Ordering::Equal),
            (">", Ordering::Greater),
        ]
        .iter()
        .cloned()
        .collect();

        for expr in [
            // simple major versions
            ("0 == 0"),
            ("0 != 1"),
            // equal due to integer coercion and "-r0" being the revision default
            ("0 == 0-r0"),
            ("1 == 01"),
            ("01 == 001"),
            ("1.00 == 1.0"),
            ("1.0100 == 1.010"),
            ("01.01 == 1.01"),
            ("0001.1 == 1.1"),
            ("1.2 == 001.2"),
            ("1.0.2 == 1.0.2-r0"),
            ("1.0.2-r0 == 1.000.2"),
            ("1.000.2 == 1.00.2-r0"),
            ("0-r0 == 0-r00"),
            ("0_beta01 == 0_beta001"),
            ("1.2_pre08-r09 == 1.2_pre8-r9"),
            ("1.010.02 != 1.01.2"),
            // minor versions
            ("0.1 < 0.11"),
            ("0.01 > 0.001"),
            ("1.0 > 1"),
            ("1.0_alpha > 1_alpha"),
            ("1.0_alpha > 1"),
            ("1.0_alpha < 1.0"),
            // version letter suffix
            ("0a < 0b"),
            ("1.1z > 1.1a"),
            // release types
            ("1_alpha < 1_beta"),
            ("1_beta < 1_pre"),
            ("1_pre < 1_rc"),
            ("1_rc < 1"),
            ("1 < 1_p"),
            // release suffix vs non-suffix
            ("1.2.3_alpha < 1.2.3"),
            ("1.2.3_beta < 1.2.3"),
            ("1.2.3_pre < 1.2.3"),
            ("1.2.3_rc < 1.2.3"),
            ("1.2.3_p > 1.2.3"),
            // release suffix version
            ("0_alpha1 < 0_alpha2"),
            ("0_alpha2-r1 > 0_alpha1-r2"),
            ("0_p1 < 0_p2"),
            // last release suffix
            ("0_alpha_rc_p > 0_alpha_rc"),
            // revision
            ("0-r2 > 0-r1"),
            ("1.0.2_pre01-r2 > 1.00.2_pre001-r1"),
            // bound limits
            (&format!("{} < {}", u32::MAX, u64::MAX)),
        ] {
            let v: Vec<&str> = expr.split(' ').collect();
            let v1 = Version::from_str(v[0]).unwrap();
            let v2 = Version::from_str(v[2]).unwrap();
            let op = v[1];
            match op {
                "!=" => {
                    assert_ne!(v1, v2, "failed comparing {}", expr);
                    assert_ne!(v2, v1, "failed comparing {}", expr);
                }
                _ => {
                    let op = op_map[op];
                    assert_eq!(v1.cmp(&v2), op, "failed comparing {}", expr);
                    assert_eq!(v2.cmp(&v1), op.reverse(), "failed comparing {}", expr);
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
            (
                "1_p 1_rc 1_pre 1_beta 1_alpha",
                "1_alpha 1_beta 1_pre 1_rc 1_p",
            ),
            ("1_p2 1_p1 1_p0", "1_p0 1_p1 1_p2"),
            // revisions
            ("1-r2 1-r1 1-r0", "1-r0 1-r1 1-r2"),
        ] {
            let mut versions: Vec<Version> = unsorted
                .split(' ')
                .map(|s| Version::from_str(s).unwrap())
                .collect();
            versions.sort();
            let sorted: Vec<String> = versions.iter().map(|v| format!("{}", v)).collect();
            assert_eq!(sorted.join(" "), expected);
        }
    }
}
