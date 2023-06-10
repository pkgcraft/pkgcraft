use strum::{Display, EnumString};

#[derive(Display, EnumString, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
#[strum(serialize_all = "kebab-case")]
pub enum CacheFormat {
    Md5Dict,
}
