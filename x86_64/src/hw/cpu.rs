use bit_field::BitField;
use core::str;

/// Describes information we know about the system we're running on.
pub struct CpuInfo {
    pub max_supported_standard_level: u32,
    pub vendor: Vendor,
    pub model_info: ModelInfo,
}

impl CpuInfo {
    pub fn new() -> CpuInfo {
        let vendor_id_cpuid = cpuid(CpuidEntry::VendorId);
        let vendor = decode_vendor(&vendor_id_cpuid);

        /*
         * Get information about the model. We also use this to work out what microarch we're
         * running on.
         */
        let model_info = decode_model_info();

        CpuInfo {
            max_supported_standard_level: vendor_id_cpuid.a,
            vendor,
            model_info,
        }
    }

    pub fn microarch(&self) -> Option<Microarch> {
        match self.vendor {
            Vendor::Intel if self.model_info.family == 0x6 => {
                match self.model_info.extended_model {
                    0x1a | 0x1e | 0x1f | 0x2e => Some(Microarch::Nehalem),
                    0x25 | 0x2c | 0x2f => Some(Microarch::Westmere),
                    0x2a | 0x2d => Some(Microarch::SandyBridge),
                    0x3a | 0x3e => Some(Microarch::IvyBridge),
                    0x3c | 0x3f | 0x45 | 0x46 => Some(Microarch::Haswell),
                    0x3d | 0x47 | 0x56 | 0x4f => Some(Microarch::Broadwell),
                    0x4e | 0x5e | 0x55 => Some(Microarch::Skylake),
                    0x8e | 0x9e => Some(Microarch::KabyLake),

                    _ => None,
                }
            }


            Vendor::Amd if self.model_info.family == 0xf => {
                match self.model_info.extended_family {
                    0x15 => Some(Microarch::Bulldozer),
                    0x16 => Some(Microarch::Jaguar),
                    0x17 => Some(Microarch::Zen),
                    _ => None,
                }
            }

            Vendor::Intel => None,
            Vendor::Amd => None,
            Vendor::Unknown => None,
        }
    }

    /// Get the frequency the APIC runs at (in Hz), if we can calculate it. If this returns `None`,
    /// we have to use another timer to work this out.
    pub fn apic_frequency(&self) -> Option<u64> {
        /*
         * If the `cpuid` info contains a non-zero core crystal clock frequency, return that.
         */
        if self.max_supported_standard_level >= 0x15 {
            let tsc_entry = cpuid(CpuidEntry::TscFrequency);

            if tsc_entry.c != 0 {
                return Some(tsc_entry.c as u64);
            }
        }

        // TODO: if that leaf is not present, we need to work it out based on what microarch we're
        // running on.
        None
    }
}

#[derive(PartialEq, Eq, Debug)]
pub enum Vendor {
    Unknown,
    Intel,
    Amd,
}

/// Intel and AMD microarchitectures we can expect processors we're running on to be. This does not
/// include microarchs that do not support x86_64, or die shrinks (they're considered their parent
/// microarch).
#[derive(Debug)]
pub enum Microarch {
    /*
     * Intel
     */
    Nehalem,
    Westmere,
    SandyBridge,
    IvyBridge,
    Haswell,
    Broadwell,
    Skylake,
    KabyLake,
    CoffeeLake,
    CannonLake,
    WhiskeyLake,
    AmberLake,
    /*
     * AMD
     */
    Bulldozer,
    Jaguar,
    Zen,
}

#[derive(Debug)]
pub struct ModelInfo {
    pub family: u8,
    pub model: u8,
    pub stepping: u8,

    pub extended_family: u8,
    pub extended_model: u8,
}

struct CpuidResult {
    pub a: u32,
    pub b: u32,
    pub c: u32,
    pub d: u32,
}

enum CpuidEntry {
    /// A = maximum supported standard level
    /// B,C,D = vendor ID string
    VendorId = 0x00,

    ProcessorTypeFamilyModel = 0x01,

    /// A = denominator
    /// B = numerator
    /// C = core crystal clock frequency
    ///
    /// TSC frequency = core crystal clock frequency * numerator / denominator
    TscFrequency = 0x15,
}

fn decode_vendor(vendor_id: &CpuidResult) -> Vendor {
    /*
     * We reinterpret the bytes of EBX, ECX, and EDX into the correct order to parse them as
     * a string. E.g:
     *
     *       MSB         LSB
     * EBX = 'u' 'n' 'e' 'G'
     * EDX = 'I' 'e' 'n' 'i'
     * ECX = 'l' 'e' 't' 'n'
     *
     * turns into "GenuineIntel".
     */
    union VendorRepr {
        vendor_id: [u32; 3],
        vendor_name: [u8; 12],
    };

    let vendor_repr = VendorRepr {
        vendor_id: [vendor_id.b, vendor_id.d, vendor_id.c],
    };

    match str::from_utf8(unsafe { &vendor_repr.vendor_name }) {
        Ok("GenuineIntel") => Vendor::Intel,
        Ok("AuthenticAMD") => Vendor::Amd,
        _ => Vendor::Unknown,
    }
}

fn decode_model_info() -> ModelInfo {
    let cpuid = cpuid(CpuidEntry::ProcessorTypeFamilyModel);

    let family = cpuid.a.get_bits(8..12) as u8;
    let model = cpuid.a.get_bits(4..8) as u8;
    let stepping = cpuid.a.get_bits(0..4) as u8;

    let extended_family = if family == 0xf {
        family + cpuid.a.get_bits(20..28) as u8
    } else {
        family
    };

    let extended_model = if family == 0xf || family == 0x6 {
        model + ((cpuid.a.get_bits(16..20) as u8) << 4)
    } else {
        model
    };

    ModelInfo {
        family,
        model,
        stepping,

        extended_family,
        extended_model,
    }
}

fn cpuid(entry: CpuidEntry) -> CpuidResult {
    let (a, b, c, d): (u32, u32, u32, u32);

    unsafe {
        asm!("cpuid"
         : "={eax}"(a), "={ebx}"(b), "={ecx}"(c), "={edx}"(d)
         : "{rax}"(entry as u64)
         : "eax", "ebx", "ecx", "edx"
         : "intel"
        );
    }

    CpuidResult { a, b, c, d }
}
