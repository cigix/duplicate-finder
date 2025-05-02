use crate::cache;
use crate::files;

use std::io;
use std::io::Write;
use std::collections::HashMap;

use rayon::prelude::*;
use simple_tqdm::ParTqdm;

/// The default value for [diff]'s `bits` argument.
pub const DEFAULT_BITS: usize = 0;
/// The default value for [diff]'s `parallel` argument.
pub const DEFAULT_PARALLEL: usize = 4;

/// Find and report duplicate and similar files in the current folder.
///
/// Arguments:
/// - `bits`: The bit distance in perceptual hashes to consider two images to be
///   similar. The amount of work grows exponentially with this value; `0` is a
///   good start. Default: [
/// - `parallel`: The number of parallel executions to perform the work.
pub fn diff(bits: Option<usize>, parallel: Option<usize>) -> ()
{
    let bits = bits.unwrap_or(DEFAULT_BITS);
    let parallel = parallel.unwrap_or(DEFAULT_PARALLEL);

    print!("Looking for files... ");
    io::stdout().flush().unwrap();
    let paths = files::list_files();
    println!("found {}", paths.len());

    print!("Loading cache... ");
    io::stdout().flush().unwrap();
    let cache = match cache::load_cache() {
        Ok(cache) => {
            println!("{} entries loaded", cache.len());
            cache
        }
        Err(e) => {
            println!("Could not load cache: {}", e);
            println!("Continuing with empty cache");
            HashMap::new()
        }
    };

    let config = simple_tqdm::Config::new()
        .with_desc("Processing files")
        .with_unit("files");
    let files_result: Result<Vec<files::File>, String> =
        paths.into_par_iter()
        .tqdm_config(config)
        .map(files::File::from)
        .collect();
    let files = match files_result {
        Ok(files) => files,
        Err(e) => {
            println!("Could not parse files: {}", e);
            return;
        }
    };
}
