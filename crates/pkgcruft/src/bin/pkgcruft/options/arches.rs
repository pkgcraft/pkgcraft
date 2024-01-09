use clap::Args;

#[derive(Debug, Args)]
#[clap(next_help_heading = Some("Arches"))]
pub(crate) struct Arches {
    /// Specific arches to target
    #[arg(short, long)]
    pub(crate) arches: Vec<String>,
}
