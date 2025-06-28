use core::{ops, ptr, slice, str};
use hal::memory::{Frame, FrameAllocator, FrameSize, PAddr, Size4KiB, VAddr};
use seed_bootinfo::{Header, LoadedSegment, MemoryEntry, MemoryType, VideoModeInfo};

pub struct BootInfo {
    pub base: *const Header,
    string_table_offset: usize,
}

impl BootInfo {
    pub unsafe fn new(base: *const ()) -> BootInfo {
        let base = base as *const Header;

        if unsafe { *base }.magic != seed_bootinfo::MAGIC {
            panic!("Boot info passed from bootloader has incorrect magic!");
        }

        let string_table_offset = unsafe { *base }.string_table_offset as usize;

        BootInfo { base, string_table_offset }
    }

    pub fn memory_map(&self) -> &[MemoryEntry] {
        let header = unsafe { *self.base };
        unsafe {
            slice::from_raw_parts(
                self.base.byte_add(header.mem_map_offset as usize) as *const MemoryEntry,
                header.mem_map_length as usize,
            )
        }
    }

    pub fn kernel_free_start(&self) -> VAddr {
        VAddr::new(unsafe { *self.base }.kernel_free_start as usize)
    }

    pub fn rsdp_addr(&self) -> Option<u64> {
        match unsafe { *self.base }.rsdp_address {
            0 => None,
            addr => Some(addr),
        }
    }

    pub fn device_tree_addr(&self) -> Option<u64> {
        match unsafe { *self.base }.device_tree_address {
            0 => None,
            addr => Some(addr),
        }
    }

    pub fn num_loaded_images(&self) -> usize {
        unsafe { *self.base }.num_loaded_images as usize
    }

    pub fn loaded_images(&self) -> impl Iterator<Item = LoadedImage<'_>> {
        let header = unsafe { *self.base };
        let raw_ptr = unsafe {
            self.base.byte_add(header.loaded_images_offset as usize) as *const seed_bootinfo::LoadedImage
        };

        // TODO: bounds check length
        unsafe { slice::from_raw_parts(raw_ptr, header.num_loaded_images as usize) }.iter().map(move |raw| {
            let name = unsafe { self.read_string(raw.name_offset, raw.name_len) };

            LoadedImage {
                name,
                num_segments: raw.num_segments as usize,
                segments: raw.segments,
                entry_point: raw.entry_point,
            }
        })
    }

    pub fn video_mode_info(&self) -> Option<VideoModeInfo> {
        match unsafe { *self.base }.video_mode_offset {
            0 => None,
            offset => Some(unsafe { ptr::read(self.base.byte_add(offset as usize) as *const VideoModeInfo) }),
        }
    }

    unsafe fn read_string(&self, offset: u16, len: u16) -> &'_ str {
        let start = self.base.byte_add(self.string_table_offset + offset as usize) as *const u8;
        // TODO: bounds check against string table length
        unsafe { str::from_raw_parts(start, len as usize) }
    }
}

pub struct LoadedImage<'a> {
    pub name: &'a str,
    // TODO: maybe we can just do a slice here?
    pub num_segments: usize,
    pub segments: [LoadedSegment; seed_bootinfo::LOADED_IMAGE_MAX_SEGMENTS],
    pub entry_point: u64,
}

pub struct EarlyFrameAllocator<'a> {
    boot_info: &'a mut BootInfo,
}

impl<'a> EarlyFrameAllocator<'a> {
    pub fn new(boot_info: &'a mut BootInfo) -> EarlyFrameAllocator<'a> {
        EarlyFrameAllocator { boot_info }
    }
}

impl<'a> FrameAllocator<Size4KiB> for EarlyFrameAllocator<'a> {
    fn allocate_n(&self, n: usize) -> ops::Range<Frame> {
        let header = unsafe { *self.boot_info.base };
        let memory_map = unsafe {
            slice::from_raw_parts_mut(
                self.boot_info.base.byte_add(header.mem_map_offset as usize) as *mut MemoryEntry,
                header.mem_map_length as usize,
            )
        };

        for i in 0..(header.mem_map_length as usize) {
            if memory_map[i].typ == MemoryType::Usable && memory_map[i].length as usize >= n * Size4KiB::SIZE {
                let base =
                    PAddr::new(memory_map[i].base as usize + memory_map[i].length as usize - n * Size4KiB::SIZE)
                        .unwrap();
                memory_map[i].length -= (n * Size4KiB::SIZE) as u64;
                return Frame::starts_with(base)..(Frame::starts_with(base) + n);
            }
        }

        panic!("Failed to allocate from early memory map!");
    }

    fn free_n(&self, start: Frame, n: usize) {
        // TODO: we could put free entries back into a scratch entry, but for now we just leak freed frames
    }
}
