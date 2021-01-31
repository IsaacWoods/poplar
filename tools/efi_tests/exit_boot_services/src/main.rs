#![no_std]
#![no_main]
#![feature(abi_efiapi, never_type, panic_info_message, asm)]

use core::{cell::RefCell, fmt, fmt::Write, mem, ops::Range, panic::PanicInfo, slice};
use gfxconsole::{Bgr32, Format, Framebuffer, GfxConsole, Pixel};
use hal::memory::{Flags, Frame, FrameAllocator, FrameSize, PageTable, PhysicalAddress, Size4KiB, VirtualAddress};
use hal_x86_64::{hw::serial::SerialPort, paging::PageTableImpl};
use uefi::{
    prelude::*,
    proto::console::gop::GraphicsOutput,
    table::boot::{MemoryAttribute, MemoryDescriptor, MemoryType},
};

#[entry]
fn efi_main(handle: Handle, system_table: SystemTable<Boot>) -> Status {
    writeln!(system_table.stdout(), "Hello, World!").unwrap();
    let rsp: usize;
    unsafe {
        asm!("mov {}, rsp", out(reg) rsp);
    }
    writeln!(system_table.stdout(), "Stack pointer: {:#x}", rsp);

    /*
     * Get the framebuffer from the GOP driver.
     * XXX: rather than correctly detecting the pixel format, we've just chosen colors that will be the same on
     * both RGB32 and BGR32.
     */
    let (framebuffer_ptr, width, height, stride) = {
        let gop = system_table.boot_services().locate_protocol::<GraphicsOutput>().expect_success("No GOP");
        let gop = unsafe { &mut *gop.get() };
        let mode_info = gop.current_mode_info();
        let (width, height) = mode_info.resolution();
        let stride = mode_info.stride();

        (gop.frame_buffer().as_mut_ptr(), width, height, stride)
    };
    let mut gfx_console = GfxConsole::new(
        Framebuffer { ptr: framebuffer_ptr as *mut Pixel<_>, width, height, stride },
        Bgr32::pixel(0xff, 0x00, 0xff, 0xff),
        Bgr32::pixel(0xff, 0xff, 0xff, 0xff),
    );
    gfx_console.clear();
    writeln!(gfx_console, "Initialized GOP-based console!").unwrap();

    /*
     * Allocate memory to hold the memory map. We add space for 8 extra entries because doing this allocation can
     * retroactively change the memory map, making this not true anymore.
     */
    let memory_map_size = system_table.boot_services().memory_map_size() + 8 * mem::size_of::<MemoryDescriptor>();
    let memory_map_ptr =
        system_table.boot_services().allocate_pool(MemoryType::LOADER_DATA, memory_map_size)?.unwrap();
    let memory_map_slice = unsafe { slice::from_raw_parts_mut(memory_map_ptr, memory_map_size) };

    let (system_table, memory_map) = system_table.exit_boot_services(handle, memory_map_slice)?.unwrap();
    let mut logger = Logger::new(gfx_console);
    writeln!(logger, "Successfully exited boot services").unwrap();

    let allocator = Allocator::new(memory_map.clone().copied());
    let mut page_tables = PageTableImpl::new(allocator.allocate(), VirtualAddress::new(0x0));

    for entry in memory_map.clone() {
        if entry.att.contains(MemoryAttribute::RUNTIME)
            || entry.ty == MemoryType::LOADER_CODE
            || entry.ty == MemoryType::LOADER_DATA
        {
            writeln!(logger, "Identity mapping entry {:x?}", entry);
            page_tables
                .map_area(
                    VirtualAddress::new(entry.phys_start as usize),
                    PhysicalAddress::new(entry.phys_start as usize).unwrap(),
                    (entry.page_count as usize) * Size4KiB::SIZE,
                    // TODO: do these off the attributes?
                    Flags { writable: true, executable: true, user_accessible: false, cached: true },
                    &allocator,
                )
                .unwrap();
        } else {
            writeln!(logger, "Dropping entry: {:x?}", entry);
        }
    }

    writeln!(logger, "Switching to new page tables!").unwrap();
    // TODO: when we switch, we crash due to the UEFI stack??? For some reason, the stack of our image doesn't
    // actually appear in the memory map? UEFI...
    unsafe {
        page_tables.switch_to();
    }
    writeln!(logger, "Switched!").unwrap();
    loop {}
}

struct Allocator<M>
where
    M: ExactSizeIterator<Item = MemoryDescriptor> + Clone,
{
    memory_map: M,
    consuming: Option<MemoryDescriptor>,
}

impl<M> Allocator<M>
where
    M: ExactSizeIterator<Item = MemoryDescriptor> + Clone,
{
    pub fn new(memory_map: M) -> AllocatorCell<M> {
        AllocatorCell(RefCell::new(Allocator { memory_map, consuming: None }))
    }
}

struct AllocatorCell<M>(pub RefCell<Allocator<M>>)
where
    M: ExactSizeIterator<Item = MemoryDescriptor> + Clone;

impl<M> FrameAllocator<Size4KiB> for AllocatorCell<M>
where
    M: ExactSizeIterator<Item = MemoryDescriptor> + Clone,
{
    fn allocate(&self) -> Frame<Size4KiB> {
        while self.0.borrow().consuming.is_none() || self.0.borrow().consuming.as_ref().unwrap().page_count == 0 {
            let descriptor = self.0.borrow_mut().memory_map.next().unwrap();
            match descriptor.ty {
                MemoryType::CONVENTIONAL => self.0.borrow_mut().consuming = Some(descriptor),
                _ => continue,
            }
        }

        let mut guard = self.0.borrow_mut();
        let descriptor = guard.consuming.as_mut().unwrap();
        let frame_addr = descriptor.phys_start;
        descriptor.phys_start += Size4KiB::SIZE as u64;
        descriptor.page_count -= 1;

        Frame::starts_with(PhysicalAddress::new(frame_addr as usize).unwrap())
    }

    fn allocate_n(&self, _n: usize) -> Range<Frame<Size4KiB>> {
        unimplemented!()
    }

    fn free_n(&self, _start: Frame<Size4KiB>, _n: usize) {
        unimplemented!()
    }
}

pub struct Logger {
    console: GfxConsole<Bgr32>,
    serial_port: SerialPort,
}

impl Logger {
    pub fn new(console: GfxConsole<Bgr32>) -> Logger {
        let mut serial_port = unsafe { SerialPort::new(hal_x86_64::hw::serial::COM1) };
        unsafe {
            serial_port.initialise();
        }

        Logger { console, serial_port }
    }
}

impl fmt::Write for Logger {
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
        self.console.write_str(s)?;
        self.serial_port.write_str(s)?;
        Ok(())
    }
}

#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    /*
     * XXX: this is just a test, so we just spit the message out on the serial port, assuming it's initialized.
     * We're probably either still under UEFI, or have set up the serial port ourselves after exiting, so this is
     * fine.
     */
    let mut serial_port = unsafe { SerialPort::new(hal_x86_64::hw::serial::COM1) };

    if let Some(message) = info.message() {
        if let Some(location) = info.location() {
            writeln!(
                serial_port,
                "Panic message: {} ({} - {}:{})",
                message,
                location.file(),
                location.line(),
                location.column()
            );
        } else {
            writeln!(serial_port, "Panic message: {} (no location info)", message);
        }
    }
    loop {}
}
