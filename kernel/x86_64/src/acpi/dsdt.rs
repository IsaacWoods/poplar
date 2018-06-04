/*
 * Copyright (C) 2018, Pebble Developers.
 * See LICENCE.md
 */

use super::aml::AmlParser;
use super::{AcpiInfo, SdtHeader};
use core::mem;
use memory::paging::{PhysicalMapping, VirtualAddress};

#[derive(Clone, Debug)]
#[repr(packed)]
pub struct Dsdt {
    pub(super) header: SdtHeader,
}

pub fn parse_dsdt(mapping: &PhysicalMapping<Dsdt>, acpi_info: &mut AcpiInfo) {
    let mut parser = unsafe {
        AmlParser::new(
            VirtualAddress::from(mapping.ptr as usize).offset(mem::size_of::<SdtHeader>() as isize),
            (*mapping).header.length as usize - mem::size_of::<SdtHeader>(),
        )
    };

    match parser.parse(acpi_info) {
        Ok(_) => {}

        Err(error) => {
            error!("Failed to parse DSDT (error: {:?})", error);
            warn!("The kernel will carry on, but functionality may be reduced / we might crash");
        }
    }
}
