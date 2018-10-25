pub mod entry;
pub mod frame;
pub mod frame_allocator;
pub mod mapper;
pub mod page;
pub mod table;

pub use self::frame::Frame;
pub use self::frame_allocator::FrameAllocator;
pub use self::page::Page;

pub const FRAME_SIZE: u64 = 0x1000;
pub const PAGE_SIZE: u64 = 0x1000;
