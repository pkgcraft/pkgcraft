use std::io::{self, prelude::*};
use std::path::{Path, PathBuf};
use std::{fmt, fs};

use once_cell::sync::Lazy;
use regex::Regex;

use crate::{atom, eapi, pkg, Error, Result};

static EAPI_LINE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new("^EAPI=['\"]?(?P<EAPI>[A-Za-z0-9+_.-]*)['\"]?[\t ]*(?:#.*)?").unwrap());

#[derive(Debug)]
pub struct Pkg {
    atom: atom::Atom,
    path: PathBuf,
    eapi: &'static eapi::Eapi,
}

impl Pkg {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let eapi = Pkg::get_eapi(path)?;
        let atom = atom::parse::dep("=cat/pkg-1", eapi)?;
        Ok(Pkg {
            atom,
            path: PathBuf::from(path),
            eapi,
        })
    }

    pub fn slot(&self) -> Option<&str> {
        self.atom.slot()
    }

    pub fn subslot(&self) -> Option<&str> {
        self.atom.slot()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn get_eapi<P: AsRef<Path>>(path: P) -> Result<&'static eapi::Eapi> {
        let mut eapi = &*eapi::EAPI0;
        let path = path.as_ref();
        let f = fs::File::open(path).map_err(|e| Error::IO(e.to_string()))?;
        let reader = io::BufReader::new(f);
        for line in reader.lines() {
            let line = line.map_err(|e| Error::IO(e.to_string()))?;
            match line.chars().next() {
                None | Some('#') => continue,
                _ => {
                    if let Some(c) = EAPI_LINE_RE.captures(&line) {
                        eapi = eapi::get_eapi(c.name("EAPI").unwrap().as_str())?;
                    }
                    break;
                }
            }
        }
        Ok(eapi)
    }
}

impl fmt::Display for Pkg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.path())
    }
}

impl pkg::Pkg for Pkg {
    fn eapi(&self) -> &eapi::Eapi {
        self.eapi
    }
}
