use std::cmp::{Ordering, min};
use std::fmt;
use std::str::FromStr;

use once_cell::sync::Lazy;
use regex::Regex;

use crate::utils::rstrip;
use super::parser::ParseError;
use super::parser::pkg::version as parse;

static SUFFIX_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new("^(?P<suffix>alpha|beta|pre|rc|p)(?P<version>\\d*)$").unwrap()
});

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Suffix {
    Alpha, // _alpha
    Beta, // _beta
    Pre, // _pre
    Rc, // _rc
    P, // _p
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

#[derive(Debug, Eq, Hash)]
pub struct Revision {
    pub value: String
}

impl PartialEq for Revision {
    fn eq(&self, other: &Self) -> bool {
        // parsing shouldn't fail when using grammar-parsed values
        let rev_num: u32 = self.value.parse().unwrap();
        let other_num: u32 = other.value.parse().unwrap();
        rev_num == other_num
    }
}

impl PartialEq<str> for Revision {
    fn eq(&self, other: &str) -> bool {
        self.value == other
    }
}

impl PartialOrd for Revision {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // parsing shouldn't fail when using grammar-parsed values
        let rev_num: u32 = self.value.parse().unwrap();
        let other_num: u32 = other.value.parse().unwrap();
        rev_num.partial_cmp(&other_num)
    }
}

impl Ord for Revision {
    fn cmp(&self, other: &Self) -> Ordering {
        // parsing shouldn't fail when using grammar-parsed values
        let rev_num: u32 = self.value.parse().unwrap();
        let other_num: u32 = other.value.parse().unwrap();
        rev_num.cmp(&other_num)
    }
}

impl fmt::Display for Revision {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Version {
    pub base: String,
    pub revision: Option<Revision>,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut s = self.base.clone();
        if let Some(rev) = &self.revision {
            s.push_str(&format!("-r{}", rev));
        }
        write!(f, "{}", s)
    }
}

// compare two optional revisions
fn rev_cmp(rev1: &Option<Revision>, rev2: &Option<Revision>) -> Option<Ordering> {
    match (&rev1, &rev2) {
        (Some(r1), Some(r2)) => r1.partial_cmp(&r2),
        (None, Some(r2)) => {
            if r2 == "0" {
                Some(Ordering::Equal)
            } else {
                Some(Ordering::Less)
            }
        },
        (Some(r1), None) => {
            if r1 == "0" {
                Some(Ordering::Equal)
            } else {
                Some(Ordering::Greater)
            }
        },
        (None, None) => Some(Ordering::Equal),
    }
}

impl PartialOrd for Version {
    fn partial_cmp<'a>(&'a self, other: &'a Self) -> Option<Ordering> {
        let mut cmp: Option<Ordering>;

        // if versions are equal, comparing revisions suffices
        if self.base == other.base {
            return rev_cmp(&self.revision, &other.revision);
        }

        // split versions into dotted strings and lists of suffixes
        let self_parts: Vec<&str> = self.base.split("_").collect();
        let other_parts: Vec<&str> = other.base.split("_").collect();

        // if dotted strings differ, then perform comparisons on them
        if self_parts[0] != other_parts[0] {
            // split dotted strings into components
            let self_ver_parts: Vec<&str> = self_parts[0].split(".").collect();
            let other_ver_parts: Vec<&str> = other_parts[0].split(".").collect();

            // iterate through the components
            for (v1, v2) in self_ver_parts.iter().zip(other_ver_parts.iter()) {
                // if string is lexically equal, it is numerically equal too
                if v1 == v2 {
                    continue;
                }

                // If one of the components begins with a "0" then they are compared as integers so
                // that 1.1 > 1.02; otherwise they are compared as strings. Note that we can use
                // byte-slicing since version strings are guaranteed to use ASCII characters.
                match (&v1[..1], &v2[..1]) {
                    ("0", _) | (_, "0") => {
                        let v1_stripped = rstrip(v1, '0');
                        let v2_stripped = rstrip(v2, '0');
                        cmp = v1_stripped.partial_cmp(&v2_stripped);
                        if cmp != Some(Ordering::Equal) {
                            return cmp;
                        }
                    },
                    _ => {
                        let v1_int: u32 = v1.parse().unwrap();
                        let v2_int: u32 = v2.parse().unwrap();
                        cmp = v1_int.partial_cmp(&v2_int);
                        if cmp != Some(Ordering::Equal) {
                            return cmp;
                        }
                    },
                }
            }

            cmp = self_ver_parts.len().partial_cmp(&other_ver_parts.len());
            if cmp != Some(Ordering::Equal) {
                return cmp;
            }

            // get the last character from the last string
            let last = |parts: &Vec<&'a str>| {
                let s: &'a str = parts.last().unwrap();
                match s.chars().last().unwrap() {
                    'a'..='z' => s,
                    _ => "",
                }
            };

