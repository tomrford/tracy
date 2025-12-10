use crate::filter::FilterArgs;
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

    #[arg(short, long, help = "Write output to file (in addition to stdout)")]
    pub output: Option<PathBuf>,

    #[arg(short, long, help = "Suppress output")]
    pub quiet: bool,

    #[command(flatten)]
    pub filter: FilterArgs,

    #[command(flatten)]
    pub scan: ScanArgs,
}
