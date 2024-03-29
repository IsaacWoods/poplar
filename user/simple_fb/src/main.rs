use gfxconsole::{Bgr32, Format, Framebuffer, Pixel};
use log::info;
use std::{
    mem::MaybeUninit,
    poplar::{
        caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_GET_FRAMEBUFFER, CAP_PADDING, CAP_SERVICE_USER},
        early_logger::EarlyLogger,
        syscall::{self, FramebufferInfo, PixelFormat},
        Handle,
    },
};

pub fn main() {
    log::set_logger(&EarlyLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("Simple framebuffer driver is running!");

    let framebuffer = make_framebuffer();
    let mut yields = 0;

    loop {
        framebuffer.clear(Bgr32::pixel(0xaa, 0xaa, 0xaa, 0xff));
        framebuffer.draw_string(
            &format!("The framebuffer driver has yielded {} times!", yields),
            400,
            400,
            Bgr32::pixel(0xff, 0x00, 0xff, 0xff),
        );
        yields += 1;

        info!("Yielding from FB");
        syscall::yield_to_kernel();
    }
}

fn make_framebuffer() -> Framebuffer<Bgr32> {
    /*
     * This is the virtual address the framebuffer will be mapped to in our address space.
     * NOTE: this address was basically pulled out of thin air.
     */
    const FRAMEBUFFER_ADDRESS: usize = 0x00000005_00000000;

    let (framebuffer_handle, framebuffer_info) = {
        let mut framebuffer_info: MaybeUninit<FramebufferInfo> = MaybeUninit::uninit();

        let framebuffer_handle =
            syscall::get_framebuffer(framebuffer_info.as_mut_ptr()).expect("Failed to get handle to framebuffer!");

        (framebuffer_handle, unsafe { framebuffer_info.assume_init() })
    };

    unsafe {
        syscall::map_memory_object(framebuffer_handle, Handle::ZERO, Some(FRAMEBUFFER_ADDRESS), 0x0 as *mut _)
            .unwrap();
    }
    assert_eq!(framebuffer_info.pixel_format, PixelFormat::Bgr32);

    Framebuffer {
        ptr: FRAMEBUFFER_ADDRESS as *mut Pixel<Bgr32>,
        width: framebuffer_info.width as usize,
        height: framebuffer_info.height as usize,
        stride: framebuffer_info.stride as usize,
    }
}

#[used]
#[link_section = ".caps"]
pub static mut CAPS: CapabilitiesRepr<4> =
    CapabilitiesRepr::new([CAP_EARLY_LOGGING, CAP_GET_FRAMEBUFFER, CAP_SERVICE_USER, CAP_PADDING]);
