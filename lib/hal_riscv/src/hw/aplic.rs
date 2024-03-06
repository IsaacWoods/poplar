use bit_field::BitField;
use volatile::Volatile;

/// The Advanced Platform-Level Interrupt Controller (APLIC) is a component of the Advanced
/// Interrupt Architecture that translates traditionally wired interrupts into MSIs that can be
/// distributed to harts' IMSICs.
#[repr(C)]
pub struct AplicDomain {
    pub domaincfg: Volatile<u32>,
    pub sourcecfg: Volatile<[u32; 1023]>,
    _reserved0: [u8; 0xbc0],
    pub m_msi_addr_cfg: Volatile<u32>,
    pub m_msi_addr_cfg_hi: Volatile<u32>,
    pub s_msi_addr_cfg: Volatile<u32>,
    pub s_msi_addr_cfg_hi: Volatile<u32>,
    _reserved1: [u8; 0x30],
    pub setip: Volatile<[u32; 32]>,
    _reserved2: [u8; 0x5c],
    pub setipnum: Volatile<u32>,
    _reserved3: [u8; 0x20],
    pub clear_ip: Volatile<[u32; 32]>,
    _reserved4: [u8; 0x5c],
    pub clear_ip_num: Volatile<u32>,
    _reserved5: [u8; 0x20],
    pub set_ie: Volatile<[u32; 32]>,
    _reserved6: [u8; 0x5c],
    pub set_ie_num: Volatile<u32>,
    _reserved7: [u8; 0x20],
    pub clear_ie: Volatile<[u32; 32]>,
    _reserved8: [u8; 0x5c],
    pub clear_ie_num: Volatile<u32>,
    _reserved9: [u8; 0x20],
    pub set_ip_num_le: Volatile<u32>,
    pub set_ip_num_be: Volatile<u32>,
    _reserved10: [u8; 0xff8],
    pub gen_msi: Volatile<u32>,
    pub target: Volatile<[u32; 1023]>,
}

impl AplicDomain {
    pub fn init(&self) {
        /*
         * Enable the APLIC and set it to send MSIs. We just assume this is a little-endian
         * machine.
         */
        let mut domaincfg = 0;
        domaincfg.set_bit(8, true); // Enable interrupts
        domaincfg.set_bit(2, true); // Send MSIs instead of direct interrupts
        self.domaincfg.write(domaincfg);
    }

    pub fn set_msi_address(&self, address: usize) {
        let lo = address.get_bits(12..44) as u32;
        let hi = {
            let mut value = 0u32;
            value.set_bits(0..12, address.get_bits(44..64) as u32);
            value.set_bits(20..23, 0); // Low hart index shift(??)
            value
        };
        self.s_msi_addr_cfg.write(lo);
        self.s_msi_addr_cfg_hi.write(hi);
    }

    // TODO: maybe take the hart to send it to as a param?
    pub fn set_target_msi(&self, irq: u32, message: u32) {
        let mut value = 0u32;
        value.set_bits(0..11, message);
        value.set_bits(12..18, 0); // Guest index
        value.set_bits(18..32, 0); // Hart index
        self.target[irq as usize - 1].write(value);
    }

    pub fn set_source_cfg(&self, irq: u32, mode: SourceMode) {
        let mut source_cfg = 0;
        source_cfg.set_bits(0..3, mode as u32);
        self.sourcecfg[irq as usize - 1].write(source_cfg);
    }

    pub fn enable_interrupt(&self, irq: u32) {
        let index = irq / 32;
        self.set_ie[index as usize].write(1 << ((irq as usize) % 32));
    }
}

#[repr(u32)]
pub enum SourceMode {
    Inactive = 0,
    Detached = 1,
    RisingEdge = 4,
    FallingEdge = 5,
    LevelHigh = 6,
    LevelLow = 7,
}
