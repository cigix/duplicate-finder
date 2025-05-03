//! Manage the storage of reports. The report file is placed in the cache folder
//! as returned by [`dirs::cache_dir()`] or the current directory if
//! unavailable.

use crate::files;

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;

use dirs;
use serde::{Deserialize, Serialize};

/// The name of the report file.
const REPORT_NAME: &str = "duplicate-finder_report.json";

fn set_to_vec(files: &HashSet<Vec<&files::File>>) -> Vec<Vec<String>>
{
    files.iter()
        // Iter<&Vec<&files::File>>
        .map(|v| {
            let mut v2: Vec<String> = v.iter()
                // Iter<&&files::File>
                .map(|f| f.displayname())
                // Iter<String>
                .collect();
            v2.sort();
            v2
        })
        // Iter<Vec<String>>
        .collect()
}

#[derive(Serialize, Deserialize)]
pub struct Report {
    /// The sets of identical files.
    pub identicals: Vec<Vec<String>>,
    /// The sets of similar files.
    pub similars: Vec<Vec<String>>
}

impl Report {
    pub fn from(identicals: &HashSet<Vec<&files::File>>,
                similars: &HashSet<Vec<&files::File>>) -> Report
    {
        Report {
            identicals: set_to_vec(identicals),
            similars: set_to_vec(similars)
        }
    }
}

/// The path of the report file. This path is computed dynamically from
/// [`dirs::cache_dir()`] and [`REPORT_NAME`].
pub fn report_path() -> PathBuf
{
    let mut path = dirs::cache_dir()
        .unwrap_or(".".into());
    path.push(REPORT_NAME);
    path
}

pub fn store_report(report: &Report) -> Result<(), String>
{
    let path = report_path();
    let file = File::create(path).map_err(|e| e.to_string())?;
    let writer = BufWriter::new(file);
    serde_json::to_writer(writer, &report).map_err(|e| e.to_string())
}

pub fn load_report() -> Result<Report, String>
{
    let path = report_path();
    let file = File::open(path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    let report = serde_json::from_reader(reader).map_err(|e| e.to_string())?;
    Ok(report)
}
