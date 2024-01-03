use strum::{AsRefStr, EnumIter, EnumString};

use crate::check;

#[derive(
    AsRefStr, EnumIter, EnumString, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone,
)]
pub enum ReportKind {
    DeprecatedDependency,
    InvalidDependency,
    MissingMetadata,
    SourcingError,
    UnstableOnly,
}

#[allow(clippy::large_enum_variant)]
pub enum Report {
    DeprecatedDependency(check::dependency::DeprecatedDependency),
    InvalidDependency(check::metadata::InvalidDependency),
    MissingMetadata(check::metadata::MissingMetadata),
    SourcingError(check::metadata::SourcingError),
    UnstableOnly(check::unstable_only::UnstableOnly),
}

impl std::fmt::Display for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use Report::*;
        match self {
            DeprecatedDependency(r) => write!(f, "{r}"),
            InvalidDependency(r) => write!(f, "{r}"),
            MissingMetadata(r) => write!(f, "{r}"),
            SourcingError(r) => write!(f, "{r}"),
            UnstableOnly(r) => write!(f, "{r}"),
        }
    }
}
