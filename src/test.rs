use std::fs;
use std::path::PathBuf;

use itertools::Itertools;
use once_cell::sync::Lazy;
use serde::Deserialize;

use crate::macros::build_from_paths;
use crate::{Error, Result};

static TEST_DATA_DIR: Lazy<PathBuf> =
    Lazy::new(|| build_from_paths!(env!("CARGO_MANIFEST_DIR"), "tests"));

#[derive(Debug, Deserialize)]
pub(crate) struct TestData {
    pub(crate) ver_cmp: Vec<String>,
}

impl TestData {
    pub(crate) fn load() -> Result<Self> {
        let path = TEST_DATA_DIR.join("cmp-data.toml");
        let data = fs::read_to_string(&path)
            .map_err(|e| Error::IO(format!("failed loading test data: {path:?}: {e}")))?;
        toml::from_str(&data)
            .map_err(|e| Error::IO(format!("invalid test data format: {path:?}: {e}")))
    }

    pub(crate) fn ver_cmp(&self) -> CmpIter {
        CmpIter {
            iter: self.ver_cmp.iter(),
        }
    }
}

pub(crate) struct CmpIter<'a> {
    iter: std::slice::Iter<'a, String>,
}

impl<'a> Iterator for CmpIter<'a> {
    type Item = (&'a str, (&'a str, &'a str, &'a str));

    fn next(&mut self) -> Option<Self::Item> {
        // forcibly panic for wrong data format
        self.iter
            .next()
            .map(|s| (s.as_str(), s.split(' ').collect_tuple().unwrap()))
    }
}
