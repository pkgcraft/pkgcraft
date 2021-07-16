use crate::atom::Atom;

pub mod license;
pub mod pkgdep;
pub mod required_use;

#[derive(Debug, PartialEq)]
pub enum DepSpec {
    Strings(Vec<String>),
    Atoms(Vec<Atom>),
    AllOf(Box<DepSpec>),
    AnyOf(Box<DepSpec>),
    ExactlyOneOf(Box<DepSpec>), // REQUIRED_USE only
    AtMostOneOf(Box<DepSpec>), // REQUIRED_USE only
    ConditionalUse(String, Box<DepSpec>),
}
