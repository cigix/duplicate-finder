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

/// Files who are considered videos based on their extensions.
pub const VIDEO_EXTENSIONS: [&str;2] = ["mp4", "gif"];

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

fn get_image_hash(path: &PathBuf) -> Result<ImageHash, String>
{
    let image = image::open(path).map_err(|e| e.to_string())?;
    let hasher = HasherConfig::new().to_hasher();
    let hash = hasher.hash_image(&image);
    Ok(hash)
}

fn get_video_hash(path: &PathBuf) -> Result<ImageHash, String>
{
    // Adapted from https://github.com/zmwangx/rust-ffmpeg/blob/master/examples/dump-frames.rs
    let mut ictx = ffmpeg_next::format::input(&path)
        .map_err(|e| e.to_string())?;
    let video_stream = ictx.streams().best(ffmpeg_next::media::Type::Video)
        .ok_or("No suitable video stream found")?;
    let video_stream_index = video_stream.index();

    let mut decoder = ffmpeg_next::codec::context::Context::from_parameters(
            video_stream.parameters())
        // Result<Context, Error>
        .and_then(|c| c.decoder().video())
        // Result<Video, Error>
        .map_err(|e| e.to_string())?;

    let mut scaler = ffmpeg_next::software::scaling::context::Context::get(
            decoder.format(),
            decoder.width(),
            decoder.height(),
            ffmpeg_next::format::Pixel::RGB24,
            decoder.width(),
            decoder.height(),
            ffmpeg_next::software::scaling::flag::Flags::BILINEAR)
        .map_err(|e| e.to_string())?;

    let mut decoded_frame = ffmpeg_next::util::frame::video::Video::empty();
    let mut scaled_frame = ffmpeg_next::util::frame::video::Video::empty();
    for (_, packet) in ictx.packets()
        .filter(|(s,_)| s.index() == video_stream_index)
    {
        decoder.send_packet(&packet).map_err(|e| e.to_string())?;
        match decoder.receive_frame(&mut decoded_frame) {
            Ok(()) => break,
            Err(e) => match e {
                ffmpeg_next::util::error::Error::Other {
                    errno: ffmpeg_next::util::error::EAGAIN } => continue,
                e => return Err(e.to_string()),
            }
        }
    }
    scaler.run(&decoded_frame, &mut scaled_frame).map_err(|e| e.to_string())?;

    let data : Vec<u8> = Vec::from(scaled_frame.data(0));
    let width = scaled_frame.width();
    let height = scaled_frame.height();
    // The data vector has row-padding to 32 pixels. Don't ask where this is
    // documented. Don't ask how much time I spent on this either.
    let padded_width = (width + 31) & !31;
    let padded_image : image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
        image::ImageBuffer::from_raw(padded_width, height, data)
        .ok_or("Could not convert frame to image")?;
    let image = image::imageops::crop_imm(&padded_image, 0, 0, width, height)
        .to_image();

    let hasher = HasherConfig::new().to_hasher();
    let hash = hasher.hash_image(&image);
    Ok(hash)
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
            file.ihash = match get_image_hash(path) {
                Ok(h) => Some(h),
                Err(e) => {
                    eprintln!("{}: {}", path.display(), e);
                    None
                }
            }
        }
        if VIDEO_EXTENSIONS.contains(&extension.as_str()) {
            file.ihash = match get_video_hash(path) {
                Ok(h) => Some(h),
                Err(e) => {
                    eprintln!("{}: {}", path.display(), e);
                    None
                }
            }
        }
        Ok(file)
    }
    pub fn from_noihash(path: &PathBuf) -> Result<Self, String>
    {
        let mut file = fs::File::open(path)
            .map_err(|s| format!("{}: {}", path.display(), s))?;
        let mut hasher = Md5::new();
        let _ = io::copy(&mut file, &mut hasher)
            .map_err(|s| format!("{}: {}", path.display(), s))?;
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