            // dotted components were equal so compare single letter suffixes
            cmp = last(&self_ver_parts).partial_cmp(&last(&other_ver_parts));
            if cmp != Some(Ordering::Equal) {
                return cmp;
            }
        }

        let self_suffixes = &self_parts[1..];
        let self_suffixes_len = self_suffixes.len();
        let other_suffixes = &other_parts[1..];
        let other_suffixes_len = other_suffixes.len();
        let suffix_range = min(self_suffixes_len, other_suffixes_len);

        if suffix_range >= 1 {
            for x in 0..suffix_range {
                // if the strings are equal, continue to the next
                if self_suffixes[x] == other_suffixes[x] {
                    continue;
                }

                // use regex to split suffixes from versions
                let m1 = SUFFIX_REGEX.captures(self_suffixes[x]).unwrap();
                let m2 = SUFFIX_REGEX.captures(other_suffixes[x]).unwrap();
                let s1 = Suffix::from_str(m1.name("suffix").unwrap().as_str()).unwrap();
                let s2 = Suffix::from_str(m2.name("suffix").unwrap().as_str()).unwrap();

                // if suffixes differ, use them for comparison
                cmp = s1.partial_cmp(&s2);
                if cmp != Some(Ordering::Equal) {
                    return cmp;
                }

                // otherwise use the suffix versions for comparison
                let v1: u32 = m1.name("version").unwrap().as_str().parse().unwrap_or_default();
                let v2: u32 = m2.name("version").unwrap().as_str().parse().unwrap_or_default();
                cmp = v1.partial_cmp(&v2);
                if cmp != Some(Ordering::Equal) {
                    return cmp;
                }
            }

            // One version has more suffixes than the other, use its last
            // suffix to determine ordering.
            if self_suffixes_len > other_suffixes_len {
                let m = SUFFIX_REGEX.captures(self_suffixes[self_suffixes_len - 1]).unwrap();
                match m.name("suffix").unwrap().as_str() {
                    "_p" => return Some(Ordering::Greater),
                    _ => return Some(Ordering::Less),
                }
            } else if self_suffixes_len < other_suffixes_len {
                let m = SUFFIX_REGEX.captures(other_suffixes[other_suffixes_len - 1]).unwrap();
                match m.name("suffix").unwrap().as_str() {
                    "_p" => return Some(Ordering::Less),
                    _ => return Some(Ordering::Greater),
                }
            }
        }

        // finally compare the revisions
        return rev_cmp(&self.revision, &other.revision);
    }
}

impl FromStr for Version {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (ver, rev) = parse(s)?;
        Ok(Version {
            base: ver.to_string(),
            revision: rev.and_then(|s| Some(Revision {value: s.to_string()})),
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
    fn test_ordering() {
        let op_map: HashMap<&str, Ordering> = [
            ("<", Ordering::Less),
            ("=", Ordering::Equal),
            (">", Ordering::Greater),
        ].iter().cloned().collect();

        for expr in [
                ("0 = 0"),
                ("0 = 0-r0"),
                ("0-r0 = 0"),
                ("1.0.2 = 1.0.2-r0"),
                ("1.0.2-r0 = 1.000.2"),
                ("1.000.2 = 1.00.2-r0"),
                ("0-r0 = 0-r00"),
                ("0.1 < 0.11"),
                ("0.01 > 0.001"),
                ("0_alpha1 < 0_alpha2"),
                ("0_alpha2-r1 > 0_alpha1-r2"),
                ("0_beta01 = 0_beta001"),
                ] {
            let v: Vec<&str> = expr.split(" ").collect();
            let (v1, op, v2) = (v[0], v[1], v[2]);
            let ver1 = Version::from_str(v1).unwrap();
            let ver2 = Version::from_str(v2).unwrap();
            assert_eq!(
                ver1.partial_cmp(&ver2), Some(op_map[op]),
                "failed comparing {}", expr);
        }
    }
}
