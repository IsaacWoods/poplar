use bit_field::BitField;
use core::ptr;

#[derive(Debug)]
pub struct Capabilities {
    /// The offset from the base of the register space that the operational registers begin at.
    pub operation_registers_offset: u8,
    /// A BCD encoding of the xHCI version supported by this controller. The most significant byte is the major
    /// version, and the least significant byte is the minor version (e.g. 0x0110 is version 1.1.0).
    pub hci_version: u16,
    /// The maximum number of Device Context Structures and Doorbell Array entries this controller supports.
    pub max_device_slots: u8,
    /// The number of Interruptors implemented by this device. Each Interruptor may be allocated to an MSI or MSI-X
    /// vector. This value determines how many Interruptor Register Sets are addressable in the Runtime Register
    /// Space.
    pub max_interruptors: u16,
    pub max_ports: u8,
    pub isoch_scheduling_threshold: IsochSchedulingThreshold,
    pub num_event_ring_segment_table_entries: u16,
    pub max_scratchpad_buffers: u16,
    /// `true` if the controller uses the Scratchpad Buffers to save state when executing Save State and Restore State
    /// operations.
    pub scratchpad_restore: bool,
    /// The worst case latency to transition a root hub Port Link State from U1 to U0. Should be interpreted as
    /// "less than `x` microseconds".
    pub u1_device_exit_latency: u8,
    /// The worst case latency to transition a root hub Port Link State from U2 to U0. Should be interpreted as
    /// "less than `x` microseconds".
    pub u2_device_exit_latency: u8,
    pub can_address_64bit: bool,
    pub can_negotiate_bandwidth: bool,
    /// Size of Context data structures, in bytes. Does not apply to Stream Contexts.
    pub context_size: u8,
    pub has_port_power_control: bool,
    pub has_port_indicator_control: bool,
    pub supports_light_reset: bool,
    pub supports_latency_tolerance_messaging: bool,
    pub has_secondary_stream_ids: bool,
    pub parses_all_event_data_trbs: bool,
    pub can_generate_short_packet_codes: bool,
    pub supports_stopped_edtla: bool,
    pub supports_isoch_frame_id_matching: bool,
    /// `None` if Streams are not supported. Otherwise, the size of a Primary Stream Array.
    pub primary_stream_array_size: Option<u16>,
    /// Offset from the Register Space Base, in `dwords`, to the first Extended Capability.
    pub ext_capabilities_offset: u16,
    /// Offset from the Register Space Base, in `dwords`, to the Doorbell Array.
    pub doorbell_offset: u32,
    pub runtime_registers_offset: u32,
    pub supports_suspend_complete: bool,
    pub can_generate_max_exit_latency_too_large: bool,
    pub supports_force_save_context: bool,
    pub supports_compliance_transistion_enabled_flag: bool,
    pub supports_large_esit_payloads: bool,
    pub supports_config_info: bool,
    pub supports_large_burst_counts: bool,
    pub trbs_indicates_additional_info: bool,
    pub supports_extended_property_commands: bool,
    pub supports_virtualization_trusted_io: bool,
}

