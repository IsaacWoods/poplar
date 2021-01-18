#![no_std]
#![no_main]
#![feature(abi_efiapi, never_type)]

use core::{fmt::Write, mem, ops::Range, panic::PanicInfo, slice};
use gfxconsole::{Bgr32, Format, Framebuffer, GfxConsole, Pixel};
use hal::memory::{Frame, FrameAllocator, Size4KiB};
use uefi::{
    prelude::*,
    proto::console::gop::GraphicsOutput,
    table::boot::{MemoryDescriptor, MemoryType},
};

#[entry]
fn efi_main(handle: Handle, system_table: SystemTable<Boot>) -> Status {
    writeln!(system_table.stdout(), "Hello, World!").unwrap();

    /*
     * Get the framebuffer from the GOP driver.
     * XXX: rather than correctly detecting the pixel format, we've just chosen colors that will be the same on
     * both RGB32 and BGR32.
     */
    let gop = system_table.boot_services().locate_protocol::<GraphicsOutput>().expect_success("No GOP");
    let gop = unsafe { &mut *gop.get() };
    let mode_info = gop.current_mode_info();
    let (width, height) = mode_info.resolution();
    let stride = mode_info.stride();
    let mut gfx_console = GfxConsole::new(
        Framebuffer { ptr: gop.frame_buffer().as_mut_ptr() as *mut Pixel<_>, width, height, stride },
        Bgr32::pixel(0xff, 0x00, 0xff, 0xff),
        Bgr32::pixel(0xff, 0xff, 0xff, 0xff),
    );
    gfx_console.clear();
    writeln!(gfx_console, "Initialized GOP-based console!\n").unwrap();

    /*
     * Allocate memory to hold the memory map. We add space for 8 extra entries because doing this allocation can
     * retroactively change the memory map, making this not true anymore.
     */
    let memory_map_size = system_table.boot_services().memory_map_size() + 8 * mem::size_of::<MemoryDescriptor>();
    let memory_map_ptr =
        system_table.boot_services().allocate_pool(MemoryType::LOADER_DATA, memory_map_size)?.unwrap();
    let memory_map_slice = unsafe { slice::from_raw_parts_mut(memory_map_ptr, memory_map_size) };

    let (system_table, memory_map) = system_table.exit_boot_services(handle, memory_map_slice)?.unwrap();
    let memory_map = UefiMemoryMap::new(memory_map);

    writeln!(gfx_console, "Successfully exited boot services").unwrap();

    loop {}
}

struct UefiMemoryMap<'a, M>
where
    M: ExactSizeIterator<Item = &'a MemoryDescriptor> + Clone,
{
    memory_map: M,
}

impl<'a, M> UefiMemoryMap<'a, M>
where
    M: ExactSizeIterator<Item = &'a MemoryDescriptor> + Clone,
{
    pub fn new(memory_map: M) -> UefiMemoryMap<'a, M> {
        UefiMemoryMap { memory_map }
    }
}

impl<'a, M> FrameAllocator<Size4KiB> for &mut UefiMemoryMap<'a, M>
where
    M: ExactSizeIterator<Item = &'a MemoryDescriptor> + Clone,
{
    fn allocate(&self) -> Frame<Size4KiB> {
        todo!()
    }

    fn allocate_n(&self, _n: usize) -> Range<Frame<Size4KiB>> {
        unimplemented!()
    }

    fn free_n(&self, _start: Frame<Size4KiB>, _n: usize) {
        unimplemented!()
    }
}

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    loop {}
}
