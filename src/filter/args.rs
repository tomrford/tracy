use clap::Args;

#[derive(Debug, Default, Args)]
pub struct FilterArgs {
    #[arg(long, help = "Include vendored files")]
    pub include_vendored: bool,

    #[arg(long, help = "Include generated files")]
    pub include_generated: bool,

    #[arg(long, help = "Include submodules")]
    pub include_submodules: bool,
}
