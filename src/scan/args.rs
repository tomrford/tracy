use clap::Args;

#[derive(Debug, Args)]
pub struct ScanArgs {
    #[arg(
        long,
        short = 's',
        help = "Slug pattern to search for (e.g., 'REQ' would match 'REQ-123')"
    )]
    pub slug: String,
}
