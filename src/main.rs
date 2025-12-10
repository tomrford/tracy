use clap::Parser;
use std::fs;
use std::process::ExitCode;

use tracy::args::Args;
use tracy::error::TracyError;
use tracy::filter::collect_files;
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
    if files.is_empty() {
        return Err(TracyError::NoFiles);
    }

    let matches = scan_files(&args.root, &files, &args.scan)?;
    if matches.is_empty() {
        return Err(TracyError::NoResults);
    }

    let output = serde_json::to_string_pretty(&matches)?;

    if !args.quiet {
        println!("{output}");
    }

    if let Some(path) = &args.output {
        fs::write(path, &output)?;
    }

    Ok(())
}
