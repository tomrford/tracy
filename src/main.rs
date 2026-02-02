use clap::Parser;
use serde::Serialize;
use std::fs;
use std::process::ExitCode;

use tracy::args::Args;
use tracy::error::TracyError;
use tracy::filter::collect_files;
use tracy::git::collect_git_meta;
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
    let args = Args::parse();

    let files = collect_files(&args.root, &args.filter)?;
    let matches = scan_files(&args.root, &files, &args.scan)?;

    if args.fail_on_empty && matches.is_empty() {
        return Err(TracyError::NoResults);
    }

    #[derive(Serialize)]
    struct JsonReport<'a> {
        meta: tracy::git::GitMeta,
        results: &'a tracy::scan::ScanResult,
    }

    let output = if args.include_git_meta {
        let meta = collect_git_meta(&args.root)?;
        serde_json::to_string_pretty(&JsonReport {
            meta,
            results: &matches,
        })?
    } else {
        serde_json::to_string_pretty(&matches)?
    };

    if !args.quiet {
        println!("{output}");
    }

    if let Some(path) = &args.output {
        fs::write(path, &output)?;
    }

    Ok(())
}