impl Capabilities {
    /// This gathers information out of the controller's Capability Register Space. It's kind of a big mess, so
    /// refer to §5.3 of the xHCI spec for descriptions of where everything is.
    pub unsafe fn read_from_registers(base: usize) -> Capabilities {
        let cap_length: u8 = unsafe { Self::read_register(base, 0x00) };
        let hci_version: u16 = unsafe { Self::read_register(base, 0x02) };
        let (max_device_slots, max_interruptors, max_ports) = {
            let hcs_params_1: u32 = unsafe { Self::read_register(base, 0x04) };
            let max_device_slots = hcs_params_1.get_bits(0..8) as u8;
            let max_interruptors = hcs_params_1.get_bits(8..19) as u16;
            let max_ports = hcs_params_1.get_bits(24..32) as u8;
            (max_device_slots, max_interruptors, max_ports)
        };
        let (
            isoch_scheduling_threshold,
            num_event_ring_segment_table_entries,
            max_scratchpad_buffers,
            scratchpad_restore,
        ) = {
            let hcs_params_2: u32 = unsafe { Self::read_register(base, 0x08) };
            let isoch_scheduling_threshold = match hcs_params_2.get_bit(3) {
                true => IsochSchedulingThreshold::Frames(hcs_params_2.get_bits(0..2) as u8),
                false => IsochSchedulingThreshold::Microframes(hcs_params_2.get_bits(0..2) as u8),
            };
            let num_event_ring_segment_table_entries = 1 << (hcs_params_2.get_bits(4..8) as u16);
            let max_scratchpad_buffers = {
                let mut result = hcs_params_2.get_bits(27..32) as u16;
                result.set_bits(5..10, hcs_params_2.get_bits(21..26) as u16);
                result
            };
            let scratchpad_restore = hcs_params_2.get_bit(26);

            (
                isoch_scheduling_threshold,
                num_event_ring_segment_table_entries,
                max_scratchpad_buffers,
                scratchpad_restore,
            )
        };
        let (u1_device_exit_latency, u2_device_exit_latency) = {
            let hcs_params_3: u32 = unsafe { Self::read_register(base, 0x0c) };
            let u1_device_exit_latency = hcs_params_3.get_bits(0..8) as u8;
            let u2_device_exit_latency = hcs_params_3.get_bits(8..16) as u8;
            (u1_device_exit_latency, u2_device_exit_latency)
        };
        let (
            can_address_64bit,
            can_negotiate_bandwidth,
            context_size,
            has_port_power_control,
            has_port_indicator_control,
            supports_light_reset,
            supports_latency_tolerance_messaging,
            has_secondary_stream_ids,
            parses_all_event_data_trbs,
            can_generate_short_packet_codes,
            supports_stopped_edtla,
            supports_isoch_frame_id_matching,
            primary_stream_array_size,
            ext_capabilities_offset,
        ) = {
            let hcc_params_1: u32 = unsafe { Self::read_register(base, 0x10) };
            let context_size = if hcc_params_1.get_bit(2) { 64 } else { 32 };
            let primary_stream_size = match hcc_params_1.get_bits(12..16) {
                0 => None,
                size => Some(1 << (size + 1)),
            };

            (
                hcc_params_1.get_bit(0),
                hcc_params_1.get_bit(1),
                context_size,
                hcc_params_1.get_bit(3),
                hcc_params_1.get_bit(4),
                hcc_params_1.get_bit(5),
                hcc_params_1.get_bit(6),
                hcc_params_1.get_bit(7),
                hcc_params_1.get_bit(8),
                hcc_params_1.get_bit(9),
                hcc_params_1.get_bit(10),
                hcc_params_1.get_bit(11),
                primary_stream_size,
                hcc_params_1.get_bits(16..32) as u16,
            )
        };
        let doorbell_offset: u32 = unsafe { Self::read_register::<u32>(base, 0x14) }.get_bits(2..32);
        let runtime_registers_offset: u32 = unsafe { Self::read_register::<u32>(base, 0x18) }.get_bits(5..32);
        let (
            supports_suspend_complete,
            can_generate_max_exit_latency_too_large,
            supports_force_save_context,
            supports_compliance_transistion_enabled_flag,
            supports_large_esit_payloads,
            supports_config_info,
            supports_large_burst_counts,
            trbs_indicates_additional_info,
            supports_extended_property_commands,
            supports_virtualization_trusted_io,
        ) = {
            let hcc_params_2: u32 = unsafe { Self::read_register(base, 0x1c) };
            (
                hcc_params_2.get_bit(0),
                hcc_params_2.get_bit(1),
                hcc_params_2.get_bit(2),
                hcc_params_2.get_bit(3),
                hcc_params_2.get_bit(4),
                hcc_params_2.get_bit(5),
                hcc_params_2.get_bit(6),
                hcc_params_2.get_bit(7),
                hcc_params_2.get_bit(8),
                hcc_params_2.get_bit(9),
            )
        };

        Capabilities {
            operation_registers_offset: cap_length,
            hci_version,
            max_device_slots,
            max_interruptors,
            max_ports,
            isoch_scheduling_threshold,
            num_event_ring_segment_table_entries,
            max_scratchpad_buffers,
            scratchpad_restore,
            u1_device_exit_latency,
            u2_device_exit_latency,
            can_address_64bit,
            can_negotiate_bandwidth,
            context_size,
            has_port_power_control,
            has_port_indicator_control,
            supports_light_reset,
            supports_latency_tolerance_messaging,
            has_secondary_stream_ids,
            parses_all_event_data_trbs,
            can_generate_short_packet_codes,
            supports_stopped_edtla,
            supports_isoch_frame_id_matching,
            primary_stream_array_size,
            ext_capabilities_offset,
            doorbell_offset,
            runtime_registers_offset,
            supports_suspend_complete,
            can_generate_max_exit_latency_too_large,
            supports_force_save_context,
            supports_compliance_transistion_enabled_flag,
            supports_large_esit_payloads,
            supports_config_info,
            supports_large_burst_counts,
            trbs_indicates_additional_info,
            supports_extended_property_commands,
            supports_virtualization_trusted_io,
        }
    }

    unsafe fn read_register<T>(base: usize, offset: usize) -> T {
        unsafe { ptr::read_volatile((base + offset) as *const T) }
    }
}

#[derive(Debug)]
pub enum IsochSchedulingThreshold {
    Frames(u8),
    Microframes(u8),
}
