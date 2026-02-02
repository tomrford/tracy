use crate::config::Config;
use crate::error::TracyError;
use crate::filter::FilterArgs;
use crate::output::OutputFormat;
use crate::scan::ScanArgs;
use clap::Parser;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Scan codebases for requirement references in comments and output results"
)]
pub struct Args {
    #[arg(long, help = "Root directory to scan (default: config dir or '.')")]
    pub root: Option<PathBuf>,

    #[arg(long, value_enum, help = "Output format")]
    pub format: Option<OutputFormat>,

    #[arg(long, value_name = "PATH", help = "Path to config file (default: search for tracy.toml)")]
    pub config: Option<PathBuf>,

    #[arg(long, help = "Disable config file loading")]
    pub no_config: bool,

    #[arg(short, long, help = "Write output to file (in addition to stdout)")]
    pub output: Option<PathBuf>,

    #[arg(short, long, help = "Suppress stdout output")]
    pub quiet: bool,

    #[arg(long, help = "Exit with error if no matches found")]
    pub fail_on_empty: bool,

    #[arg(long, help = "Include git repository metadata in output")]
    pub include_git_meta: bool,

    #[arg(long, help = "Include git blame metadata for each match")]
    pub include_blame: bool,

    #[command(flatten)]
    pub filter: FilterArgs,

    #[command(flatten)]
    pub scan: ScanArgs,
}

#[derive(Debug)]
pub struct ResolvedArgs {
    pub root: PathBuf,
    pub format: OutputFormat,
    pub output: Option<PathBuf>,
    pub quiet: bool,
    pub fail_on_empty: bool,
    pub include_git_meta: bool,
    pub include_blame: bool,
    pub filter: FilterArgs,
    pub scan: ScanArgs,
}

pub fn resolve_args(
    cli: Args,
    config: Option<Config>,
    config_dir: Option<&Path>,
) -> Result<ResolvedArgs, TracyError> {
    let config = config.unwrap_or_default();

    let base_dir = config_dir.unwrap_or_else(|| Path::new("."));

    let root = match (cli.root, config.root) {
        (Some(root), _) => root,
        (None, Some(root)) => resolve_path(base_dir, root),
        (None, None) => config_dir.map(|d| d.to_path_buf()).unwrap_or_else(|| PathBuf::from(".")),
    };

    let format = cli
        .format
        .or(config.format)
        .unwrap_or(OutputFormat::Json);

    let output = match (cli.output, config.output) {
        (Some(output), _) => Some(output),
        (None, Some(output)) => Some(resolve_path(base_dir, output)),
        (None, None) => None,
    };

    let quiet = cli.quiet || config.quiet.unwrap_or(false);
    let fail_on_empty = cli.fail_on_empty || config.fail_on_empty.unwrap_or(false);
    let include_git_meta = cli.include_git_meta || config.include_git_meta.unwrap_or(false);
    let include_blame = cli.include_blame || config.include_blame.unwrap_or(false);

    let include = if !cli.filter.include.is_empty() {
        cli.filter.include
    } else {
        config.filter.include.unwrap_or_default()
    };
    let exclude = if !cli.filter.exclude.is_empty() {
        cli.filter.exclude
    } else {
        config.filter.exclude.unwrap_or_default()
    };

    let filter = FilterArgs {
        include_vendored: cli.filter.include_vendored || config.filter.include_vendored.unwrap_or(false),
        include_generated: cli.filter.include_generated || config.filter.include_generated.unwrap_or(false),
        include_submodules: cli.filter.include_submodules || config.filter.include_submodules.unwrap_or(false),
        include,
        exclude,
    };

    let slug = if !cli.scan.slug.is_empty() {
        cli.scan.slug
    } else {
        config.scan.slug.unwrap_or_default()
    };

    if slug.is_empty() {
        return Err(TracyError::NoSlugs);
    }

    Ok(ResolvedArgs {
        root,
        format,
        output,
        quiet,
        fail_on_empty,
        include_git_meta,
        include_blame,
        filter,
        scan: ScanArgs { slug },
    })
}

fn resolve_path(base_dir: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        base_dir.join(path)
    }
}
