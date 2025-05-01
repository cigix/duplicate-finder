use crate::files;

/// The default value for [diff]'s `bits` argument.
pub const DEFAULT_BITS: usize = 0;
/// The default value for [diff]'s `parallel` argument.
pub const DEFAULT_PARALLEL: usize = 4;

/// Find and report duplicate and similar files in the current folder.
///
/// Arguments:
/// - `bits`: The bit distance in perceptual hashes to consider two images to be
///   similar. The amount of work grows exponentially with this value; `0` is a
///   good start. Default: [
/// - `parallel`: The number of parallel executions to perform the work.
pub fn diff(bits: Option<usize>, parallel: Option<usize>) -> ()
{
    let bits = bits.unwrap_or(DEFAULT_BITS);
    let parallel = parallel.unwrap_or(DEFAULT_PARALLEL);
    let paths = files::list_files();
    println!("{:?}", paths);
}
