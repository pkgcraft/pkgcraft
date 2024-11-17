use pkgcraft::restrict::dep::Restrict as DepRestrict;
use pkgcraft::restrict::Restrict;
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

impl From<&Restrict> for Scope {
    fn from(value: &Restrict) -> Self {
        use DepRestrict::{Category, Package, Version};

        let restrict_scope = |restrict: &Restrict| match restrict {
            Restrict::Dep(Category(_)) => Scope::Category,
            Restrict::Dep(Package(_)) => Scope::Package,
            Restrict::Dep(Version(Some(_))) => Scope::Version,
            _ => Scope::Repo,
        };

        match value {
            Restrict::And(vals) => vals
                .iter()
                .map(|x| restrict_scope(x))
                .min()
                .unwrap_or(Scope::Repo),
            Restrict::Or(vals) => vals
                .iter()
                .map(|x| restrict_scope(x))
                .max()
                .unwrap_or(Scope::Repo),
            _ => restrict_scope(value),
        }
    }
}
