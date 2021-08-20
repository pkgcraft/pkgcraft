use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use once_cell::sync::Lazy;
use regex::Regex;

use super::parse;
use crate::error::Error;
use crate::utils::rstrip;

static SUFFIX_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new("^(?P<suffix>alpha|beta|pre|rc|p)(?P<version>\\d*)$").unwrap());

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Suffix {
    Alpha, // _alpha
    Beta,  // _beta
    Pre,   // _pre
    Rc,    // _rc
    P,     // _p
}

impl FromStr for Suffix {
    type Err = ();

    fn from_str(s: &str) -> Result<Suffix, Self::Err> {
        match s {
            "alpha" => Ok(Suffix::Alpha),
            "beta" => Ok(Suffix::Beta),
            "pre" => Ok(Suffix::Pre),
            "rc" => Ok(Suffix::Rc),
            "p" => Ok(Suffix::P),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Default, Eq)]
pub struct Revision {
    pub value: Option<String>,
    int: u32,
}

impl Revision {
    pub fn new(rev: Option<&str>) -> crate::Result<Self> {
        match &rev {
            Some(s) => {
                let int: u32 = s
                    .parse()
                    .map_err(|e| Error::InvalidValue(format!("invalid revision {:?}: {}", s, e)))?;
                Ok(Revision {
                    value: Some(s.to_string()),
                    int,
                })
            }
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

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Version {
    pub base: String,
    pub revision: Revision,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}", self.base, self.revision)
    }
}

impl Ord for Version {
    fn cmp<'a>(&'a self, other: &'a Self) -> Ordering {
        let mut cmp: Ordering;

        if self.base != other.base {
            // split versions into dotted strings and lists of suffixes
            let self_parts: Vec<&str> = self.base.split('_').collect();
            let other_parts: Vec<&str> = other.base.split('_').collect();

            // if dotted strings differ, then perform comparisons on them
            if self_parts[0] != other_parts[0] {
                // separate letter suffix from version string
                let split = |s: &'a str| -> (Option<char>, &'a str) {
                    match s.chars().last().unwrap() {
                        c @ 'a'..='z' => (Some(c), &s[..s.len() - 1]),
                        _ => (None, s),
                    }
                };

                // pull letter suffixes for later comparison
                let (self_letter, self_str) = split(self_parts[0]);
                let (other_letter, other_str) = split(other_parts[0]);
                // split dotted version string into components
                let self_ver_parts: Vec<&str> = self_str.split('.').collect();
                let other_ver_parts: Vec<&str> = other_str.split('.').collect();

                // iterate through the components
                for (v1, v2) in self_ver_parts.iter().zip(other_ver_parts.iter()) {
                    // if string is lexically equal, it is numerically equal too
                    if v1 == v2 {
                        continue;
                    }

                    // If one of the components begins with a "0" then they are compared as
                    // integers so that 1.1 > 1.02; otherwise they are compared as strings. Note
                    // that we can use byte-slicing since version strings are guaranteed to use
                    // ASCII characters.
                    match (&v1[..1], &v2[..1]) {
                        ("0", _) | (_, "0") => {
                            let v1_stripped = rstrip(v1, '0');
                            let v2_stripped = rstrip(v2, '0');
                            cmp = v1_stripped.cmp(v2_stripped);
                            if cmp != Ordering::Equal {
                                return cmp;
                            }
                        }
                        _ => {
                            let v1_int: u32 = v1.parse().unwrap();
                            let v2_int: u32 = v2.parse().unwrap();
                            cmp = v1_int.cmp(&v2_int);
                            if cmp != Ordering::Equal {
                                return cmp;
                            }
                        }
                    }
                }

                cmp = self_ver_parts.len().cmp(&other_ver_parts.len());
                if cmp != Ordering::Equal {
                    return cmp;
                }

                // dotted components were equal so compare letter suffixes
                cmp = self_letter.cmp(&other_letter);
                if cmp != Ordering::Equal {
                    return cmp;
                }
            }

            let self_suffixes = &self_parts[1..];
            let other_suffixes = &other_parts[1..];
            let mut suffix_count: u32 = 0;

