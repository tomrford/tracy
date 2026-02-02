use crate::filter::FilterArgs;
use crate::output::OutputFormat;
use crate::scan::ScanArgs;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Scan codebases for requirement references in comments and output JSON"
)]
pub struct Args {
    #[arg(long, default_value = ".", help = "Root directory to scan")]
    pub root: PathBuf,

    #[arg(
        long,
        value_enum,
        default_value_t = OutputFormat::Json,
        help = "Output format"
    )]
    pub format: OutputFormat,

    #[arg(short, long, help = "Write output to file (in addition to stdout)")]
    pub output: Option<PathBuf>,

    #[arg(short, long, help = "Suppress stdout output")]
    pub quiet: bool,

    #[arg(long, help = "Exit with error if no matches found")]
    pub fail_on_empty: bool,

    #[arg(long, help = "Include git repository metadata in output")]
    pub include_git_meta: bool,

    #[command(flatten)]
    pub filter: FilterArgs,

    #[command(flatten)]
    pub scan: ScanArgs,
}
