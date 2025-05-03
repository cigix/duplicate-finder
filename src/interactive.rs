/// Interact with the report built by [`diff`].
///
/// Files are moved to a temporary trash directory as returned by
/// [`std::env::tmp_dir()`].

use crate::report;

use std::io;
use std::io::Write;
use std::path::PathBuf;

/// The name of the trash directory.
pub const TRASH_NAME: &str = "duplicate-finder_trash";

pub fn trash_path() -> PathBuf
{
    let mut path = std::env::temp_dir();
    path.push(TRASH_NAME);
    path
}

pub fn interactive()
{
    let mut report = match report::load_report() {
        Ok(report) => report,
        Err(e) => {
            println!("Could not load report: {}", e);
            std::process::exit(1);
        }
    };
    
    let trash = trash_path();
    let _ = std::fs::remove_dir_all(&trash);
    std::fs::create_dir(&trash).expect("Could not create trash directory");
    
    let todelete: Vec<PathBuf> = report.identicals.iter()
        // Iter<&Vec<String>>
        .map(|v| v.iter().skip(1))
        // Iter<Iter<&String>>
        .flatten()
        // Iter<&String>
        .map(PathBuf::from)
        // Iter<PathBuf>
        .filter(|f| f.is_file())
        .collect();
    if !todelete.is_empty() {
        println!("{} files have identical matches.", todelete.len());
        print!("Delete them? [Y/n] ");
        io::stdout().flush().unwrap();
        let mut answer = String::new();
        io::stdin().read_line(&mut answer).unwrap();
        answer.make_ascii_lowercase();
        if answer == "\n" || answer == "y\n" {
            for file in todelete {
                let source = PathBuf::from(file);
                let name: String = source.file_name()
                    // Option<&OsStr>
                    .unwrap()
                    .to_string_lossy()
                    // Cow<&str>
                    .into_owned();
                let destination = trash.join(name);
                // Assume the trash is a different mountpoint, cannot rename
                std::fs::copy(&source, destination)
                    .expect(&format!("Could not trash {}", &source.display()));
                std::fs::remove_file(&source)
                    .expect(&format!("Could not trash {}", &source.display()));
            }
            report.identicals.clear();
        }
    }

    if let Err(e) = report::store_report(&report) {
        println!("Could not store report: {}", e);
    } else {
        println!("Report written");
    }
}
