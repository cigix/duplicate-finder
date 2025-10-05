/// Interact with the report built by [`diff`].
///
/// Files are moved to a temporary trash directory as returned by
/// [`std::env::tmp_dir()`].

use crate::false_positives;
use crate::files;
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

#[derive(Clone, Copy, PartialEq)]
enum Choice {
    Yes,
    No,
    First,
    Second,
    KeepBoth,
    FalsePositive,
}

impl Choice {
    pub fn to_letter(&self) -> char
    {
        match self {
            Self::Yes => 'y',
            Self::No => 'n',
            Self::First => '1',
            Self::Second => '2',
            Self::KeepBoth => 'k',
            Self::FalsePositive => 'f',
        }
    }
    pub fn from_letter(c: char, default: Choice) -> Self
    {
        match c.to_ascii_lowercase() {
            'y' => Self::Yes,
            'n' => Self::No,
            '1' => Self::First,
            '2' => Self::Second,
            'k' => Self::KeepBoth,
            'f' => Self::FalsePositive,
            _ => default
        }
    }
    pub fn each() -> [Self;6]
    {
        [Self::Yes, Self::No, Self::First, Self::Second, Self::KeepBoth, Self::FalsePositive]
    }
}

fn make_choice(prompt: &str, default: Choice) -> Choice
{
    print!("{} [", prompt);
    let mut first = true;
    for choice in Choice::each() {
        let mut c = choice.to_letter();
        if default == choice {
            c.make_ascii_uppercase()
        }
        if first {
            first = false;
        } else {
            print!("/");
        }
        print!("{}", c);
    }
    print!("] ");
    io::stdout().flush().unwrap();
    let mut answer = String::new();
    io::stdin().read_line(&mut answer).unwrap();
    match answer.chars().next() {
        Some('\n') => default,
        Some(c) => Choice::from_letter(c, Choice::No),
        None => default
    }
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
    let mut fp = match false_positives::load() {
        Ok(fp) => fp,
        Err(e) => {
            println!("Could not load false positives: {}", e);
            false_positives::FalsePositives::default()
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

    // The pairs of files that reprensent similar animations, images, and
    // videos, by id of appearance in the report.
    let mut pairs: HashMap<usize, (files::File, files::File)> = HashMap::new();

    // The ids of similar animations.
    let mut similar_anims: HashSet<usize> = HashSet::new();
    // The ids of similar videos.
    let mut similar_videos: HashSet<usize> = HashSet::new();
    // The ids of similar images that have one clearly larger than the other.
    let mut diffdims: HashSet<usize> = HashSet::new();
    // The ids of similar images that have the same dimensions.
    let mut samedims: HashSet<usize> = HashSet::new();
    // The ids of similar images that do not fit the two previous categories.
    let mut other_images: HashSet<usize> = HashSet::new();

    // The ids of pairs that have been handled and can be taken out of the
    // report.
    let mut handled: Vec<usize> = Vec::new();
    // The number of false positives that have been handled automatically from
    // the false_positives report.
    let mut fp_auto = 0usize;

    for (id, similarityset) in report.similars.iter().enumerate() {
        // We only consider pairs here
        if similarityset.len() != 2 {
            continue;
        }

        let path1 = PathBuf::from(similarityset.get(0).unwrap());
        let path2 = PathBuf::from(similarityset.get(1).unwrap());

        let extension1 = path1.extension()
            // Option<&OsStr>
            .unwrap_or_default()
            // &OsStr
            .to_string_lossy()
            // Cow<&str>
            .into_owned();
        let extension2 = path2.extension()
            // Option<&OsStr>
            .unwrap_or_default()
            // &OsStr
            .to_string_lossy()
            // Cow<&str>
            .into_owned();

        let file1 = files::File::from_noihash(&path1).unwrap();
        let file2 = files::File::from_noihash(&path2).unwrap();

        let pair1 = [file1.md5, file2.md5];
        let pair2 = [file2.md5, file1.md5];
        if fp.keep.contains(&pair1) || fp.keep.contains(&pair2) {
            handled.push(id);
            continue;
        }
        if fp.false_positives.contains(&pair1)
            || fp.false_positives.contains(&pair2) {
            handled.push(id);
            fp_auto += 1;
            continue
        }

        if files::ANIM_EXTENSIONS.contains(&extension1.as_str())
            && files::ANIM_EXTENSIONS.contains(&extension2.as_str()) {
            // both are animations
            pairs.insert(id, (file1, file2));
            similar_anims.insert(id);
        } else if files::VIDEO_EXTENSIONS.contains(&extension1.as_str())
            && files::VIDEO_EXTENSIONS.contains(&extension2.as_str()) {
            // both are videos
            pairs.insert(id, (file1, file2));
            similar_videos.insert(id);
        } else if files::IMAGE_EXTENSIONS.contains(&extension1.as_str())
            && files::IMAGE_EXTENSIONS.contains(&extension2.as_str()) {
            // both are images
            // We do not insert into pairs yet: we need to be able to borrow,
            // and alter the order of the pair.

            let image1 = image::open(&path1)
                .expect(&format!("Could not open image {}", &path1.display()));
            let image2 = image::open(&path2)
                .expect(&format!("Could not open image {}", &path2.display()));

            if image1.width() == image2.width()
                && image1.height() == image2.height() {
                let metadata1 = std::fs::metadata(&file1.path).unwrap();
                let metadata2 = std::fs::metadata(&file2.path).unwrap();
                let size1 = metadata1.len();
                let size2 = metadata2.len();
                // The lighter image is first
                if size1 < size2 {
                    pairs.insert(id, (file1, file2));
                } else {
                    pairs.insert(id, (file2, file1));
                }
                samedims.insert(id);
                continue;
            }
            if image1.width() < image2.width()
                && image1.height() < image2.height() {
                // The smaller image is first
                pairs.insert(id, (file1, file2));
                diffdims.insert(id);
                continue;
            }
            if image2.width() < image1.width()
                && image2.height() < image1.height() {
                // The smaller image is first
                pairs.insert(id, (file2, file1));
                diffdims.insert(id);
                continue;
            }

            pairs.insert(id, (file1, file2));
            other_images.insert(id);
        }
    }
    // Remove mutability
    let pairs = pairs;
    let similar_anims = similar_anims;
    let similar_videos = similar_videos;
    let diffdims = diffdims;
    let samedims = samedims;
    let other_images = other_images;

    let mut fp_added = 0usize;

    let diffdims_len = diffdims.len();
    for (progress, id) in diffdims.into_iter().enumerate() {
        let (file1, file2) = pairs.get(&id).unwrap();
        println!("\n{}/{}: {} vs {}", progress + 1, diffdims_len,
            file1.path.display(), file2.path.display());
        let mut viewer = Command::new("feh")
            .args([&file1.path, &file2.path])
            .stdin(Stdio::null())
            .spawn()
            .expect("Could not start `feh`");
        println!("These pictures are similar but of different dimensions.");
        match make_choice("Delete the smaller one?", Choice::Yes) {
            Choice::No => println!("Keeping them in the report"),
            Choice::Yes | Choice::First => {
                println!("Deleting {}", file1.path.display());
                if send_to_trash(&file1.path) { handled.push(id); }
            }
            Choice::Second => {
                println!("Deleting {}", file2.path.display());
                if send_to_trash(&file2.path) { handled.push(id); }
            }
            Choice::KeepBoth => {
                println!("Keeping both");
                fp.keep.insert([file1.md5, file2.md5]);
                handled.push(id);
            }
            Choice::FalsePositive => {
                println!("False positive");
                fp.false_positives.insert([file1.md5, file2.md5]);
                handled.push(id);
                fp_added += 1;
            }
        }
        let _ = viewer.kill();
    }

    println!("\n====================");

    let samesize_len = samedims.len();
    for (progress, id) in samedims.into_iter().enumerate() {
        let (file1, file2) = pairs.get(&id).unwrap();
        println!("\n{}/{}: {} vs {}", progress + 1, samesize_len,
            file1.path.display(), file2.path.display());
        let mut viewer = Command::new("feh")
            .args([&file1.path, &file2.path])
            .stdin(Stdio::null())
            .spawn()
            .expect("Could not start `feh`");
        println!("These pictures are similar and have the same dimensions.");
        match make_choice("Delete the heavier one?", Choice::Yes) {
            Choice::No => println!("Keeping them in the report."),
            Choice::First => {
                println!("Deleting {}", file1.path.display());
                if send_to_trash(&file1.path) { handled.push(id); }
            }
            Choice::Yes | Choice::Second => {
                println!("Deleting {}", file2.path.display());
                if send_to_trash(&file2.path) { handled.push(id); }
            }
            Choice::KeepBoth => {
                println!("Keeping both");
                fp.keep.insert([file1.md5, file2.md5]);
                handled.push(id);
            }
            Choice::FalsePositive => {
                println!("False positive");
                fp.false_positives.insert([file1.md5, file2.md5]);
                handled.push(id);
                fp_added += 1;
            }
        }
        let _ = viewer.kill();
    }

    println!("\n====================");

    let others_len = other_images.len();
    for (progress, id) in other_images.into_iter().enumerate() {
        let (file1, file2) = pairs.get(&id).unwrap();
        println!("\n{}/{}: {} vs {}", progress + 1, others_len,
            file1.path.display(), file2.path.display());
        let mut viewer = Command::new("feh")
            .args([&file1.path, &file2.path])
            .stdin(Stdio::null())
            .spawn()
            .expect("Could not start `feh`");
        println!("These pictures are roughly similar.");
        match make_choice("Keep both?", Choice::Yes) {
            Choice::No => println!("Keeping them in the report."),
            Choice::First => {
                println!("Deleting {}", file1.path.display());
                if send_to_trash(&file1.path) { handled.push(id); }
            }
            Choice::Second => {
                println!("Deleting {}", file2.path.display());
                if send_to_trash(&file2.path) { handled.push(id); }
            }
            Choice::Yes | Choice::KeepBoth => {
                println!("Keeping both");
                fp.keep.insert([file1.md5, file2.md5]);
                handled.push(id);
            }
            Choice::FalsePositive => {
                println!("False positive");
                fp.false_positives.insert([file1.md5, file2.md5]);
                handled.push(id);
                fp_added += 1;
            }
        }
        let _ = viewer.kill();
    }

    println!("\n====================");

    let anims_len = similar_anims.len();
    for (progress, id) in similar_anims.into_iter().enumerate() {
        let (file1, file2) = pairs.get(&id).unwrap();
        println!("\n{}/{}: {} vs {}", progress + 1, anims_len,
            file1.path.display(), file2.path.display());
        let mut viewer = Command::new("gwenview")
            .args([&file1.path, &file2.path])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("Could not start `gwenview`");
        println!("These animations start similarly.");
        match make_choice("Keep both?", Choice::Yes) {
            Choice::No => println!("Keeping them in the report."),
            Choice::First => {
                println!("Deleting {}", file1.path.display());
                if send_to_trash(&file1.path) { handled.push(id); }
            }
            Choice::Second => {
                println!("Deleting {}", file2.path.display());
                if send_to_trash(&file2.path) { handled.push(id); }
            }
            Choice::Yes | Choice::KeepBoth => {
                println!("Keeping both");
                fp.keep.insert([file1.md5, file2.md5]);
                handled.push(id);
            }
            Choice::FalsePositive => {
                println!("False positive");
                fp.false_positives.insert([file1.md5, file2.md5]);
                handled.push(id);
                fp_added += 1;
            }
        }
        let _ = viewer.kill();
    }

    println!("\n====================");

    let videos_len = similar_videos.len();
    for (progress, id) in similar_videos.into_iter().enumerate() {
        let (file1, file2) = pairs.get(&id).unwrap();
        println!("\n{}/{}: {} vs {}", progress + 1, videos_len,
            file1.path.display(), file2.path.display());
        let mut viewer = Command::new("vlc")
            .args([&file1.path, &file2.path])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("Could not start `vlc`");
        println!("These videos start similarly.");
        match make_choice("Keep both?", Choice::Yes) {
            Choice::No => println!("Keeping them in the report."),
            Choice::First => {
                println!("Deleting {}", file1.path.display());
                if send_to_trash(&file1.path) { handled.push(id); }
            }
            Choice::Second => {
                println!("Deleting {}", file2.path.display());
                if send_to_trash(&file2.path) { handled.push(id); }
            }
            Choice::Yes | Choice::KeepBoth => {
                println!("Keeping both");
                fp.keep.insert([file1.md5, file2.md5]);
                handled.push(id);
            }
            Choice::FalsePositive => {
                println!("False positive");
                fp.false_positives.insert([file1.md5, file2.md5]);
                handled.push(id);
                fp_added += 1;
            }
        }
        let _ = viewer.kill();
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
    if let Err(e) = false_positives::store(&fp) {
        println!("Could not store false positives: {}", e);
    } else {
        println!("False positive reviews written");
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

    if 0 < pairs.len() {
        let fp_total = fp_auto + fp_added;
        println!();
        println!("False positives rate: {}% ({}/{})",
                 100 * fp_total / pairs.len(), fp_total, pairs.len());
    }
}
