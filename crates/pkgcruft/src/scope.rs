use strum::{AsRefStr, Display, EnumIter, EnumString, VariantNames};

#[derive(
    AsRefStr,
    Display,
    EnumIter,
    EnumString,
    VariantNames,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Copy,
    Clone,
)]
#[strum(serialize_all = "kebab-case")]
pub enum Scope {
    Version,
    Package,
    Category,
    Repo,
}
