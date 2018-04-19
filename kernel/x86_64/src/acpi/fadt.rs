/*
 * Copyright (C) 2017, Pebble Developers.
 * See LICENCE.md
 */

use super::SdtHeader;

#[derive(Clone,Copy,Debug)]
#[repr(packed)]
pub(super) struct AddressStructure
{
    pub(super) address_space   : u8,
    pub(super) bit_width       : u8,
    pub(super) bit_offset      : u8,
    pub(super) access_size     : u8,
    pub(super) address         : u64,
}

#[derive(Clone,Copy,Debug)]
#[repr(packed)]
pub struct Fadt
{
    pub(super) header              : SdtHeader,
    pub(super) firmware_ctrl       : u32,
    pub(super) dsdt_address        : u32,
               reserved_1          : u8,
    pub(super) power_profile       : u8,
    pub(super) sci_interrupt       : u16,
    pub(super) smi_command_port    : u32,
    pub(super) acpi_enable         : u8,
    pub(super) acpi_disable        : u8,
    pub(super) s4bios_req          : u8,
    pub(super) pstate_control      : u8,
    pub(super) pm1a_event_block    : u32,
    pub(super) pm1b_event_block    : u32,
    pub(super) pm1a_control_block  : u32,
    pub(super) pm1b_control_block  : u32,
    pub(super) pm2_control_block   : u32,
    pub(super) pm_timer_block      : u32,
    pub(super) gpe0_block          : u32,
    pub(super) gpe1_block          : u32,
    pub(super) pm1_event_length    : u8,
    pub(super) pm1_control_length  : u8,
    pub(super) pm2_control_length  : u8,
    pub(super) pm_timer_length     : u8,
    pub(super) gpe0_length         : u8,
    pub(super) gpe1_length         : u8,
    pub(super) gpe1_base           : u8,
    pub(super) cstate_control      : u8,
    pub(super) worst_c2_latency    : u16,
    pub(super) worst_c3_latency    : u16,
    pub(super) flush_size          : u16,
    pub(super) flush_stride        : u16,
    pub(super) duty_offset         : u8,
    pub(super) duty_width          : u8,
    pub(super) day_alarm           : u8,
    pub(super) month_alarm         : u8,
    pub(super) century             : u8,

               reserved_2          : [u8; 3],
    pub(super) flags               : u32,
    pub(super) reset_reg           : AddressStructure,
    pub(super) reset_value         : u8,
               reserved_3          : [u8; 3],
}
