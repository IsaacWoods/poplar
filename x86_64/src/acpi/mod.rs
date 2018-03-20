/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

mod fadt;
mod madt;

use core::{str,mem,ptr};
use ::Platform;
use memory::{Frame,FrameAllocator};
use memory::paging::{PhysicalAddress,VirtualAddress,TemporaryPage};
use multiboot2::BootInformation;
use alloc::boxed::Box;
use self::fadt::Fadt;

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
     * run-time splice without messing up the representation. The actual number of tables here is:
     * `(header.length - size_of::<SDTHeader>) / 4`
     */
    tables  : [u32; 8],
}

impl Rsdt
{
    fn parse<A>(&self,
                acpi_info   : &mut AcpiInfo,
                platform    : &mut Platform<A>)
        where A : FrameAllocator
    {
        use ::memory::map::TEMP_PAGE;

        let num_tables = (self.header.length as usize - mem::size_of::<SdtHeader>()) / mem::size_of::<u32>();
        let table_base_ptr = VirtualAddress::new(self as *const Rsdt as usize).offset(mem::size_of::<SdtHeader>() as isize).ptr() as *const u32;

        for i in 0..num_tables
        {
            let pointer_address = unsafe { table_base_ptr.offset(i as isize) };
            let physical_address = PhysicalAddress::new(unsafe { *pointer_address } as usize);
            let mut temporary_page = TemporaryPage::new(TEMP_PAGE, &mut platform.memory_controller.frame_allocator);
            temporary_page.map(Frame::containing_frame(physical_address),
                               &mut platform.memory_controller.kernel_page_table);

            let sdt_pointer = TEMP_PAGE.start_address().offset(physical_address.offset_into_frame() as isize).ptr() as *const SdtHeader;
            let signature = unsafe { str::from_utf8_unchecked(&(*sdt_pointer).signature) };

            match unsafe { &(*sdt_pointer).signature }
            {
                b"FACP" => fadt::parse_fadt(sdt_pointer, acpi_info),
                b"APIC" => madt::parse_madt(sdt_pointer, acpi_info, platform),
                _       => warn!("Unhandled SDT type: {}", signature),
            }

            temporary_page.unmap(&mut platform.memory_controller.kernel_page_table);
        }
    }
}

#[derive(Clone,Debug)]
pub struct AcpiInfo
{
    pub rsdp        : &'static Rsdp,
    pub rsdt        : Option<Box<Rsdt>>,
    pub fadt        : Option<Box<Fadt>>,
}

impl AcpiInfo
{
    pub fn new<A>(boot_info : &BootInformation,
                  platform  : &mut Platform<A>) -> AcpiInfo
        where A : FrameAllocator
    {
        use ::memory::map::TEMP_PAGE;

        let rsdp : &'static Rsdp = boot_info.rsdp().expect("Couldn't find RSDP tag").rsdp();
        rsdp.validate().unwrap();

        trace!("Loading ACPI tables with OEM ID: {}", rsdp.oem_str());
        let physical_address = PhysicalAddress::new(rsdp.rsdt_address as usize);
        let mut temporary_page = TemporaryPage::new(TEMP_PAGE, &mut platform.memory_controller.frame_allocator);
        temporary_page.map(Frame::containing_frame(physical_address),
                           &mut platform.memory_controller.kernel_page_table);
        let rsdt_ptr = (TEMP_PAGE.start_address().offset(physical_address.offset_into_frame() as isize)).ptr() as *const SdtHeader;

        let rsdt : Box<Rsdt> = unsafe { Box::new(ptr::read_unaligned(rsdt_ptr as *const Rsdt)) };
        rsdt.header.validate("RSDT").unwrap();
        temporary_page.unmap(&mut platform.memory_controller.kernel_page_table);

        let mut acpi_info = AcpiInfo
                            {
                                rsdp,
                                rsdt        : None,
                                fadt        : None,
                            };

        rsdt.parse(&mut acpi_info, platform);
        acpi_info.rsdt = Some(rsdt);

        acpi_info
    }
}
