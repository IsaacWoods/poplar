use bit_field::BitField;
use core::marker::PhantomData;

pub type VendorId = u16;
pub type DeviceId = u16;

#[derive(Clone, Copy, Debug)]
pub struct PciAddress {
    pub segment: u16,
    pub bus: u8,
    pub device: u8,
    pub function: u8,
}

pub trait ConfigRegionAccess {
    fn function_exists(&self, address: PciAddress) -> bool;
    unsafe fn read(&self, address: PciAddress, offset: u16) -> u32;
    unsafe fn write(&self, address: PciAddress, offset: u16, value: u32);
}

/// Every PCI configuration region starts with a header made up of two parts:
///    - a predefined region that identify the function (bytes `0x00..0x10`)
///    - a device-dependent region that depends on the Header Type field
///
/// The predefined region is of the form:
/// ```ignore
///     32                            16                              0
///      +-----------------------------+------------------------------+
///      |       Device ID             |       Vendor ID              | 0x00
///      |                             |                              |
///      +-----------------------------+------------------------------+
///      |         Status              |       Command                | 0x04
///      |                             |                              |
///      +-----------------------------+---------------+--------------+
///      |               Class Code                    |   Revision   | 0x08
///      |                                             |      ID      |
///      +--------------+--------------+---------------+--------------+
///      |     BIST     |    Header    |    Latency    |  Cacheline   | 0x0c
///      |              |     type     |     timer     |    size      |
///      +--------------+--------------+---------------+--------------+
/// ```
///
/// Endpoints have a Type-0 header, so the remainder of the header is of the form:
/// ```ignore
///     32                           16                              0
///     +-----------------------------------------------------------+
///     |                  Base Address Register 0                  | 0x10
///     |                                                           |
///     +-----------------------------------------------------------+
///     |                  Base Address Register 1                  | 0x14
///     |                                                           |
///     +-----------------------------------------------------------+
///     |                  Base Address Register 2                  | 0x18
///     |                                                           |
///     +-----------------------------------------------------------+
///     |                  Base Address Register 3                  | 0x1c
///     |                                                           |
///     +-----------------------------------------------------------+
///     |                  Base Address Register 4                  | 0x20
///     |                                                           |
///     +-----------------------------------------------------------+
///     |                  Base Address Register 5                  | 0x24
///     |                                                           |
///     +-----------------------------------------------------------+
///     |                  CardBus CIS Pointer                      | 0x28
///     |                                                           |
///     +----------------------------+------------------------------+
///     |       Subsystem ID         |    Subsystem vendor ID       | 0x2c
///     |                            |                              |
///     +----------------------------+------------------------------+
///     |               Expansion ROM Base Address                  | 0x30
///     |                                                           |
///     +--------------------------------------------+--------------+
///     |                 Reserved                   | Capabilities | 0x34
///     |                                            |   Pointer    |
///     +--------------------------------------------+--------------+
///     |                         Reserved                          | 0x38
///     |                                                           |
///     +--------------+--------------+--------------+--------------+
///     |   Max_Lat    |   Min_Gnt    |  Interrupt   |  Interrupt   | 0x3c
///     |              |              |   line       |   line       |
///     +--------------+--------------+--------------+--------------+
/// ```
pub struct PciHeader<A>(PciAddress, PhantomData<A>)
where
    A: ConfigRegionAccess;

impl<A> PciHeader<A>
where
    A: ConfigRegionAccess,
{
    pub fn new(address: PciAddress) -> PciHeader<A> {
        PciHeader(address, PhantomData)
    }

    pub fn id(&self, access: &A) -> (VendorId, DeviceId) {
        let id = unsafe { access.read(self.0, 0x00) };
        (id.get_bits(0..16) as u16, id.get_bits(16..32) as u16)
    }

    pub fn header_type(&self, access: &A) -> u8 {
        /*
         * Read bits 0..=6 of the Header Type. Bit 7 dictates whether the device has multiple functions and so
         * isn't read here.
         */
        unsafe { access.read(self.0, 0x0c) }.get_bits(16..23) as u8
    }

    pub fn has_multiple_functions(&self, access: &A) -> bool {
        /*
         * Reads bit 7 of the Header Type, which is 1 if the device has multiple functions.
         */
        unsafe { access.read(self.0, 0x0c) }.get_bit(23)
    }
}
