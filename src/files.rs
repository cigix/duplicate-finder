use std::cmp::Ordering;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::PathBuf;

use image_hasher::{HasherConfig, ImageHash};
use md5::{Digest, Md5};
use walkdir::WalkDir;

/// Files who are considered images based on their extensions.
pub const IMAGE_EXTENSIONS: [&str;3] = ["jpg", "png", "webp"];

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
    pub path: PathBuf,
    pub md5: [u8;16],
    pub ihash: Option<ImageHash>
}

fn get_image_hash(path: &PathBuf) -> Option<ImageHash>
{
    let image = image::open(path).ok()?;
    let hasher = HasherConfig::new().to_hasher();
    let hash = hasher.hash_image(&image);
    Some(hash)
}

impl File {
    pub fn from(path: &PathBuf) -> Result<Self, String>
    {
        let mut file = File::from_noihash(path)?;
        let extension: String = path.extension()
            // Option<&OsStr>
            .unwrap_or_default()
            // &OsStr
            .to_string_lossy()
            // Cow<&str>
            .into_owned();

        if IMAGE_EXTENSIONS.contains(&extension.as_str()) {
            file.ihash = get_image_hash(path)
        }
        Ok(file)
    }
    pub fn from_noihash(path: &PathBuf) -> Result<Self, String>
    {

        let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
        let mut hasher = Md5::new();
        let _ = io::copy(&mut file, &mut hasher).map_err(|e| e.to_string())?;
        Ok(File {
            path: path.to_path_buf(),
            md5: hasher.finalize().into(),
            ihash: None
        })
    }

    pub fn displayname(&self) -> String
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
