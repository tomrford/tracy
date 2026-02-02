use clap::Args;

#[derive(Debug, Default, Args)]
pub struct FilterArgs {
    #[arg(long, help = "Include vendored files")]
    pub include_vendored: bool,

    #[arg(long, help = "Include generated files")]
    pub include_generated: bool,

    #[arg(long, help = "Include submodules")]
    pub include_submodules: bool,

    #[arg(
        long,
        value_name = "GLOB",
        help = "Only include paths matching this glob (repeatable)"
    )]
    pub include: Vec<String>,

    #[arg(
        long,
        value_name = "GLOB",
        help = "Exclude paths matching this glob (repeatable)"
    )]
    pub exclude: Vec<String>,
}
