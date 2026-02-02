use clap::Parser;
use std::fs;
use std::process::ExitCode;

use tracy::args::Args;
use tracy::args::resolve_args;
use tracy::config::{find_config, load_config};
use tracy::error::TracyError;
use tracy::filter::collect_files;
use tracy::git::{add_blame, collect_git_meta};
use tracy::output::format_output;
use tracy::scan::scan_files;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), TracyError> {
    let cli = Args::parse();

    let cwd = std::env::current_dir()?;
    let search_start = cli.root.clone().unwrap_or_else(|| cwd.clone());

    let config_path = if cli.no_config {
        None
    } else if let Some(path) = cli.config.clone() {
        Some(if path.is_absolute() { path } else { cwd.join(path) })
    } else {
        find_config(&search_start)
    };

    let (config, config_dir) = match config_path {
        Some(path) => {
            let config = load_config(&path)?;
            let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or(cwd);
            (Some(config), Some(dir))
        }
        None => (None, None),
    };

    let args = resolve_args(cli, config, config_dir.as_deref())?;

    let files = collect_files(&args.root, &args.filter)?;
    let mut matches = scan_files(&args.root, &files, &args.scan)?;

    if args.include_blame {
        add_blame(&args.root, &mut matches)?;
    }

    if args.fail_on_empty && matches.is_empty() {
        return Err(TracyError::NoResults);
    }

    let meta = if args.include_git_meta {
        Some(collect_git_meta(&args.root)?)
    } else {
        None
    };

    let output = format_output(args.format, meta.as_ref(), &matches)?;

    if !args.quiet {
        println!("{output}");
    }

    if let Some(path) = &args.output {
        fs::write(path, &output)?;
    }

    Ok(())
}
