use strum::{AsRefStr, EnumIter, EnumString};

#[derive(
    AsRefStr, EnumIter, EnumString, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone,
)]
pub enum Scope {
    Version,
    Package,
}
