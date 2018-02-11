/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use core::{str,mem,ptr};
use memory::{MemoryController,Frame,Page,FrameAllocator,EntryFlags};
use memory::paging::{PhysicalAddress,VirtualAddress,TemporaryPage};
use multiboot2::BootInformation;
use alloc::boxed::Box;

/*
 * The RSDP (Root System Descriptor Pointer) is the first ACPI structure located.
 */
#[derive(Clone,Copy,Debug)]
#[repr(packed)]
pub struct RSDP
{
    pub(self) signature       : [u8; 8],
    pub(self) checksum        : u8,
    pub(self) oem_id          : [u8; 6],
    pub(self) revision        : u8,
    pub(self) rsdt_address    : u32,
}

impl RSDP
{
    fn validate(&self) -> Result<(), &str>
    {
        // Check the RSDP's signature (should be "RSD PTR ")
        if unsafe { str::from_utf8_unchecked(&self.signature) } != "RSD PTR "
        {
            return Err("RSDP has incorrect signature");
        }

        let mut sum : usize  = 0;

        for i in 0..mem::size_of::<RSDP>()
        {
            sum += unsafe { *(self as *const RSDP as *const u8).offset(i as isize) } as usize;
        }

        // Check that the lowest byte is 0
        if sum & 0b11111111 != 0
        {
            return Err("RSDP has incorrect checksum");
        }

        Ok(())
    }
}

/*
 * This is the layout of the header for all of the System Descriptor Tables.
 * XXX: This will be followed by variable amounts of data, depending on which SDT this is.
 */
#[derive(Clone,Copy,Debug)]
#[repr(packed)]
struct SDTHeader
{
    signature           : [u8; 4],
    length              : u32,
    revision            : u8,
    checksum            : u8,
    oem_id              : [u8; 6],
    oem_table_id        : [u8; 8],
    oem_revision        : u32,
    creator_id          : u32,
    creator_revision    : u32,
    // ...
}

impl SDTHeader
{
    fn validate(&self, signature : &str) -> Result<(), &str>
    {
        // Check the signature
        if unsafe { str::from_utf8_unchecked(&self.signature) } != signature
        {
            return Err("SDT has incorrect signature");
        }

        let mut sum : usize  = 0;

        for i in 0..self.length
        {
            sum += unsafe { *(self as *const SDTHeader as *const u8).offset(i as isize) } as usize;
        }

        // Check that the lowest byte is 0
        if sum & 0b11111111 != 0
        {
            return Err("SDT has incorrect checksum");
        }

        Ok(())
    }
}

#[derive(Clone,Copy,Debug)]
#[repr(packed)]
struct AddressStructure
{
    address_space   : u8,
    bit_width       : u8,
    bit_offset      : u8,
    access_size     : u8,
    address         : u64,
}

#[derive(Clone,Copy,Debug)]
#[repr(packed)]
pub struct FADT
{
    header              : SDTHeader,
    firmware_ctrl       : u32,
    dsdt_address        : u32,
    reserved_1          : u8,
    power_profile       : u8,
    sci_interrupt       : u16,
    smi_command_port    : u32,
    acpi_enable         : u8,
    acpi_disable        : u8,
    s4bios_req          : u8,
    pstate_control      : u8,
    pm1a_event_block    : u32,
    pm1b_event_block    : u32,
    pm1a_control_block  : u32,
    pm1b_control_block  : u32,
    pm2_control_block   : u32,
    pm_timer_block      : u32,
    gpe0_block          : u32,
    gpe1_block          : u32,
    pm1_event_length    : u8,
    pm1_control_length  : u8,
    pm2_control_length  : u8,
    pm_timer_length     : u8,
    gpe0_length         : u8,
    gpe1_length         : u8,
    gpe1_base           : u8,
    cstate_control      : u8,
    worst_c2_latency    : u16,
    worst_c3_latency    : u16,
    flush_size          : u16,
    flush_stride        : u16,
    duty_offset         : u8,
    duty_width          : u8,
    day_alarm           : u8,
    month_alarm         : u8,
    century             : u8,

