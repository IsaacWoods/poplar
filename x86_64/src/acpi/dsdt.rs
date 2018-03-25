/*
 * Copyright (C) 2018, Pebble Developers.
 * See LICENCE.md
 */

use core::mem;
use memory::paging::{VirtualAddress,PhysicalMapping};
use super::{SdtHeader,AcpiInfo};
use super::aml::AmlParser;

#[derive(Clone,Debug)]
#[repr(packed)]
pub struct Dsdt
{
    pub(super) header   : SdtHeader,
}

pub fn parse_dsdt(mapping : &PhysicalMapping<Dsdt>, acpi_info : &mut AcpiInfo)
{
    let mut parser = unsafe
                     {
                         AmlParser::new(VirtualAddress::from(mapping.ptr as usize).offset(mem::size_of::<SdtHeader>() as isize),
                                        (*mapping).header.length as usize - mem::size_of::<SdtHeader>())
                     };

    parser.parse(acpi_info);
}
