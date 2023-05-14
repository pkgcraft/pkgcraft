use strum::{Display, EnumString};

pub(crate) mod ebuild;

#[derive(Display, EnumString, Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[strum(serialize_all = "snake_case")]
pub(crate) enum Operation {
    Pretend,
    Build,
    Install,
    Uninstall,
    Replace,
    Config,
    Info,
    NoFetch,
}
