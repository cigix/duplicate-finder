/// Interact with the report built by [`diff`].
///
/// Files are moved to a temporary trash directory as returned by
/// [`std::env::tmp_dir()`].

use crate::report;

use std::collections::{HashMap, HashSet};
use std::io;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// The name of the trash directory.
pub const TRASH_NAME: &str = "duplicate-finder_trash";

pub fn trash_path() -> PathBuf
{
    let mut path = std::env::temp_dir();
    path.push(TRASH_NAME);
    path
}

/// Send a file to the trash.
///
/// In case of error, the reason is printed, and `false` is returned. Otherwise,
/// return `true`.
pub fn send_to_trash(file: &PathBuf) -> bool
{
    let trash = trash_path();
    let source = PathBuf::from(file);
    let name: String = source.file_name()
        // Option<&OsStr>
        .unwrap()
        .to_string_lossy()
        // Cow<&str>
        .into_owned();
    let destination = trash.join(name);
    // Assume the trash is a different mountpoint, cannot rename
    if let Err(e) = std::fs::copy(&source, &destination) {
        println!("Could not trash {}: {}", &source.display(), e);
        return false;
    }
    if let Err(e) = std::fs::remove_file(&source) {
        println!("Could not trash {}: {}", &source.display(), e);
        return false;
    }
    return true;
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
                if !send_to_trash(&file) {
                    std::process::exit(1);
                }
            }
            report.identicals.clear();
        }
    }

    let mut images: HashMap<usize, (PathBuf, PathBuf)> = HashMap::new();
    let mut samesize: HashSet<usize> = HashSet::new();
    let mut diffsize: HashSet<usize> = HashSet::new();
    for (id, similarityset) in report.similars.iter().enumerate() {
        if similarityset.len() != 2 {
            continue;
        }
        let file1 = PathBuf::from(similarityset.get(0).unwrap());
        let file2 = PathBuf::from(similarityset.get(1).unwrap());
        let image1 = image::open(&file1)
            .expect(&format!("Could not open image {}", &file1.display()));
        let image2 = image::open(&file2)
            .expect(&format!("Could not open image {}", &file2.display()));

        if image1.width() == image2.width()
            && image1.height() == image2.height() {
            let metadata1 = std::fs::metadata(&file1).unwrap();
            let metadata2 = std::fs::metadata(&file2).unwrap();
            let size1 = metadata1.len();
            let size2 = metadata2.len();
            // The lighter image is first
            if size1 < size2 {
                images.insert(id, (file1, file2));
            } else {
                images.insert(id, (file2, file1));
            }
            samesize.insert(id);
            continue;
        }
        if image1.width() < image2.width()
            && image1.height() < image2.height() {
            // The smaller image is first
            images.insert(id, (file1, file2));
            diffsize.insert(id);
            continue;
        }
        if image2.width() < image1.width()
            && image2.height() < image1.height() {
            // The smaller image is first
            images.insert(id, (file2, file1));
            diffsize.insert(id);
            continue;
        }
    }
    // Remove mutability
    let images = images;
    let samesize = samesize;
    let diffsize = diffsize;

    let mut handled: Vec<usize> = Vec::new();

    let diffsize_len = diffsize.len();
    for (progress, id) in diffsize.into_iter().enumerate() {
        let (file1, file2) = images.get(&id).unwrap();
        println!("\n{}/{}: {} vs {}", progress + 1, diffsize_len,
            file1.display(), file2.display());
        let mut feh = Command::new("feh")
            .args([file1, file2])
            .stdin(Stdio::null())
            .spawn()
            .expect("Could not start `feh`");
        println!("These pictures are similar but of different size.");
        print!("Delete the smaller one? [Y/n] ");
        io::stdout().flush().unwrap();
        let mut answer = String::new();
        io::stdin().read_line(&mut answer).unwrap();
        let _ = feh.kill();
        answer.make_ascii_lowercase();
        if answer == "\n" || answer == "y\n" {
            if send_to_trash(&file1) {
                handled.push(id);
                continue
            }
        }
        println!("Keeping it in the report.");
    }

    println!("====================");

    let samesize_len = samesize.len();
    for (progress, id) in samesize.into_iter().enumerate() {
        let (file1, file2) = images.get(&id).unwrap();
        println!("\n{}/{}: {} vs {}", progress + 1, samesize_len,
            file1.display(), file2.display());
        let mut feh = Command::new("feh")
            .args([file1, file2])
            .stdin(Stdio::null())
            .spawn()
            .expect("Could not start `feh`");
        println!("These pictures are similar and have the same size.");
        print!("Delete the heavier one? [Y/n] ");
        io::stdout().flush().unwrap();
        let mut answer = String::new();
        io::stdin().read_line(&mut answer).unwrap();
        let _ = feh.kill();
        answer.make_ascii_lowercase();
        if answer == "\n" || answer == "y\n" {
            if send_to_trash(&file2) {
                handled.push(id);
                continue
            }
        }
        println!("Keeping it in the report.");
    }

    println!();
    handled.sort();
    handled.reverse();
    for id in handled {
        report.similars.swap_remove(id);
    }

    if let Err(e) = report::store_report(&report) {
        println!("Could not store report: {}", e);
    } else {
        println!("Report written");
    }

    if !report.similars.is_empty() {
        println!();
        println!("These files still need attention:");
        for similarityset in report.similars {
            print!("-");
            for file in similarityset {
                print!(" {}", file);
            }
            println!();
        }
    }
}
