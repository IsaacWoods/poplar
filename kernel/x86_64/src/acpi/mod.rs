/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

mod fadt;
mod madt;

use core::{str,mem,ptr};
use memory::{MemoryController,Frame,FrameAllocator};
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
        if unsafe { str::from_utf8_unchecked(&self.signature) } != "RSD PTR "
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
        unsafe { str::from_utf8_unchecked(&self.oem_id) }
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
        if unsafe { str::from_utf8_unchecked(&self.signature) } != signature
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
            println!("SDT has incorrect final sum: {} (checksum: {}) (length: {}={:#x})", sum, self.checksum, self.length, self.length);
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
                acpi_info           : &mut AcpiInfo,
                memory_controller   : &mut MemoryController<A>)
        where A : FrameAllocator
    {
        use ::memory::map::TEMP_PAGE;

        let num_tables = (self.header.length as usize - mem::size_of::<SdtHeader>()) / mem::size_of::<u32>();
        serial_println!("RSDT has pointers to {} SDTs", num_tables);

        let table_base_ptr = VirtualAddress::new(self as *const Rsdt as usize).offset(mem::size_of::<SdtHeader>() as isize).ptr() as *const u32;

        for i in 0..num_tables
        {
            let pointer_address = unsafe { table_base_ptr.offset(i as isize) };
            let physical_address = PhysicalAddress::new(unsafe { *pointer_address } as usize);
            let mut temporary_page = TemporaryPage::new(TEMP_PAGE, &mut memory_controller.frame_allocator);
            temporary_page.map(Frame::get_containing_frame(physical_address), &mut memory_controller.active_table);

            let sdt_pointer = TEMP_PAGE.start_address().offset(physical_address.offset_into_frame() as isize).ptr() as *const SdtHeader;
            let signature = unsafe { str::from_utf8_unchecked(&(*sdt_pointer).signature) };
            serial_println!("Found SDT: {} at {:#x}", signature, physical_address);

            match signature.as_ref()
            {
                "FACP" => self::fadt::parse_fadt(sdt_pointer, acpi_info),
                "APIC" => self::madt::parse_madt(sdt_pointer, acpi_info),
                _      => println!("Unknown table: {}", signature),
            }

            temporary_page.unmap(&mut memory_controller.active_table);
        }
    }
}

#[derive(Clone,Debug)]
pub struct AcpiInfo
{
    pub rsdp                            : &'static Rsdp,
    pub rsdt                            : Option<Box<Rsdt>>,

    // FADT
    pub fadt                            : Option<Box<Fadt>>,

    // MADT
    pub local_apic_address              : PhysicalAddress,
    pub legacy_pics_active              : bool,
    pub ioapic_address                  : PhysicalAddress,
    pub ioapic_global_interrupt_base    : u32,
}

impl AcpiInfo
{
    pub fn new<A>(boot_info            : &BootInformation,
                  memory_controller    : &mut MemoryController<A>) -> AcpiInfo
        where A : FrameAllocator
    {
        use ::memory::map::TEMP_PAGE;

        let rsdp : &'static Rsdp = boot_info.rsdp().expect("Couldn't find RSDP tag").rsdp();
        rsdp.validate().unwrap();

        serial_println!("Loading ACPI tables with OEM ID: {}", rsdp.oem_str());
        let physical_address = PhysicalAddress::new(rsdp.rsdt_address as usize);
        let mut temporary_page = TemporaryPage::new(TEMP_PAGE, &mut memory_controller.frame_allocator);
        temporary_page.map(Frame::get_containing_frame(physical_address), &mut memory_controller.active_table);
        let rsdt_ptr = (TEMP_PAGE.start_address().offset(physical_address.offset_into_frame() as isize)).ptr() as *const SdtHeader;

        let rsdt : Box<Rsdt> = unsafe { Box::new(ptr::read_unaligned(rsdt_ptr as *const Rsdt)) };
        rsdt.header.validate("RSDT").unwrap();
        temporary_page.unmap(&mut memory_controller.active_table);

        let mut acpi_info = AcpiInfo
                            {
                                rsdp,
                                rsdt                            : None,
                                fadt                            : None,
                                local_apic_address              : 0.into(),
                                legacy_pics_active              : true,
                                ioapic_address                  : 0.into(),
                                ioapic_global_interrupt_base    : 0,
                            };

        rsdt.parse(&mut acpi_info, memory_controller);
        acpi_info.rsdt = Some(rsdt);

        acpi_info
    }
}
