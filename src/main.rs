use clap::Parser;
use std::fs;
use std::process::ExitCode;

use tracy::args::Args;
use tracy::error::TracyError;
use tracy::filter::collect_files;
use tracy::git::collect_git_meta;
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
    let args = Args::parse();

    let files = collect_files(&args.root, &args.filter)?;
    let matches = scan_files(&args.root, &files, &args.scan)?;

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
