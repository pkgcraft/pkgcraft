use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;

use crate::repo;

#[derive(Debug, PartialEq)]
pub struct Repo<'a> {
    pub id: &'a str,
    pub path: &'a str,
    categories: Option<HashSet<&'a str>>,
    packages: Option<HashMap<&'a str, &'a str>>,
    versions: Option<HashMap<(&'a str, &'a str), Vec<&'a str>>>,
}

impl Repo<'_> {
    pub fn new<'a>(id: &'a str, path: &'a str) -> Result<Repo<'a>, Box<dyn Error>> {
        Ok(Repo {
            id: id,
            path: path,
            categories: None,
            packages: None,
            versions: None,
        })
    }
}

impl fmt::Display for Repo<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.id, self.path)
    }
}

//impl Iterator for Repo {
    //type Item = Package;

    //fn next(&mut self) -> Option<Self::Item> {
        //None
    //}
//}

impl repo::Repo for Repo<'_> {
    fn categories(&mut self) -> &HashSet<&str> {
        if self.categories.is_none() {
            self.categories = Some(["cata", "catb"].iter().cloned().collect::<HashSet<&str>>());
        }
        self.categories.as_ref().unwrap()
    }

    fn packages(&mut self) -> &HashMap<&str, &str> {
        if self.packages.is_none() {
            self.packages = Some([("cata", "pkga"), ("catb", "pkgb")].iter().cloned().collect::<HashMap<&str, &str>>());
        }
        self.packages.as_ref().unwrap()
    }

    fn versions(&mut self) -> &HashMap<(&str, &str), Vec<&str>> {
        if self.versions.is_none() {
            self.versions = Some([(("cata", "pkga"), vec!["0"]), (("catb", "pkgb"), vec!["1"])].iter().cloned().collect::<HashMap<(&str, &str), Vec<&str>>>());
        }
        self.versions.as_ref().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::repo::Repo;

    //#[test]
    //fn test_categories() {
        //let mut repo = super::Repo::new("id", "path").unwrap();
        //assert!(repo.categories().contains("cata"));
        //assert!(repo.categories().contains("catb"));
    //}
}
