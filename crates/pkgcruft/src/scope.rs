use strum::{AsRefStr, EnumIter, EnumString};

#[derive(
    AsRefStr,
    EnumIter,
    EnumString,
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
