/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

mod fadt;
mod madt;
mod dsdt;

use core::{str,mem};
use Platform;
use memory::{Frame,FrameAllocator};
use memory::paging::{PhysicalAddress,VirtualAddress,TemporaryPage,PhysicalMapping,EntryFlags};
use multiboot2::BootInformation;
use self::fadt::Fadt;
use self::madt::MadtHeader;
use self::dsdt::Dsdt;

/*
 * The RSDP (Root System Descriptor Pointer) is the first ACPI structure located.
 */
#[derive(Clone,Copy,Debug)]
#[repr(packed)]
pub struct Rsdp
{
    pub(self) signature       : [u8; 8],
    pub(self) checksum        : u8,
    pub(self) oem_id          : [u8; 6],
    pub(self) revision        : u8,
    pub(self) rsdt_address    : u32,
}

impl Rsdp
{
    fn validate(&self) -> Result<(), &str>
    {
        // Check the RSDP's signature (should be "RSD PTR ")
        if &self.signature != b"RSD PTR "
        {
            return Err("RSDP has incorrect signature");
        }

        let mut sum : usize  = 0;

        for i in 0..mem::size_of::<Rsdp>()
        {
            sum += unsafe { *(self as *const Rsdp as *const u8).offset(i as isize) } as usize;
        }

        // Check that the lowest byte is 0
        if sum & 0b11111111 != 0
        {
            return Err("RSDP has incorrect checksum");
        }

        Ok(())
    }

    fn oem_str<'a>(&'a self) -> &'a str
    {
        str::from_utf8(&self.oem_id).expect("Could not extract OEM ID from RSDP")
    }
}

/*
 * This is the layout of the header for all of the System Descriptor Tables.
 * XXX: This will be followed by variable amounts of data, depending on which SDT this is.
 */
#[derive(Clone,Copy,Debug)]
#[repr(packed)]
struct SdtHeader
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

