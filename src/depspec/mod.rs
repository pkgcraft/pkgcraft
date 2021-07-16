use crate::atom::Atom;

pub mod license;
pub mod pkgdep;
pub mod required_use;
pub mod src_uri;

#[derive(Debug, PartialEq)]
pub struct Uri {
    pub uri: String,
    pub rename: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum DepSpec {
    Strings(Vec<String>),
    Atoms(Vec<Atom>),
    Uris(Vec<Uri>),
    AllOf(Box<DepSpec>),
    AnyOf(Box<DepSpec>),
    ExactlyOneOf(Box<DepSpec>), // REQUIRED_USE only
    AtMostOneOf(Box<DepSpec>), // REQUIRED_USE only
    ConditionalUse(String, Box<DepSpec>),
}
