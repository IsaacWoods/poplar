use bit_field::BitField;

#[derive(Debug)]
pub struct Capabilities {
    cap_length: u8,
    hci_major_version: u8,
    hci_minor_version: u8,
    num_ports: u8,
    port_power_control: bool,
    port_routing_rule: bool,
    num_ports_per_companion: u8,
    num_companions: u8,
    port_indicators: bool,
    debug_port: Option<u8>,
    can_address_64bit: bool,
    programmable_frame_list: bool,
    asynchronous_schedule_park: bool,
    isochronous_schedule_threshold: u8,
    extended_caps_offset: u8,
}

impl Capabilities {
    pub fn read_from_registers(base: usize) -> Capabilities {
        let cap_length: u8 = unsafe { Self::read_register(base, 0x00) };
        let hci_version: u16 = unsafe { Self::read_register(base, 0x02) };
        let (
            num_ports,
            port_power_control,
            port_routing_rule,
            num_ports_per_companion,
            num_companions,
            port_indicators,
            debug_port,
        ) = {
            let hcs_params: u32 = unsafe { Self::read_register(base, 0x04) };
            let num_ports = hcs_params.get_bits(0..4) as u8;
            let port_power_control = hcs_params.get_bit(4);
            let port_routing_rule = hcs_params.get_bit(7);
            let num_ports_per_companion = hcs_params.get_bits(8..12) as u8;
            let num_companions = hcs_params.get_bits(12..16) as u8;
            let port_indicators = hcs_params.get_bit(16);
            let debug_port = match hcs_params.get_bits(20..24) {
                0 => None,
                port => Some(port as u8),
            };
            (
                num_ports,
                port_power_control,
                port_routing_rule,
                num_ports_per_companion,
                num_companions,
                port_indicators,
                debug_port,
            )
        };
        let (
            can_address_64bit,
            programmable_frame_list,
            asynchronous_schedule_park,
            isochronous_schedule_threshold,
            extended_caps_offset,
        ) = {
            let hcc_params: u32 = unsafe { Self::read_register(base, 0x08) };
            let can_address_64bit = hcc_params.get_bit(0);
            let programmable_frame_list = hcc_params.get_bit(1);
            let asynchronous_schedule_park = hcc_params.get_bit(2);
            let isochronous_schedule_threshold = hcc_params.get_bits(4..8) as u8;
            let extended_caps_offset = hcc_params.get_bits(8..16) as u8;
            (
                can_address_64bit,
                programmable_frame_list,
                asynchronous_schedule_park,
                isochronous_schedule_threshold,
                extended_caps_offset,
            )
        };

        Capabilities {
            cap_length,
            hci_major_version: hci_version.get_bits(8..16) as u8,
            hci_minor_version: hci_version.get_bits(0..8) as u8,
            num_ports,
            port_power_control,
            port_routing_rule,
            num_ports_per_companion,
            num_companions,
            port_indicators,
            debug_port,
            can_address_64bit,
            programmable_frame_list,
            asynchronous_schedule_park,
            isochronous_schedule_threshold,
            extended_caps_offset,
        }
    }

    unsafe fn read_register<T>(base: usize, offset: usize) -> T {
        unsafe { std::ptr::read_volatile((base as *const T).byte_add(offset)) }
    }
}
