use strum::{AsRefStr, Display, EnumIter, EnumString, VariantNames};

#[derive(
    AsRefStr,
    Display,
    EnumIter,
    EnumString,
    VariantNames,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Copy,
    Clone,
)]
pub enum Scope {
    #[default]
    Version,
    Package,
}
