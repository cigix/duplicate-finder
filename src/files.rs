use std::cmp::Ordering;
use std::fs;
use std::hash::{Hash, Hasher};
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

    pub fn hash(&self) -> &[u8;16]
    {
        &self.md5
    }
    pub fn name(&self) -> String
    {
        self.path
            // PathBuf
            .strip_prefix(".").unwrap()
            // &Path
            .to_str().unwrap()
            // &str
            .to_string()
    }
}

impl PartialEq for File {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}
impl Eq for File {}
impl Ord for File {
    fn cmp(&self, other: &Self) -> Ordering
    {
        self.path.cmp(&other.path)
    }
}
impl PartialOrd for File {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering>
    {
        Some(self.cmp(other))
    }
}
impl Hash for File {
    fn hash<H: Hasher>(&self, state: &mut H)
    {
        self.path.hash(state);
    }
}
