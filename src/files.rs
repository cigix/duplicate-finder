use std::fs;
use std::io;
use std::path::PathBuf;

use image_hasher::ImageHash;
use md5::{Digest, Md5};
use walkdir::WalkDir;

pub fn list_files() -> Vec<PathBuf>
{
    WalkDir::new(".")
        .into_iter()
        //.filter_entry(|entry| entry.file_type().is_file())
        //.map(|entry| entry.into_path())
        // Result<DirEntry, Error>
        .filter_map(|result| 
            match result {
                Ok(entry) => if entry.file_type().is_file() {
                    Some(entry.into_path())
                } else {
                    None
                },
                _ => None
            }
        )
        .collect()
}

pub struct File {
    path: PathBuf,
    md5: [u8;16],
    ihash: Option<ImageHash>
}

impl File {
    pub fn from(path: PathBuf) -> Result<Self, String>
    {
        let mut file = fs::File::open(&path).map_err(|e| e.to_string())?;
        let mut hasher = Md5::new();
        let _ = io::copy(&mut file, &mut hasher).map_err(|e| e.to_string())?;
        Ok(File {
            path,
            md5: hasher.finalize().into(),
            ihash: None
        })
    }
}
