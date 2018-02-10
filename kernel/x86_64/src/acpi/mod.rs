/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use core::{str,mem};
use memory::{MemoryController,Frame,Page,FrameAllocator,EntryFlags};
use memory::paging::{PhysicalAddress,VirtualAddress};
use multiboot2::BootInformation;
use alloc::Vec;

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

#[derive(Debug)]
#[repr(packed)]
struct RSDT
{
    header  : SDTHeader,
    /*
     * There may not be 8 tables here, but until Rust gets real type-level literals, this is a
     * massive PITA. The actual number of tables here is `(header.length - size_of::<SDTHeader)> / 4`
     */
    tables  : [u32; 8],
}

impl RSDT
{
    fn parse(&'static self)// -> Vec<*const SDTHeader>
    {
        let num_tables = (self.header.length as usize - mem::size_of::<SDTHeader>()) / mem::size_of::<u32>();
        serial_println!("RSDT has pointers to {} SDTs", num_tables);

        for i in 0..num_tables
        {
            let pointer_address = unsafe { (VirtualAddress::new(self as *const RSDT as usize).offset(mem::size_of::<SDTHeader>() as isize).ptr() as *const u32).offset(i as isize) };
            serial_println!("Found SDT: {:#x}", unsafe { *pointer_address });
        }
    }
}

#[derive(Clone,Copy,Debug)]
pub struct AcpiInfo
{
    rsdp    : &'static RSDP
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
        rsdt.parse();

        AcpiInfo
        {
            rsdp : rsdp,
        }
    }
}
