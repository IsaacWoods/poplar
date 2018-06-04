/*
 * Copyright (C) 2017, Pebble Developers.
 * See LICENCE.md
 */

mod aml;
mod dsdt;
mod fadt;
mod madt;

use self::dsdt::Dsdt;
use self::fadt::Fadt;
use self::madt::MadtHeader;
use alloc::Vec;
use core::{mem, str};
use cpu::Cpu;
use memory::paging::{EntryFlags, PhysicalAddress, PhysicalMapping, TemporaryPage, VirtualAddress};
use memory::{Frame, MemoryController};
use multiboot2::BootInformation;

/*
 * The RSDP (Root System Descriptor Pointer) is the first ACPI structure located.
 */
#[derive(Clone, Copy, Debug)]
#[repr(packed)]
pub struct Rsdp {
    pub(self) signature: [u8; 8],
    pub(self) checksum: u8,
    pub(self) oem_id: [u8; 6],
    pub(self) revision: u8,
    pub(self) rsdt_address: u32,
}

impl Rsdp {
    fn validate(&self) -> Result<(), &str> {
        // Check the RSDP's signature (should be "RSD PTR ")
        if &self.signature != b"RSD PTR " {
            return Err("RSDP has incorrect signature");
        }

        let mut sum: usize = 0;

        for i in 0..mem::size_of::<Rsdp>() {
            sum += unsafe { *(self as *const Rsdp as *const u8).offset(i as isize) } as usize;
        }

        // Check that the lowest byte is 0
        if sum & 0b1111_1111 != 0 {
            return Err("RSDP has incorrect checksum");
        }

        Ok(())
    }

    fn oem_str(&self) -> &str {
        str::from_utf8(&self.oem_id).expect("Could not extract OEM ID from RSDP")
    }
}

/*
 * This is the layout of the header for all of the System Descriptor Tables.
 * XXX: This will be followed by variable amounts of data, depending on which SDT this is.
 */