    reserved_2          : [u8; 3],
    flags               : u32,
    reset_reg           : AddressStructure,
    reset_value         : u8,
    reserved_3          : [u8; 3],
}

#[derive(Clone,Copy,Debug)]
#[repr(packed)]
struct RSDT
{
    header  : SDTHeader,
    /*
     * There may be less/more than 8 tables, but there isn't really a good way of representing a
     * run-time splice without messing up the representation. The actual number of tables here is:
     * `(header.length - size_of::<SDTHeader)> / 4`
     */
    tables  : [u32; 8],
}

impl RSDT
{
    fn parse<A>(&self,
                acpi_info           : &mut AcpiInfo,
                memory_controller   : &mut MemoryController<A>)
        where A : FrameAllocator
    {
        use ::memory::map::TEMP_PAGE;

        let num_tables = (self.header.length as usize - mem::size_of::<SDTHeader>()) / mem::size_of::<u32>();
        serial_println!("RSDT has pointers to {} SDTs", num_tables);

        let table_base_ptr = VirtualAddress::new(self as *const RSDT as usize).offset(mem::size_of::<SDTHeader>() as isize).ptr() as *const u32;

        for i in 0..num_tables
        {
            let pointer_address = unsafe { table_base_ptr.offset(i as isize) };
            let physical_address = PhysicalAddress::new(unsafe { *pointer_address } as usize);
            let mut temporary_page = TemporaryPage::new(TEMP_PAGE, &mut memory_controller.frame_allocator);
            temporary_page.map(Frame::get_containing_frame(physical_address), &mut memory_controller.active_table);

            let sdt_pointer = TEMP_PAGE.start_address().offset(physical_address.offset_into_frame() as isize).ptr() as *const SDTHeader;
            let signature = unsafe { str::from_utf8_unchecked(&(*sdt_pointer).signature) };
            serial_println!("Found SDT: {} at {:#x}", signature, physical_address);

            match signature.as_ref()
            {
                "FACP" =>
                {
                    let fadt : Box<FADT> = unsafe { Box::new(ptr::read_unaligned(sdt_pointer as *const FADT)) };
                    fadt.header.validate("FACP").unwrap();
                    acpi_info.fadt = Some(fadt);
                },

                _      => println!("Unknown table: {}", signature),
            }

            temporary_page.unmap(&mut memory_controller.active_table);
        }
    }
}

#[derive(Clone,Debug)]
pub struct AcpiInfo
{
    pub(self) rsdp    : &'static RSDP,
    pub(self) rsdt    : Box<RSDT>,

    pub(self) fadt    : Option<Box<FADT>>,
}

impl AcpiInfo
{
    pub fn new<A>(boot_info            : &BootInformation,
                  memory_controller    : &mut MemoryController<A>) -> AcpiInfo
        where A : FrameAllocator
    {
        use ::memory::map::TEMP_PAGE;

        let rsdp : &'static RSDP = boot_info.rsdp().expect("Couldn't find RSDP tag").rsdp();
        rsdp.validate().unwrap();

        let oem_str = unsafe { str::from_utf8_unchecked(&rsdp.oem_id) };
        serial_println!("Loading ACPI tables with OEM ID: {}", oem_str);

        let physical_address = PhysicalAddress::new(rsdp.rsdt_address as usize);
        let mut temporary_page = TemporaryPage::new(TEMP_PAGE, &mut memory_controller.frame_allocator);
        temporary_page.map(Frame::get_containing_frame(physical_address), &mut memory_controller.active_table);
        let rsdt_ptr = (TEMP_PAGE.start_address().offset(physical_address.offset_into_frame() as isize)).ptr() as *const SDTHeader;

        let rsdt : Box<RSDT> = unsafe { Box::new(ptr::read_unaligned(rsdt_ptr as *const RSDT)) };
        rsdt.header.validate("RSDT").unwrap();
        temporary_page.unmap(&mut memory_controller.active_table);

        let mut acpi_info = AcpiInfo
                            {
                                rsdp,
                                rsdt    : unsafe { mem::uninitialized() },
                                fadt    : None,
                            };

        rsdt.parse(&mut acpi_info, memory_controller);
        acpi_info.rsdt = rsdt;

        acpi_info
    }
}
