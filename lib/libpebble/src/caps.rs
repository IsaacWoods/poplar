#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Capability {
    /*
     * Capabilities related to kernel objects.
     */
    CreateAddressSpace,
    CreateMemoryObject,
    CreateTask,

    /*
     * Capabilities specific to tasks running on x86_64.
     */
    X86_64AccessIoPort(u16),

    /*
     * Capabilities that are owned by drivers / support services.
     */
    GetFramebuffer,
    EarlyLogging,
}

pub const CAP_PADDING: u8 = 0x00;
pub const CAP_CREATE_ADDRESS_SPACE: u8 = 0x01;
pub const CAP_CREATE_MEMORY_OBJECT: u8 = 0x02;
pub const CAP_CREATE_TASK: u8 = 0x03;

pub const CAP_BASE_X86_64: u8 = 0x20;

pub const CAP_GET_FRAMEBUFFER: u8 = 0x30;
pub const CAP_EARLY_LOGGING: u8 = 0x31;

/// `N` must be a multiple of 4, and padded with zeros, so the whole descriptor is aligned to a
/// 4-byte boundary.
///
/// This structure can be used to emit an ELF note section containing a list of capabilities. `N` must be a
/// multiple of 4 (padded with `CAP_PADDING`). You can define the capabilities of a task image like so:
/// ```
/// #[used]
/// #[link_section = ".caps"]
/// pub static mut CAPS: CapabilitiesRepr<4> = CapabilitiesRepr::new([CAP_EARLY_LOGGING,
/// CAP_GET_FRAMEBUFFER, CAP_PADDING, CAP_PADDING]);
/// ```
#[repr(C)]
pub struct CapabilitiesRepr<const N: usize> {
    name_size: u32,
    desc_size: u32,
    entry_type: u32,
    name: [u8; 8],
    desc: [u8; N],
}

impl<const N: usize> CapabilitiesRepr<{ N }> {
    pub const fn new(caps: [u8; N]) -> CapabilitiesRepr<{ N }> {
        CapabilitiesRepr {
            name_size: 6,
            desc_size: N as u32,
            entry_type: 0,
            name: [b'P', b'E', b'B', b'B', b'L', b'E', b'\0', 0x00],
            desc: caps,
        }
    }
}