#[derive(Clone, Copy, Debug)]
#[repr(packed)]
pub struct SdtHeader {
    signature: [u8; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32,
    // ...
}

impl SdtHeader {
    fn validate(&self, signature: &str) -> Result<(), &str> {
        // Check the signature
        let table_signature = match str::from_utf8(&self.signature) {
            Ok(signature) => signature,
            _ => return Err("SDT has incorrect signature"),
        };
        if table_signature != signature {
            return Err("SDT has incorrect signature");
        }

        let mut sum: usize = 0;

        for i in 0..self.length {
            sum += unsafe { *(self as *const SdtHeader as *const u8).offset(i as isize) } as usize;
        }

        // Check that the lowest byte is 0
        if sum & 0b1111_1111 != 0 {
            return Err("SDT has incorrect checksum");
        }

        Ok(())
    }
}

/// This temporarily maps a SDT to get its signature and length, then unmaps it
/// It's used to calculate the size we need to actually map
unsafe fn peek_at_table(
    table_address: PhysicalAddress,
    memory_controller: &mut MemoryController,
) -> ([u8; 4], u32) {
    use memory::map::TEMP_PAGE;

    let signature: [u8; 4];
    let length: u32;

    {
        let mut temporary_page = TemporaryPage::new(TEMP_PAGE);
        temporary_page.map(
            Frame::containing_frame(table_address),
            &mut memory_controller.kernel_page_table,
            &mut memory_controller.frame_allocator,
        );
        let sdt_pointer = TEMP_PAGE
            .start_address()
            .offset(table_address.offset_into_frame() as isize)
            .ptr() as *const SdtHeader;

        signature = (*sdt_pointer).signature;
        length = (*sdt_pointer).length;

        temporary_page.unmap(
            &mut memory_controller.kernel_page_table,
            &mut memory_controller.frame_allocator,
        );
    }

    (signature, length)
}

fn parse_rsdt(acpi_info: &mut AcpiInfo, memory_controller: &mut MemoryController) {
    let num_tables =
        (acpi_info.rsdt.length as usize - mem::size_of::<SdtHeader>()) / mem::size_of::<u32>();
    let table_base_ptr = VirtualAddress::from(acpi_info.rsdt.ptr)
        .offset(mem::size_of::<SdtHeader>() as isize)
        .ptr() as *const u32;

    for i in 0..num_tables {
        let pointer_address = unsafe { table_base_ptr.offset(i as isize) };
        let table_address = PhysicalAddress::new(unsafe { *pointer_address } as usize);
        let (signature, length) = unsafe { peek_at_table(table_address, memory_controller) };

        match &signature {
            b"FACP" => {
                let fadt_mapping = memory_controller
                    .kernel_page_table
                    .map_physical_region::<Fadt>(
                        table_address,
                        table_address.offset(length as isize),
                        EntryFlags::PRESENT,
                        &mut memory_controller.frame_allocator,
                    );
                (*fadt_mapping).header.validate("FACP").unwrap();
                let dsdt_address = PhysicalAddress::from((*fadt_mapping).dsdt_address as usize);
                acpi_info.fadt = Some(fadt_mapping);

                // Now we have the FADT, we can map and parse the DSDT
                let (_, dsdt_length) = unsafe { peek_at_table(dsdt_address, memory_controller) };
                let dsdt_mapping = memory_controller
                    .kernel_page_table
                    .map_physical_region::<Dsdt>(
                        dsdt_address,
                        dsdt_address.offset(dsdt_length as isize),
                        EntryFlags::PRESENT,
                        &mut memory_controller.frame_allocator,
                    );
                (*dsdt_mapping).header.validate("DSDT").unwrap();
                dsdt::parse_dsdt(&dsdt_mapping, acpi_info);
            }

            b"APIC" => {
                let madt_mapping = memory_controller
                    .kernel_page_table
                    .map_physical_region::<MadtHeader>(
                        table_address,
                        table_address.offset(length as isize),
                        EntryFlags::PRESENT,
                        &mut memory_controller.frame_allocator,
                    );

                (*madt_mapping).header.validate("APIC").unwrap();
                madt::parse_madt(&madt_mapping, acpi_info, memory_controller);
            }

            _ => match str::from_utf8(&signature) {
                Ok(signature_str) => warn!("Unhandled SDT type: {}", signature_str),
                Err(_) => error!("Unhandled SDT type; signature is not valid!"),
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct AcpiInfo {
    pub rsdp: &'static Rsdp,
    pub rsdt: PhysicalMapping<SdtHeader>,
    pub fadt: Option<PhysicalMapping<Fadt>>,
    pub dsdt: Option<PhysicalMapping<Dsdt>>,

    pub bootstrap_cpu: Option<Cpu>,
    pub application_cpus: Vec<Cpu>,
}

impl AcpiInfo {
    pub fn new(
        boot_info: &BootInformation,
        memory_controller: &mut MemoryController,
    ) -> Option<AcpiInfo> {
        let rsdp: &'static Rsdp = boot_info.rsdp().expect("Couldn't find RSDP tag").rsdp();
        let rsdt_address = PhysicalAddress::from(rsdp.rsdt_address as usize);
        rsdp.validate().unwrap();

        trace!("Loading ACPI tables with OEM ID: {}", rsdp.oem_str());
        let (rsdt_signature, rsdt_length) =
            unsafe { peek_at_table(rsdt_address, memory_controller) };

        if &rsdt_signature != b"RSDT" {
            return None;
        }

        let rsdt_mapping = memory_controller
            .kernel_page_table
            .map_physical_region::<SdtHeader>(
                rsdt_address,
                rsdt_address.offset(rsdt_length as isize),
                EntryFlags::PRESENT,
                &mut memory_controller.frame_allocator,
            );

        let mut acpi_info = AcpiInfo {
            rsdp,
            rsdt: rsdt_mapping,
            fadt: None,
            dsdt: None,

            bootstrap_cpu: None,
            application_cpus: Vec::new(),
        };

        (*acpi_info.rsdt).validate("RSDT").unwrap();
        parse_rsdt(&mut acpi_info, memory_controller);

        Some(acpi_info)
    }
}
