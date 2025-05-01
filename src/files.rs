use std::path::PathBuf;

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
