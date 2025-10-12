use std::cmp::Ordering;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::PathBuf;

use image::AnimationDecoder;
use image_hasher::{HasherConfig, ImageHash};
use md5::{Digest, Md5};
use walkdir::WalkDir;

/// The category of file in regards to comparison.
#[derive(PartialEq,Eq)]
pub enum Category {
    UNKNOWN,
    IMAGE,
    ANIMATION,
    VIDEO,
}

/// Files who are considered images based on their extensions. WepB is handled
/// separately, see [get_category()].
const IMAGE_EXTENSIONS: [&str;2] = ["jpg", "png"];
/// Files who are considered animations based on their extensions. WebP is
/// handled separately, see [get_category()].
const ANIM_EXTENSIONS: [&str;1] = ["gif"];
/// Files who are considered videos based on their extensions.
const VIDEO_EXTENSIONS: [&str;2] = ["mp4", "webm"];
/// Files who are of the WebP format, which can hold both images and animations.
const WEBP_EXTENSIONS: [&str;1] = ["webp"];

/// Get the category of the file based on its extension:
/// - [Category::IMAGE] for extensions in [IMAGE_EXTENSIONS],
/// - [Category::ANIMATION] for extensions in [ANIM_EXTENSIONS],
/// - [Category::VIDEO] for extensions in [VIDEO_EXTENSIONS],
/// - [Category::UNKNOWN] otherwise.
///
/// Files matching [WEBP_EXTENSIONS] are opened to determine if they are
/// [Category::IMAGE] or [Category::ANIMATION].
///
/// # Errors
///
/// This function errors if a WebP file cannot be opened or decoded.
pub fn get_category(path: &PathBuf) -> Result<Category, String>
{
    let extension: String = path.extension()
        // Option<&OsStr>
        .unwrap_or_default()
        // &OsStr
        .to_string_lossy()
        // Cow<&str>
        .into_owned();

    if WEBP_EXTENSIONS.contains(&extension.as_str()) {
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let reader = std::io::BufReader::new(file);
        let decoder = image::codecs::webp::WebPDecoder::new(reader)
            .map_err(|e| e.to_string())?;
        return Ok(if decoder.has_animation() {
            Category::ANIMATION
        } else {
            Category::IMAGE
        });
    }
    if IMAGE_EXTENSIONS.contains(&extension.as_str()) {
        return Ok(Category::IMAGE);
    }
    if ANIM_EXTENSIONS.contains(&extension.as_str()) {
        return Ok(Category::ANIMATION);
    }
    if VIDEO_EXTENSIONS.contains(&extension.as_str()) {
        return Ok(Category::VIDEO);
    }
    Ok(Category::UNKNOWN)
}

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
    pub category: Category,
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

fn get_anim_hash(path: &PathBuf) -> Result<ImageHash, String>
{
    let extension: String = path.extension()
        // Option<&OsStr>
        .unwrap_or_default()
        // &OsStr
        .to_string_lossy()
        // Cow<&str>
        .into_owned();

    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let reader = std::io::BufReader::new(file);

    let first_frame : image::Frame =
        if WEBP_EXTENSIONS.contains(&extension.as_str()) {
            image::codecs::webp::WebPDecoder::new(reader)
                // Result<WebPDecoder, ImageError>
                .map_err(|e| e.to_string())?
                // WebPDecoder
                .into_frames()
                // Frames
                .next()
                // Option<Result<Frame, ImageError>>
                .ok_or("No first frame")?
                // Result<Frame, ImageError>
                .map_err(|e| e.to_string())?
        } else {
            // Ditto but with GifDecoder
            image::codecs::gif::GifDecoder::new(reader)
                .map_err(|e| e.to_string())?
                .into_frames()
                .next()
                .ok_or("No first frame")?
                .map_err(|e| e.to_string())?
        };

    let hasher = HasherConfig::new().to_hasher();
    let hash = hasher.hash_image(first_frame.buffer());
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

        file.ihash = match file.category {
                Category::IMAGE => Some(get_image_hash(path)),
                Category::ANIMATION => Some(get_anim_hash(path)),
                Category::VIDEO => Some(get_video_hash(path)),
                Category::UNKNOWN => None
            }
            // Option<Result<ImageHash, String>>
            .transpose()
            // Result<Option<ImageHash>, String>
            .unwrap_or_else(|e| {
                eprintln!("{}: {}", path.display(), e);
                None
            });

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
            category: get_category(&path)?,
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
