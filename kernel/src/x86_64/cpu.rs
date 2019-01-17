use alloc::boxed::Box;
use core::pin::Pin;
use x86_64::hw::gdt::SegmentSelector;
use x86_64::hw::tss::Tss;

pub struct Cpu {
    pub processor_uid: u8,
    pub local_apic_id: u8,
    pub is_ap: bool,
    pub tss: Pin<Box<Tss>>,
    pub tss_selector: SegmentSelector,
}

impl Cpu {
    pub fn from_acpi(
        acpi_info: &acpi::Processor,
        tss: Pin<Box<Tss>>,
        tss_selector: SegmentSelector,
    ) -> Cpu {
        Cpu {
            processor_uid: acpi_info.processor_uid,
            local_apic_id: acpi_info.local_apic_id,
            is_ap: acpi_info.is_ap,
            tss,
            tss_selector,
        }
    }
}
