//! Manage the storage of false positive reviews. The false positives file is
//! placed in the cache folder as returned by [`dirs::cache_dir()`] or the
//! current directory if unavailable.

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;

use dirs;
use serde::{Deserialize, Serialize};

/// The name of the false positives file.
const FP_NAME: &str = "duplicate-finder_false_positives.json";

#[derive(Serialize, Deserialize)]
struct JSONFalsePositives {
    /// The sets of files we want to keep.
    pub keep: Vec<Vec<String>>,
    /// The sets of false positives.
    pub false_positives: Vec<Vec<String>>
}

#[derive(Default)]
pub struct FalsePositives {
    /// The sets of files we want to keep.
    pub keep: HashSet<[[u8;16];2]>,
    /// The sets of false positives.
    pub false_positives: HashSet<[[u8;16];2]>
}

fn set_to_vec(hashes: &HashSet<[[u8;16];2]>) -> Vec<Vec<String>>
{
    hashes.iter()
        // Iter<&[[u8;16];2]>
        .map(|a| {
            let mut v: Vec<String> = a.iter()
                // Iter<&[u8;16]>
                .map(hex::encode)
                // Iter<String>
                .collect();
            v.sort();
            v
        })
        // Iter<Vec<String>>
        .collect()
}

fn vec_to_set(hashes: &Vec<Vec<String>>) -> Result<HashSet<[[u8;16];2]>, String>
{
    hashes.iter()
        // Iter<&Vec<String>>
        .map(|v| {
            v.iter()
                // Iter<&String>
                .map(|s| hex::decode(s)
                    // Result<Vec<u8>, FromHexError>
                    .map_err(|e| e.to_string())
                    // Result<Vec<u8>, String>
                    .and_then(|v| TryInto::<[u8;16]>::try_into(v)
                        // Result<[u8;16], Vec<u8>>
                        .map_err(|v|
                            format!("Invalid hash length: {}, expected 16",
                                    v.len())
                        )
                        // Result<[u8;16], String>
                    )
                    // Result<[u8;16], String>
                )
                // Iter<Result<[u8;16], String>>
                .collect::<Result<Vec<[u8;16]>, String>>() // stops at first Err
                // Result<Vec<[u8;16]>, String>
                .and_then(|v| TryInto::<[[u8;16];2]>::try_into(v)
                    // Result<[[u8;16];2], Vec<[u8;16]>
                    .map_err(|v|
                        format!("Invalid number of entries: {}, expected 2",
                                v.len())
                    )
                    // Result<[[u8;16];2], String>
                )
                // Result<[[u8;16];2], String>
        })
        // Iter<Result<[[u8;16];2], String>
        .collect() // stops at first Err
}

impl From<&FalsePositives> for JSONFalsePositives
{
    fn from(fp: &FalsePositives) -> Self
    {
        JSONFalsePositives {
            keep: set_to_vec(&fp.keep),
            false_positives: set_to_vec(&fp.false_positives)
        }
    }
}

impl TryFrom<&JSONFalsePositives> for FalsePositives
{
    type Error = String;

    fn try_from(jfp: &JSONFalsePositives) -> Result<Self, Self::Error>
    {
        Ok(FalsePositives {
            keep: vec_to_set(&jfp.keep)?,
            false_positives: vec_to_set(&jfp.false_positives)?
        })
    }
}

/// The path of the false positives file. This path is computed dynamically from
/// [`dirs::cache_dir()`] and [`FP_NAME`].
pub fn fp_path() -> PathBuf
{
    let mut path = dirs::cache_dir()
        .unwrap_or(".".into());
    path.push(FP_NAME);
    path
}

pub fn store(fp: &FalsePositives) -> Result<(), String>
{
    let jfp : JSONFalsePositives = fp.into();
    let path = fp_path();
    let file = File::create(path).map_err(|e| e.to_string())?;
    let writer = BufWriter::new(file);
    serde_json::to_writer(writer, &jfp).map_err(|e| e.to_string())
}

pub fn load() -> Result<FalsePositives, String>
{
    let path = fp_path();
    let file = File::open(path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    let jfp : JSONFalsePositives =
        serde_json::from_reader(reader).map_err(|e| e.to_string())?;
    (&jfp).try_into()
}