            for (s1, s2) in self_suffixes.iter().zip(other_suffixes.iter()) {
                suffix_count += 1;

                // if suffix strings are equal, continue to the next
                if s1 == s2 {
                    continue;
                }

                // use regex to split suffixes from versions
                let m1 = SUFFIX_RE.captures(s1).unwrap();
                let m2 = SUFFIX_RE.captures(s2).unwrap();
                let n1 = Suffix::from_str(m1.name("suffix").unwrap().as_str()).unwrap();
                let n2 = Suffix::from_str(m2.name("suffix").unwrap().as_str()).unwrap();

                // if suffixes differ, use them for comparison
                cmp = n1.cmp(&n2);
                if cmp != Ordering::Equal {
                    return cmp;
                }

                // otherwise use the suffix versions for comparison
                let v1: u32 = m1
                    .name("version")
                    .unwrap()
                    .as_str()
                    .parse()
                    .unwrap_or_default();
                let v2: u32 = m2
                    .name("version")
                    .unwrap()
                    .as_str()
                    .parse()
                    .unwrap_or_default();
                cmp = v1.cmp(&v2);
                if cmp != Ordering::Equal {
                    return cmp;
                }
            }

            if suffix_count > 0 {
                // One version has more suffixes than the other, use its last
                // suffix to determine ordering.
                match self_suffixes.len().cmp(&other_suffixes.len()) {
                    Ordering::Equal => (),
                    Ordering::Greater => {
                        let m = SUFFIX_RE.captures(self_suffixes.last().unwrap()).unwrap();
                        match m.name("suffix").unwrap().as_str() {
                            "p" => return Ordering::Greater,
                            _ => return Ordering::Less,
                        }
                    }
                    Ordering::Less => {
                        let m = SUFFIX_RE.captures(other_suffixes.last().unwrap()).unwrap();
                        match m.name("suffix").unwrap().as_str() {
                            "p" => return Ordering::Less,
                            _ => return Ordering::Greater,
                        }
                    }
                }
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

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (ver, rev) = parse::version(s)?;
        let revision = Revision::new(rev)?;
        Ok(Version {
            base: ver.to_string(),
            revision,
        })
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
    fn test_cmp() {
        let op_map: HashMap<&str, Ordering> = [
            ("<", Ordering::Less),
            ("=", Ordering::Equal),
            (">", Ordering::Greater),
        ]
        .iter()
        .cloned()
        .collect();

        for expr in [
            ("0 = 0"),
            // equal due to integer coercion and "-r0" being the revision default
            ("0 = 0-r0"),
            ("1.0.2 = 1.0.2-r0"),
            ("1.0.2-r0 = 1.000.2"),
            ("1.000.2 = 1.00.2-r0"),
            ("0-r0 = 0-r00"),
            ("0_beta01 = 0_beta001"),
            // integer version comparison
            ("0.1 < 0.11"),
            ("0.01 > 0.001"),
            // version letter suffix
            ("0a < 0b"),
            ("1.1z > 1.1a"),
            // release suffix
            ("0_alpha < 0_beta"),
            ("0_pre < 0_rc"),
            // release suffix version
            ("0_alpha1 < 0_alpha2"),
            ("0_alpha2-r1 > 0_alpha1-r2"),
            // last release suffix
            ("0_alpha_rc_p > 0_alpha_rc"),
            // revision
            ("0-r2 > 0-r1"),
            ("1.0.2_pre01-r2 > 1.00.2_pre001-r1"),
        ] {
            let v: Vec<&str> = expr.split(' ').collect();
            let v1 = Version::from_str(v[0]).unwrap();
            let v2 = Version::from_str(v[2]).unwrap();
            let op = op_map[v[1]];
            match op {
                Ordering::Equal => {
                    assert_eq!(v1.cmp(&v2), op, "failed comparing {}", expr);
                    assert_eq!(v2.cmp(&v1), op, "failed comparing {}", expr);
                }
                Ordering::Less => {
                    assert_eq!(v1.cmp(&v2), op, "failed comparing {}", expr);
                    assert_eq!(v2.cmp(&v1), Ordering::Greater, "failed comparing {}", expr);
                }
                Ordering::Greater => {
                    assert_eq!(v1.cmp(&v2), op, "failed comparing {}", expr);
                    assert_eq!(v2.cmp(&v1), Ordering::Less, "failed comparing {}", expr);
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
