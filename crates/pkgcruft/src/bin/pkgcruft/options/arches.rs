use clap::Args;

#[derive(Debug, Args)]
#[clap(next_help_heading = Some("Arches"))]
pub(crate) struct Options {
    /// Specific arches to target
    #[arg(short, long)]
    pub(crate) arches: Vec<String>,
}
