use clap::Args;

#[derive(Debug, Args)]
pub struct ScanArgs {
    #[arg(
        long,
        short = 's',
        required = true,
        help = "Slug pattern to search for (e.g., 'REQ' matches 'REQ-123'). Can be repeated."
    )]
    pub slug: Vec<String>,
}
