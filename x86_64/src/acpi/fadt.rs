/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use core::ptr;
use alloc::boxed::Box;
use super::{SdtHeader,AcpiInfo};

#[derive(Clone,Copy,Debug)]
#[repr(packed)]
struct AddressStructure
{
    address_space   : u8,
    bit_width       : u8,
    bit_offset      : u8,
    access_size     : u8,
    address         : u64,
}

#[derive(Clone,Copy,Debug)]
#[repr(packed)]
pub struct Fadt
{
    header              : SdtHeader,
    firmware_ctrl       : u32,
    dsdt_address        : u32,
    reserved_1          : u8,
    power_profile       : u8,
    sci_interrupt       : u16,
    smi_command_port    : u32,
    acpi_enable         : u8,
    acpi_disable        : u8,
    s4bios_req          : u8,
    pstate_control      : u8,
    pm1a_event_block    : u32,
    pm1b_event_block    : u32,
    pm1a_control_block  : u32,
    pm1b_control_block  : u32,
    pm2_control_block   : u32,
    pm_timer_block      : u32,
    gpe0_block          : u32,
    gpe1_block          : u32,
    pm1_event_length    : u8,
    pm1_control_length  : u8,
    pm2_control_length  : u8,
    pm_timer_length     : u8,
    gpe0_length         : u8,
    gpe1_length         : u8,
    gpe1_base           : u8,
    cstate_control      : u8,
    worst_c2_latency    : u16,
    worst_c3_latency    : u16,
    flush_size          : u16,
    flush_stride        : u16,
    duty_offset         : u8,
    duty_width          : u8,
    day_alarm           : u8,
    month_alarm         : u8,
    century             : u8,

    reserved_2          : [u8; 3],
    flags               : u32,
    reset_reg           : AddressStructure,
    reset_value         : u8,
    reserved_3          : [u8; 3],
}

pub(super) fn parse_fadt(ptr : *const SdtHeader, acpi_info : &mut AcpiInfo)
{
    let fadt : Box<Fadt> = unsafe { Box::new(ptr::read_unaligned(ptr as *const Fadt)) };
    fadt.header.validate("FACP").unwrap();
    acpi_info.fadt = Some(fadt);
}
