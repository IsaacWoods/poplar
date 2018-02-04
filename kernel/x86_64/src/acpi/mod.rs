/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use core::{str,mem};
use multiboot2::BootInformation;

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

#[derive(Clone,Copy,Debug)]
pub struct AcpiInfo
{
    rsdp    : &'static RSDP
}

impl AcpiInfo
{
    pub fn new(boot_info : &BootInformation) -> AcpiInfo
    {
        let rsdp : &'static RSDP = boot_info.rsdp().expect("Couldn't find RSDP tag").rsdp();
        rsdp.validate().unwrap();

        let oem_str = unsafe { str::from_utf8_unchecked(&rsdp.oem_id) };
        serial_println!("Loading ACPI tables with OEM ID: {}", oem_str);
        println!("RSDT physical address is: {:#x}", rsdp.rsdt_address);

        AcpiInfo
        {
            rsdp : rsdp,
        }
    }
}
