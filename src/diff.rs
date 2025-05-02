use crate::cache;
use crate::files;

use std::io;
use std::io::Write;
use std::collections::{HashMap, HashSet};

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
            println!("Could not load cache: {}, continuing with empty cache",
                e);
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

    print!("Comparing hashes... ");
    io::stdout().flush().unwrap();
    let mut hashes: HashMap::<[u8;16], HashSet<&files::File>> = HashMap::new();
    for file in files.iter() {
        let hash = file.hash().clone();
        if let Some(set) = hashes.get_mut(&hash) {
            set.insert(file);
        } else {
            let mut set = HashSet::new();
            set.insert(file);
            hashes.insert(hash, set);
        }
    }
    let hashes = hashes; // Remove mut
    if hashes.len() == files.len() {
        println!("all uniques");
    } else {
        println!("{} unique files ({})",
            hashes.len(), hashes.len() as isize - files.len() as isize);
    }

    println!("Compiling results...");
    let mut identicals: HashSet<Vec<&files::File>>= HashSet::new();
    for identityset in hashes.values() {
        if 1 < identityset.len() {
            let mut identical_files: Vec<&files::File> =
                identityset.iter()
                // Iter<&files::File>
                .cloned()
                .collect();
            identical_files.sort();
            identicals.insert(identical_files);
        }
    }
    let identicals = identicals; // Remove mut

    println!();
    for identityset in identicals {
        print!("identical:");
        for file in identityset {
            print!(" {}", file.name());
        }
        println!()
    }
}
