/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use core::{str,mem,ptr};
use memory::{MemoryController,Frame,Page,FrameAllocator,EntryFlags};
use memory::paging::{PhysicalAddress,VirtualAddress,TemporaryPage};
use multiboot2::BootInformation;
use alloc::{boxed::Box,Vec,String};

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

        for byte in self.signature.iter()
        {
            sum += *byte as usize;
        }

        sum += self.checksum as usize;

        for byte in self.oem_id.iter()
        {
            sum += *byte as usize;
        }

        sum += self.revision as usize;

        for byte in unsafe { mem::transmute::<u32,[u8; 4]>(self.rsdt_address) }.iter()
        {
            sum += *byte as usize;
        }

        // Check that the lowest byte is 0
        match sum & 0b11111111
        {
            0 => Ok(()),
            _ => Err("Checksum is incorrect"),
        }
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

fn validate_sdt(table_ptr : *const SDTHeader, signature : &str) -> Result<(), &str>
{
    // Check the SDT's signature
    if unsafe { str::from_utf8_unchecked(&(*table_ptr).signature) } != signature
    {
        return Err("SDT has incorrect signature");
    }

    // Sum all of the bytes of the SDT
    let mut sum : usize = 0;
    let length = unsafe { *table_ptr }.length;
    println!("SDT length: {}", length);
    let bytes = table_ptr as *const u8;

    for i in 0..length
    {
        sum += unsafe { *(table_ptr as *const u8).offset(i as isize) } as usize;
    }

    // Check that the lower byte of the sum is 0
    if sum & 0b11111111 != 0
    {
        return Err("SDT has incorrect checksum");
    }

    Ok(())
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
    SCI_interrupt       : u16,
    SMI_command_port    : u32,
    acpi_enable         : u8,
    acpi_disable        : u8,
    S4BIOS_REQ          : u8,
    PSTATE_control      : u8,
    PM1a_event_block    : u32,
    PM1b_event_block    : u32,
    PM1a_control_block  : u32,
    PM1b_control_block  : u32,
    PM2_control_block   : u32,
    PM_timer_block      : u32,
    GPE0_block          : u32,
    GPE1_block          : u32,
    PM1_event_length    : u8,
    PM1_control_length  : u8,
    PM2_control_length  : u8,
    PM_timer_length     : u8,
    GPE0_length         : u8,
    GPE1_length         : u8,
    GPE1_base           : u8,
    CState_control      : u8,
    worst_C2_latency    : u16,
    worst_C3_latency    : u16,
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
    fn parse<A>(&'static self,
                memory_controller : &mut MemoryController<A>) -> Vec<(String, *mut SDTHeader)>
        where A : FrameAllocator
    {
        let num_tables = (self.header.length as usize - mem::size_of::<SDTHeader>()) / mem::size_of::<u32>();
        serial_println!("RSDT has pointers to {} SDTs", num_tables);

        let mut tables = Vec::<(String, *mut SDTHeader)>::with_capacity(num_tables);

        for i in 0..num_tables
        {
            let pointer_address = unsafe { (VirtualAddress::new(self as *const RSDT as usize).offset(mem::size_of::<SDTHeader>() as isize).ptr() as *const u32).offset(i as isize) };

            // TODO: map and extract each SDT and put it into the vector
            let physical_address = PhysicalAddress::new(unsafe { *pointer_address } as usize);
            let mut temporary_page = TemporaryPage::new(Page::get_containing_page(::memory::map::TEMP_PAGE), &mut memory_controller.frame_allocator);
            temporary_page.map(Frame::get_containing_frame(physical_address), &mut memory_controller.active_table);
            let virtual_address = ::memory::map::TEMP_PAGE.offset(physical_address.offset_into_frame() as isize);

            let sdt_pointer = virtual_address.ptr() as *const SDTHeader;

            /*
             * Find the signature of this SDT.
             * XXX: This can't be used to create the vector, its memory will be unmapped when the temporary page is!
             */
            let inplace_signature = unsafe { str::from_utf8_unchecked(&(*sdt_pointer).signature) };
            serial_println!("Found table: {} at {:#x}", inplace_signature, physical_address);

            match inplace_signature.as_ref()
            {
                "FACP" =>
                {
                    let fadt : Box<FADT> = unsafe { Box::new(ptr::read_unaligned(sdt_pointer as *const FADT)) };
                    tables.push((String::from(unsafe { str::from_utf8_unchecked(&fadt.header.signature) }),
                                 Box::into_raw(fadt) as *mut SDTHeader));
                },

                _      => println!("Unknown table: {}", inplace_signature),
            }

            temporary_page.unmap(&mut memory_controller.active_table);
        }

        tables
    }
}

#[derive(Clone,Debug)]
pub struct AcpiInfo
{
    rsdp    : &'static RSDP,

    /*
     * XXX: To allow storage of any type of table, we convert from a Box -> the wrapped raw pointer.
     *      To avoid leaking the memory, these must be converted back and dropped correctly when this
     *      is dropped.
     */
    tables  : Vec<(String, *mut SDTHeader)>,
}

impl AcpiInfo
{
    pub fn new<A>(boot_info            : &BootInformation,
                  memory_controller    : &mut MemoryController<A>) -> AcpiInfo
        where A : FrameAllocator
    {
        let rsdp : &'static RSDP = boot_info.rsdp().expect("Couldn't find RSDP tag").rsdp();
        rsdp.validate().unwrap();

        let oem_str = unsafe { str::from_utf8_unchecked(&rsdp.oem_id) };
        serial_println!("Loading ACPI tables with OEM ID: {}", oem_str);
        println!("RSDT physical address is: {:#x}", rsdp.rsdt_address);

        let rsdt_address = PhysicalAddress::new(rsdp.rsdt_address as usize);
        memory_controller.active_table.map_to(Page::get_containing_page(::memory::map::RSDT_ADDRESS),
                                              Frame::get_containing_frame(rsdt_address),
                                              EntryFlags::PRESENT,
                                              &mut memory_controller.frame_allocator);

        let rsdt_ptr = (::memory::map::RSDT_ADDRESS.offset(rsdt_address.offset_into_frame() as isize)).ptr() as *const SDTHeader;
        validate_sdt(rsdt_ptr, "RSDT").unwrap();

        let rsdt : &'static RSDT = unsafe { &*(rsdt_ptr as *const RSDT) };
        let tables = rsdt.parse(memory_controller);

        AcpiInfo
        {
            rsdp,
            tables,
        }
    }

    fn table(&self, signature : &str) -> Option<*const SDTHeader>
    {
        self.tables.iter().find(|table| table.0 == signature).map(|table| table.1 as *const SDTHeader)
    }

    pub fn fadt(&self) -> Option<&FADT>
    {
        self.table("FACP").map(|table| unsafe { &*(table as *const FADT) })
    }
}

impl Drop for AcpiInfo
{
    fn drop(&mut self)
    {
        serial_println!("Dropping ACPIInfo");
        for table in self.tables.iter()
        {
            match table.0.as_ref()
            {
                "FACP" => unsafe { ptr::drop_in_place(table.1 as *mut FADT) },
                _      => panic!("Found table with signature '{}' that is not handled during drop!", table.0),
            };
        }
    }
}
