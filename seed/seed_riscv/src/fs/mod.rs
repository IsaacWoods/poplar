pub mod ramdisk;

use alloc::string::String;

/// A `Filesystem` represents something that can meaningfully contain 'files' - discrete chunks of
/// data addressed using paths. For Seed, this is generally going to be a real filesystem that
/// occupies a partition on a block device, either real or virtual, or the very simple 'filesystem'
/// provided by the ramdisk used on some platforms.
///
/// This interface (at the moment, at least) is much simpler than a 'real' one. You can simply load
/// a file in its entirity into memory, and then close it once you're done with it. In the future,
/// this could be made smarter, but is probably sufficient for a bootloader as is.
pub trait Filesystem {
    fn load(&mut self, path: &str) -> Result<File, ()>;
    fn close(&mut self, file: File);
}

// TODO: some filesystems will need to allocate memory for this (real ones) and others wont (the
// ramdisk). We probably need a way to represent both scenarios. (this could be another trait with
// a `data` method? And then the concrete type an associated type of the FS?)
pub struct File<'a> {
    pub path: String,
    pub data: &'a [u8],
}
