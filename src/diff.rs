//use crate::cache;
use crate::clusterer::Clusterer;
use crate::files;
use crate::report;

use std::hash::Hash;
use std::io;
use std::io::Write;
use std::collections::{HashMap, HashSet};

use image_hasher::ImageHash;
use itertools::Itertools;
use rayon::prelude::*;
use simple_tqdm::{Tqdm, ParTqdm};

/// The default value for [diff]'s `bits` argument.
pub const DEFAULT_BITS: usize = 0;
///// The default value for [diff]'s `parallel` argument.
//pub const DEFAULT_PARALLEL: usize = 4;

fn make_file_sets<'a, K, F>(files: &HashSet<&'a files::File>, key: F)
    -> HashMap<K, HashSet<&'a files::File>>
    where K: Clone + Eq + Hash,
          F: Fn(&files::File) -> K
{
    let mut map: HashMap<K, HashSet<&'a files::File>> = HashMap::new();
    for file in files.iter() {
        let k = key(*file);
        if let Some(set) = map.get_mut(&k) {
            set.insert(*file);
        } else {
            let mut set = HashSet::new();
            set.insert(*file);
            map.insert(k, set);
        }
    }
    map
}

/// Find and report duplicate and similar files in the current folder.
///
/// Arguments:
/// - `bits`: The bit distance in perceptual hashes to consider two images to be
///   similar. The amount of work grows exponentially with this value; `0` is a
///   good start. Default: [
///// - `parallel`: The number of parallel executions to perform the work.
pub fn diff(bits: Option<usize>/*, parallel: Option<usize>*/) -> ()
{
    let bits = bits.unwrap_or(DEFAULT_BITS) as u32;
    //let _parallel = parallel.unwrap_or(DEFAULT_PARALLEL);

    print!("Looking for files... ");
    io::stdout().flush().unwrap();
    let paths = files::list_files();
    println!("found {}", paths.len());

    //print!("Loading cache... ");
    //io::stdout().flush().unwrap();
    //let _cache = match cache::load_cache() {
    //    Ok(cache) => {
    //        println!("{} entries loaded", cache.len());
    //        cache
    //    }
    //    Err(e) => {
    //        println!("Could not load cache: {}, continuing with empty cache",
    //            e);
    //        HashMap::new()
    //    }
    //};

    let config = simple_tqdm::Config::new()
        .with_desc("Processing files")
        .with_unit("files");
    let files_result: Result<Vec<files::File>, String> =
        paths.par_iter()
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
    let fileset: HashSet<&files::File> = files.iter().collect();

    print!("Comparing hashes... ");
    io::stdout().flush().unwrap();
    let hashes = make_file_sets(&fileset, |f| f.md5.clone());
    if hashes.len() == files.len() {
        println!("all uniques");
    } else {
        println!("{} unique files ({})",
            hashes.len(), hashes.len() as isize - files.len() as isize);
    }
    let uniques: HashSet<&files::File> = hashes.values()
        // Iter<HashSet<&files::File>>
        .map(|s| {
            let mut v: Vec<&files::File> = s.iter().cloned().collect();
            v.sort();
            *(v.first().unwrap())
        })
        // Iter<&files::File>
        .collect();
    let images: HashSet<&files::File> = uniques.iter()
        .cloned()
        .filter(|f| f.ihash.is_some())
        .collect();
    println!("{} unique images", images.len());

    let ihashes = make_file_sets(&images, |f| f.ihash.clone().unwrap());
    let mut clusterer: Clusterer<ImageHash> = Clusterer::new();
    for ihash in ihashes.keys() {
        clusterer.add_single(ihash);
    }
    let config = simple_tqdm::Config::new()
        .with_desc("Comparing image hashes")
        .with_unit("ihash");
    for ihashvector in ihashes.keys()
        .combinations(2)
        .tqdm_config(config)
    {
        let ihash1 = ihashvector.get(0).unwrap();
        let ihash2 = ihashvector.get(1).unwrap();
        if ihash1.dist(ihash2) <= bits {
            clusterer.add_link(ihash1, ihash2);
        }
    }
    let mut close_images: Vec<HashSet<&files::File>> = Vec::new();
    for scc in clusterer.into_sccs() {
        let mut group: HashSet<&files::File> = HashSet::new();
        for ihash in scc {
            group.extend(ihashes.get(&ihash).unwrap())
        }
        if 1 < group.len() {
            close_images.push(group);
        }
    }
    let close_images = close_images; // Remove mut

    println!("Compiling results...");
    let identicals : HashSet<Vec<&files::File>> = hashes.values()
        // Iter<&HashSet<&files::File>>
        .filter(|s| 1 < s.len())
        // Iter<HashSet<&files::File>>
        .map(|s| {
            let mut identical_files: Vec<&files::File> = s.iter()
                // Iter<&&files::File>
                .cloned()
                // Iter<&files::File>
                .collect();
            identical_files.sort();
            identical_files
        })
        // Iter<Vec<&files::File>>
        .collect();
    let similars: HashSet<Vec<&files::File>> = close_images.iter()
        // Iter<&HashSet<&files::File>>
        .map(|s| {
            let mut similar_files: Vec<&files::File> = s.iter()
                // Iter<&&files::File>
                .cloned()
                // Iter<&files::File>
                .collect();
            similar_files.sort();
            similar_files
        })
        // Iter<Vec<&files::File>>
        .collect();
    if let Err(e) = report::store_report(&identicals, &similars) {
        println!("Could not store report: {}", e);
    } else {
        println!("Report written");
    }

    println!();
    for identityset in identicals {
        print!("identical:");
        for file in identityset {
            print!(" {}", file.displayname());
        }
        println!();
    }
    println!();
    for similarityset in similars {
        print!("similar:");
        for file in similarityset {
            print!(" {}", file.displayname());
        }
        println!();
    }
}
