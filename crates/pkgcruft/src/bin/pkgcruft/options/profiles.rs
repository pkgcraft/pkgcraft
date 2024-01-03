use clap::Args;

#[derive(Debug, Args)]
#[clap(next_help_heading = Some("Profiles"))]
pub(crate) struct Options {
    /// Specific profiles to target
    #[arg(short, long)]
    pub(crate) profiles: Vec<String>,
}
