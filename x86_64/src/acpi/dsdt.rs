/*
 * Copyright (C) 2018, Pebble Developers.
 * See LICENCE.md
 */

use memory::paging::PhysicalMapping;
use super::{SdtHeader,AcpiInfo};

#[derive(Clone,Debug)]
#[repr(packed)]
pub struct Dsdt
{
    pub(super) header   : SdtHeader,
}

pub fn parse_dsdt(mapping : &PhysicalMapping<Dsdt>, acpi_info : &mut AcpiInfo)
{
    info!("Parsing DSDT: {:#x},{}", ::memory::paging::VirtualAddress::from(mapping.ptr).offset(::core::mem::size_of::<SdtHeader>() as isize), mapping.size);
}
