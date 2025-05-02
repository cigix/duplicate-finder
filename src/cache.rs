//! Manage the caching of data. The cache file is placed in the cache folder as
//! returned by [`dirs::cache_dir()`] or the current directory if unavailable.

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use dirs;

/// The name of the cache file.
const CACHE_NAME: &str = "duplicate-finder_cache.json";

/// The path of the cache file. This path is computed dynamically from
/// [`dirs::cache_dir()`] and [`CACHE_NAME`].
pub fn cache_path() -> PathBuf
{
    let mut path = dirs::cache_dir()
        .unwrap_or(".".into());
    path.push(CACHE_NAME);
    path
}

pub fn load_cache() -> Result<HashMap<String, f32>, String>
{
    let path = cache_path();
    let file = File::open(path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    let cache = serde_json::from_reader(reader).map_err(|e| e.to_string())?;
    Ok(cache)
}
