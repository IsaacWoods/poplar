pub struct Cpu {
    pub processor_uid: u8,
    pub local_apic_id: u8,
    pub is_ap: bool,
}

impl Cpu {
    pub fn from_acpi(acpi_info: &acpi::Processor) -> Cpu {
        Cpu {
            processor_uid: acpi_info.processor_uid,
            local_apic_id: acpi_info.local_apic_id,
            is_ap: acpi_info.is_ap,
        }
    }
}