impl SdtHeader
{
    fn validate(&self, signature : &str) -> Result<(), &str>
    {
        // Check the signature
        let table_signature = match str::from_utf8(&self.signature)
                              {
                                  Ok(signature) => signature,
                                  _ => return Err("SDT has incorrect signature"),
                              };
        if table_signature != signature
        {
            return Err("SDT has incorrect signature");
        }

        let mut sum : usize  = 0;

        for i in 0..self.length
        {
            sum += unsafe { *(self as *const SdtHeader as *const u8).offset(i as isize) } as usize;
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
pub struct Rsdt
{
    header  : SdtHeader,
    /*
     * There may be less/more than 8 tables, but there isn't really a good way of representing a
     * run-time slice without messing up the representation.
     * The actual number of tables here is: `(header.length - size_of::<SDTHeader>) / 4`
     */
    tables  : [u32; 8],
}

/// This temporarily maps a SDT to get its signature and length, then unmaps it
/// It's used to calculate the size we need to actually map
unsafe fn peek_at_table<A>(table_address    : PhysicalAddress,
                           platform         : &mut Platform<A>) -> ([u8; 4], u32)
    where A : FrameAllocator
{
    use ::memory::map::TEMP_PAGE;

    let signature   : [u8; 4];
    let length      : u32;

    {
        let mut temporary_page = TemporaryPage::new(TEMP_PAGE, &mut platform.memory_controller.frame_allocator);
        temporary_page.map(Frame::containing_frame(table_address), &mut platform.memory_controller.kernel_page_table);
        let sdt_pointer = TEMP_PAGE.start_address().offset(table_address.offset_into_frame() as isize).ptr() as *const SdtHeader;

        signature = (*sdt_pointer).signature;
        length = (*sdt_pointer).length;

        temporary_page.unmap(&mut platform.memory_controller.kernel_page_table);
    }

    (signature, length)
}

fn parse_rsdt<A>(acpi_info : &mut AcpiInfo, platform : &mut Platform<A>)
    where A : FrameAllocator
{
    let num_tables = (acpi_info.rsdt.header.length as usize - mem::size_of::<SdtHeader>()) / mem::size_of::<u32>();
    let table_base_ptr = VirtualAddress::from(acpi_info.rsdt.ptr).offset(mem::size_of::<SdtHeader>() as isize).ptr() as *const u32;

    for i in 0..num_tables
    {
        let pointer_address = unsafe { table_base_ptr.offset(i as isize) };
        let table_address = PhysicalAddress::new(unsafe { *pointer_address } as usize);
        let (signature, length) = unsafe { peek_at_table(table_address, platform) };

        match &signature
        {
            b"FACP" =>
            {
                let fadt_mapping = platform.memory_controller
                                           .kernel_page_table
                                           .map_physical_region::<Fadt, A>(table_address,
                                                                           table_address.offset(length as isize),
                                                                           EntryFlags::PRESENT,
                                                                           &mut platform.memory_controller.frame_allocator);
                (*fadt_mapping).header.validate("FACP").unwrap();
                let dsdt_address = PhysicalAddress::from((*fadt_mapping).dsdt_address as usize);
                acpi_info.fadt = Some(fadt_mapping);

                // Now we have the FADT, we can map and parse the DSDT
                let (_, dsdt_length) = unsafe { peek_at_table(dsdt_address, platform) };
                let dsdt_mapping = platform.memory_controller
                                           .kernel_page_table
                                           .map_physical_region::<Dsdt, A>(dsdt_address,
                                                                           dsdt_address.offset(dsdt_length as isize),
                                                                           EntryFlags::PRESENT,
                                                                           &mut platform.memory_controller.frame_allocator);
                (*dsdt_mapping).header.validate("DSDT").unwrap();
                dsdt::parse_dsdt(&dsdt_mapping, acpi_info);
            },

            b"APIC" =>
            {
                let madt_mapping = platform.memory_controller
                                           .kernel_page_table
                                           .map_physical_region::<MadtHeader, A>(table_address,
                                                                                 table_address.offset(length as isize),
                                                                                 EntryFlags::PRESENT,
                                                                                 &mut platform.memory_controller.frame_allocator);
                (*madt_mapping).header.validate("APIC").unwrap();
                madt::parse_madt(&madt_mapping, platform);
            },

            _ =>
            {
                match str::from_utf8(&signature)
                {
                    Ok(signature_str) => warn!("Unhandled SDT type: {}", signature_str),
                    Err(_) => error!("Unhandled SDT type; signature is not valid!"),
                }
            },
        }
    }
}

#[derive(Clone,Debug)]
pub struct AcpiInfo
{
    pub rsdp        : &'static Rsdp,
    pub rsdt        : PhysicalMapping<Rsdt>,
    pub fadt        : Option<PhysicalMapping<Fadt>>,
    pub dsdt        : Option<PhysicalMapping<Dsdt>>,
}

impl AcpiInfo
{
    pub fn new<A>(boot_info : &BootInformation,
                  platform  : &mut Platform<A>) -> Option<AcpiInfo>
        where A : FrameAllocator
    {
        let rsdp : &'static Rsdp = boot_info.rsdp().expect("Couldn't find RSDP tag").rsdp();
        let rsdt_address = PhysicalAddress::from(rsdp.rsdt_address as usize);
        rsdp.validate().unwrap();

        trace!("Loading ACPI tables with OEM ID: {}", rsdp.oem_str());
        let (rsdt_signature, rsdt_length) = unsafe { peek_at_table(rsdt_address, platform) };

        if &rsdt_signature != b"RSDT"
        {
            return None;
        }

        let rsdt_mapping = platform.memory_controller
                                   .kernel_page_table
                                   .map_physical_region::<Rsdt, A>(rsdt_address,
                                                                   rsdt_address.offset(rsdt_length as isize),
                                                                   EntryFlags::PRESENT,
                                                                   &mut platform.memory_controller.frame_allocator);

        let mut acpi_info = AcpiInfo
                            {
                                rsdp,
                                rsdt    : rsdt_mapping,
                                fadt    : None,
                                dsdt    : None,
                            };

        (*acpi_info.rsdt).header.validate("RSDT").unwrap();
        parse_rsdt(&mut acpi_info, platform);

        Some(acpi_info)
    }
}
