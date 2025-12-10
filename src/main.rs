mod error;
mod filter;

use error::TracyError;
use filter::FilterArgs;
use std::path::PathBuf;

fn main() -> Result<(), TracyError> {
    let root = PathBuf::from(".");
    let args = FilterArgs::default();

    let files = filter::collect_files(&root, &args)?;

    for file in files {
        println!("{}", file.display());
    }

    Ok(())
}
