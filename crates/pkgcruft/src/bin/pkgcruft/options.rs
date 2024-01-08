use clap::Args;

pub(crate) mod arches;
pub(crate) mod checks;
pub(crate) mod profiles;

#[derive(Debug, Args)]
pub struct Options {
    /// Specific checks to run
    #[clap(flatten)]
    pub(super) checks: checks::Options,

    #[clap(flatten)]
    arches: arches::Options,

    #[clap(flatten)]
    profiles: profiles::Options,
}
