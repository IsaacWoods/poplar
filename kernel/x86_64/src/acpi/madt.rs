/*
 * Copyright (C) 2017, Pebble Developers.
 * See LICENCE.md
 */

use super::{AcpiInfo, SdtHeader};
use apic::{DeliveryMode, PinPolarity, TriggerMode, IO_APIC, LOCAL_APIC};
use bit_field::BitField;
use core::{mem, ptr};
use cpu::{Cpu, CpuState};
use memory::paging::{PhysicalAddress, PhysicalMapping, VirtualAddress};
use memory::MemoryController;

#[derive(Clone, Copy, Debug)]
#[repr(packed)]
pub struct MadtHeader {
    pub(super) header: SdtHeader,
    local_apic_address: u32,
    flags: u32,
}

#[derive(Clone, Copy, Debug)]
#[repr(packed)]
struct MadtEntryHeader {
    entry_type: u8,
    length: u8,
}

#[derive(Clone, Copy, Debug)]
#[repr(packed)]
struct LocalApicEntry {
    header: MadtEntryHeader,
    processor_id: u8,
    apic_id: u8,
    flags: u32,
}

#[derive(Clone, Copy, Debug)]
#[repr(packed)]
struct IoApicEntry {
    header: MadtEntryHeader,
    id: u8,
    reserved_1: u8,
    address: u32,
    global_system_interrupt_base: u32,
}

#[derive(Clone, Copy, Debug)]
#[repr(packed)]
struct InterruptSourceOverrideEntry {
    header: MadtEntryHeader,
    bus_source: u8,
    irq_source: u8,
    global_system_interrupt: u32,
    flags: u16,
}

#[derive(Clone, Copy, Debug)]
#[repr(packed)]
struct NonMaskableInterruptEntry {
    header: MadtEntryHeader,
    processor_id: u8,
    flags: u16,
    lint: u8,
}

#[derive(Clone, Copy, Debug)]
#[repr(packed)]
struct LocalApicAddressOverrideEntry {
    header: MadtEntryHeader,
    address: u64,
}

/*
 * It seems way too coupled to initialise the local APIC and IOAPIC here, but it's very convienient
 * while we have all the data from the MADT already mapped. Takes a reference to the platform
 * instead of using the global to enfore as much borrow-checking as we can.
 */
pub(super) fn parse_madt(
    mapping: &PhysicalMapping<MadtHeader>,
    acpi_info: &mut AcpiInfo,
    memory_controller: &mut MemoryController,
) {
    // Initialise the local APIC
    let local_apic_address = PhysicalAddress::new((*mapping).local_apic_address as usize);
    unsafe {
        LOCAL_APIC.enable(local_apic_address, memory_controller);
    }

    let mut entry_address =
        VirtualAddress::from(mapping.ptr).offset(mem::size_of::<MadtHeader>() as isize);
    let end_address =
        VirtualAddress::from(mapping.ptr).offset(((*mapping).header.length - 1) as isize);

    while entry_address < end_address {
        let header = unsafe { ptr::read_unaligned(entry_address.ptr() as *const MadtEntryHeader) };

        match header.entry_type
        {
            0 =>    // Processor local APIC
            {
                trace!("Found MADT entry: processor local APIC (type=0)");
                let entry = unsafe { ptr::read_unaligned(entry_address.ptr() as *const LocalApicEntry) };

                /*
                 * The first CPU described in the MADT is the BSP. Subsequent ones are APs, and
                 * should be brought up in the order they are defined in.
                 */
                let is_ap       = acpi_info.bootstrap_cpu.is_some();
                let is_disabled = unsafe { !entry.flags.get_bit(0) };

                let state = match (is_ap, is_disabled)
                            {
                                (_    , true ) => CpuState::Disabled,
                                (true , false) => CpuState::WaitingForSipi,
                                (false, false) => CpuState::Running,
                            };

                if is_ap
                {
                    acpi_info.application_cpus.push(Cpu::new(entry.processor_id,
                                                             entry.apic_id,
                                                             true,
                                                             state));
                }
                else
                {
                    acpi_info.bootstrap_cpu = Some(Cpu::new(entry.processor_id,
                                                            entry.apic_id,
                                                            false,
                                                            state));
                }

                entry_address = entry_address.offset(mem::size_of::<LocalApicEntry>() as isize);
            },

            1 =>    // I/O APIC
            {
                trace!("Found MADT entry: I/O APIC (type=1)");
                let entry = unsafe { ptr::read_unaligned(entry_address.ptr() as *const IoApicEntry) };
                let io_apic_address = PhysicalAddress::new(entry.address as usize);

                unsafe
                {
                    IO_APIC.enable(io_apic_address,
                                   entry.global_system_interrupt_base as u8,
                                   memory_controller);
                }

                entry_address = entry_address.offset(12);
            },

            2 =>    // Interrupt source override
            {
                trace!("Found MADT entry: interrupt source override (type=2)");
                let entry = unsafe { ptr::read_unaligned(entry_address.ptr() as *const InterruptSourceOverrideEntry) };

                let pin_polarity = if (entry.flags & 2) > 0 { PinPolarity::Low  }
                                                       else { PinPolarity::High };

                let trigger_mode = if (entry.flags & 8) > 0 { TriggerMode::Level }
                                                       else { TriggerMode::Edge  };

                unsafe
                {
                    IO_APIC.write_entry((entry.global_system_interrupt as u8) - IO_APIC.global_interrupt_base(),
                                        ::interrupts::IOAPIC_BASE + entry.irq_source,
                                        DeliveryMode::Fixed,
                                        pin_polarity,
                                        trigger_mode,
                                        true,    // Masked by default
                                        0xFF);
                }

                entry_address = entry_address.offset(10);
            },

            4 =>    // Non-maskable interrupt
            {
                trace!("Found MADT entry: non-maskable interrupt(type=4)");
                let entry = unsafe { ptr::read_unaligned(entry_address.ptr() as *const NonMaskableInterruptEntry) };
                assert_eq!(entry.processor_id, 0xFF, "Unhandled case - NMI for subset of processors!");

                // TODO: handle flags on the MADT entry - edge/level triggered? high or low?
                let nmi_entry = (0b100<<8) | 2; // Non-maskable interrupt on vector 2
                match entry.lint
                {
                    0 => unsafe { ptr::write(LOCAL_APIC.register_ptr(0x350), nmi_entry) },
                    1 => unsafe { ptr::write(LOCAL_APIC.register_ptr(0x360), nmi_entry) },
                    _ => panic!("LINT for MADT entry-type=4 should either be 0 or 1!"),
                }

                entry_address = entry_address.offset(6);
            },

            5 =>    // Local APIC address override
            {
                /*
                 * XXX: Yeah, this won't actually work for various reasons (pages already mapped
                 * and won't redo config by previous entries) but QEMU doesn't use it so idc for
                 * now.
                 */
                trace!("Found MADT entry: local APIC address override (type=5)");
                panic!("We don't support systems where the local APIC has been overridden!");

                // let entry = unsafe { ptr::read_unaligned(entry_address.ptr() as *const LocalApicAddressOverrideEntry) };
                // let local_apic_address_override = PhysicalAddress::new(entry.address as usize);
                // entry_address = entry_address.offset(12);
            },

            _ => warn!("Unknown MADT entry type: '{}'. Ignoring.", header.entry_type),
        }
    }
}
