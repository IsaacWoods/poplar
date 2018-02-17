/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use core::{mem,ptr};
use alloc::boxed::Box;
use super::{AcpiInfo,SdtHeader};
use ::memory::{MemoryController,FrameAllocator};
use ::memory::paging::{PhysicalAddress,VirtualAddress};
use ::apic::{LocalApic,IoApic};

#[derive(Clone,Copy,Debug)]
#[repr(packed)]
pub struct MadtHeader
{
    header              : SdtHeader,
    local_apic_address  : u32,
    flags               : u32,
    /*
     * After this, there are a number of entries (also variable length). It's not really practical to
     * represent this whole structure in Rust nicely, so we don't.
     */
}

#[derive(Clone,Copy,Debug)]
#[repr(packed)]
struct MadtEntryHeader
{
    entry_type  : u8,
    length      : u8,
}

#[derive(Clone,Copy,Debug)]
#[repr(packed)]
struct LocalApicEntry
{
    header          : MadtEntryHeader,
    processor_id    : u8,
    apic_id         : u8,
    flags           : u32,
}

#[derive(Clone,Copy,Debug)]
#[repr(packed)]
struct IoApicEntry
{
    header                          : MadtEntryHeader,
    id                              : u8,
    reserved_1                      : u8,
    address                         : u32,
    global_system_interrupt_base    : u32,
}

#[derive(Clone,Copy,Debug)]
#[repr(packed)]
struct InterruptSourceOverrideEntry
{
    header                  : MadtEntryHeader,
    bus_source              : u8,
    irq_source              : u8,
    global_system_interrupt : u32,
    flags                   : u16,
}

#[derive(Clone,Copy,Debug)]
#[repr(packed)]
struct NonMaskableInterruptEntry
{
    header          : MadtEntryHeader,
    processor_id    : u8,
    flags           : u16,
    lint            : u8,
}

#[derive(Clone,Copy,Debug)]
#[repr(packed)]
struct LocalApicAddressOverrideEntry
{
    header  : MadtEntryHeader,
    address : u64,
}

/*
 * It seems way too coupled to initialise the local APIC and IOAPIC here, but it's very convienient
 * while we have all the data from the MADT already mapped.
 */
pub(super) fn parse_madt<A>(ptr                : *const SdtHeader,
                            acpi_info          : &mut AcpiInfo,
                            memory_controller  : &mut MemoryController<A>)
    where A : FrameAllocator
{
    let madt : Box<MadtHeader> = Box::new(unsafe { ptr::read_unaligned(ptr as *const MadtHeader) });
    //madt.header.validate("APIC").unwrap(); //TODO: why isn't checksum correct?

    let local_apic_address = PhysicalAddress::new(madt.local_apic_address as usize);
    acpi_info.local_apic = Some(unsafe { LocalApic::new(local_apic_address, memory_controller) });

    let mut entry_address = VirtualAddress::new(ptr as usize).offset(mem::size_of::<MadtHeader>() as isize);
    let end_address = VirtualAddress::new(ptr as usize).offset((madt.header.length - 1) as isize);

    while entry_address < end_address
    {
        let header = unsafe { ptr::read_unaligned(entry_address.ptr() as *const MadtEntryHeader) };

        match header.entry_type
        {
            0 =>    // Processor local APIC
            {
                serial_println!("Found MADT entry: processor local APIC (type=0)");
                let entry = unsafe { ptr::read_unaligned(entry_address.ptr() as *const LocalApicEntry) };
                serial_println!("{:#?}", entry);
                // TODO: keep track of each core and its local APIC
                entry_address = entry_address.offset(mem::size_of::<LocalApicEntry>() as isize);
            },

            1 =>    // I/O APIC
            {
                serial_println!("Found MADT entry: I/O APIC (type=1)");
                let entry = unsafe { ptr::read_unaligned(entry_address.ptr() as *const IoApicEntry) };

                let io_apic_address = PhysicalAddress::new(entry.address as usize);
                acpi_info.io_apic = Some(unsafe { IoApic::new(io_apic_address, memory_controller) });
                // TODO: do something with the global system interrupt base?
                entry_address = entry_address.offset(12);
            },

            2 =>    // Interrupt source override
            {
                serial_println!("Found MADT entry: interrupt source override (type=2)");
                let entry = unsafe { ptr::read_unaligned(entry_address.ptr() as *const InterruptSourceOverrideEntry) };
                serial_println!("{:#?}", entry);
                // TODO: Idk do stuff with this?
                entry_address = entry_address.offset(10);
            },

            4 =>    // Non-maskable interrupt
            {
                serial_println!("Found MADT entry: non-maskable interrupt(type=4)");
                let entry = unsafe { ptr::read_unaligned(entry_address.ptr() as *const NonMaskableInterruptEntry) };
                serial_println!("{:#?}", entry);
                // TODO: Configure LINT0 and LINT1 in the local vector table of the local APIC
                entry_address = entry_address.offset(6);
            },

            5 =>    // Local APIC address override
            {
                /*
                 * XXX: Yeah, this won't actually work for various reasons (pages already mapped
                 * and won't redo config by previous entries) but QEMU doesn't use it so idc for
                 * now.
                 */
                serial_println!("Found MADT entry: local APIC address override (type=5)");
                let entry = unsafe { ptr::read_unaligned(entry_address.ptr() as *const LocalApicAddressOverrideEntry) };
                let local_apic_address_override = PhysicalAddress::new(entry.address as usize);
                acpi_info.local_apic = Some(unsafe { LocalApic::new(local_apic_address_override, memory_controller) });
                entry_address = entry_address.offset(12);
            },

            _ => panic!("Unknown MADT entry type: {}", header.entry_type),
        }
    }
}
