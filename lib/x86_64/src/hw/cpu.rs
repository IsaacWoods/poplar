/// This module gets and decodes information about the CPU we're running on, using the `cpuid`
/// instruction. If we're running under a hypervisor, we use the [Linux standard for
/// interacting with hypervisors][linux-hypervisors].
///
/// [linux-hypervisors]: https://lwn.net/Articles/301888/
use bit_field::BitField;
use core::str;

/// Describes information we know about the system we're running on.
pub struct CpuInfo {
    pub max_supported_standard_level: u32,
    pub vendor: Vendor,
    pub model_info: ModelInfo,

    /// Information about the hypervisor we're running under, if we are. `None` if we're not
    /// running on virtualised hardware.
    pub hypervisor_info: Option<HypervisorInfo>,
}

impl CpuInfo {
    pub fn new() -> CpuInfo {
        let vendor_id_cpuid = cpuid(CpuidEntry::VendorId);
        let vendor = decode_vendor(&vendor_id_cpuid);
        let model_info = decode_model_info();
        let hypervisor_info = decode_hypervisor_info();

        CpuInfo { max_supported_standard_level: vendor_id_cpuid.a, vendor, model_info, hypervisor_info }
    }

    pub fn microarch(&self) -> Option<Microarch> {
        match self.vendor {
            Vendor::Intel if self.model_info.family == 0x6 => match self.model_info.extended_model {
                0x1a | 0x1e | 0x1f | 0x2e => Some(Microarch::Nehalem),
                0x25 | 0x2c | 0x2f => Some(Microarch::Westmere),
                0x2a | 0x2d => Some(Microarch::SandyBridge),
                0x3a | 0x3e => Some(Microarch::IvyBridge),
                0x3c | 0x3f | 0x45 | 0x46 => Some(Microarch::Haswell),
                0x3d | 0x47 | 0x56 | 0x4f => Some(Microarch::Broadwell),
                0x4e | 0x5e | 0x55 => Some(Microarch::Skylake),
                0x8e | 0x9e => Some(Microarch::KabyLake),

                _ => None,
            },

            Vendor::Amd if self.model_info.family == 0xf => match self.model_info.extended_family {
                0x15 => Some(Microarch::Bulldozer),
                0x16 => Some(Microarch::Jaguar),
                0x17 => Some(Microarch::Zen),
                _ => None,
            },

            Vendor::Intel => None,
            Vendor::Amd => None,
            Vendor::Unknown => None,
        }
    }

    /// Get the frequency the APIC runs at (in Hz), if we can calculate it. If this returns `None`,
    /// we have to use another timer to work this out.
    pub fn apic_frequency(&self) -> Option<u32> {
        /*
         * If we're running under a hypervisor, see if we've been able to work out the APIC
         * frequency from its leaves.
         */
        if let Some(ref hypervisor_info) = self.hypervisor_info {
            if let Some(apic_freq) = hypervisor_info.apic_frequency {
                return Some(apic_freq);
            }
        }

        /*
         * If the `cpuid` info contains a non-zero core crystal clock frequency, return that.
         */
        if self.max_supported_standard_level >= 0x15 {
            let tsc_entry = cpuid(CpuidEntry::TscFrequency);

            if tsc_entry.c != 0 {
                return Some(tsc_entry.c);
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

#[derive(Debug, PartialEq, Eq)]
pub enum HypervisorVendor {
    Unknown,
    Kvm,
}

#[derive(Debug)]
pub struct HypervisorInfo {
    vendor: HypervisorVendor,
    max_leaf: u32,
    apic_frequency: Option<u32>,
}

/// This is used to reinterpret the bytes of the vendor strings that are spread across the three
/// registers into a byte slice that can be fed to `str::from_utf8`.
///
/// For example:
///
///       MSB         LSB
/// EBX = 'u' 'n' 'e' 'G'
/// EDX = 'I' 'e' 'n' 'i'
/// ECX = 'l' 'e' 't' 'n'
///
/// turns into "GenuineIntel".
union VendorRepr {
    vendor_id: [u32; 3],
    vendor_name: [u8; 12],
}

struct CpuidResult {
    pub a: u32,
    pub b: u32,
    pub c: u32,
    pub d: u32,
}

enum CpuidEntry {
    /// A = maximum supported standard level
    /// B,D,C = vendor ID string
    VendorId = 0x00,

    ProcessorTypeFamilyModel = 0x01,

    /// A = denominator
    /// B = numerator
    /// C = core crystal clock frequency
    ///
    /// TSC frequency = core crystal clock frequency * numerator / denominator
    TscFrequency = 0x15,

    /// A = max hypervisor leaf
    /// B,C,D = vendor ID string
    HypervisorVendor = 0x4000_0000,

    /// A = (virtual) TSC frequency
    /// B = (virtual) bus (local APIC timer) frequency in kHz
    HypervisorFrequencies = 0x4000_0010,
}

fn decode_vendor(vendor_id: &CpuidResult) -> Vendor {
    let vendor_repr = VendorRepr { vendor_id: [vendor_id.b, vendor_id.d, vendor_id.c] };

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

    let extended_family = if family == 0xf { family + cpuid.a.get_bits(20..28) as u8 } else { family };

    let extended_model =
        if family == 0xf || family == 0x6 { model + ((cpuid.a.get_bits(16..20) as u8) << 4) } else { model };

    ModelInfo { family, model, stepping, extended_family, extended_model }
}

fn decode_hypervisor_info() -> Option<HypervisorInfo> {
    /*
     * First, we detect if we're running under a hypervisor at all. This is done by checking bit
     * 31 of ECX of the 0x1 cpuid leaf, which the hypervisor intercepts the access to and
     * advertises its presence.
     */
    if !cpuid(CpuidEntry::ProcessorTypeFamilyModel).c.get_bit(31) {
        return None;
    }

    /*
     * Next, we detect how many hypervisor leaves are present, and the hypervisor vendor.
     */
    let hypervisor_vendor_cpuid = cpuid(CpuidEntry::HypervisorVendor);
    let max_leaf = hypervisor_vendor_cpuid.a;

    let vendor_repr = VendorRepr {
        vendor_id: [hypervisor_vendor_cpuid.b, hypervisor_vendor_cpuid.c, hypervisor_vendor_cpuid.d],
    };

    let vendor = match str::from_utf8(unsafe { &vendor_repr.vendor_name }) {
        Ok("KVMKVMKVM\0\0\0") => HypervisorVendor::Kvm,
        _ => HypervisorVendor::Unknown,
    };

    /*
     * If cpuid has the hypervisor timing leaf, use the bus frequency of that.
     * NOTE: this is in kHz, so we convert to Hz
     * NOTE: for this to exist under KVM, the `vmware-cpuid-freq` and `invtsc` cpu flags must be
     * set.
     */
    let apic_frequency =
        if max_leaf >= 0x4000_0010 { Some(cpuid(CpuidEntry::HypervisorFrequencies).b * 1000) } else { None };

    Some(HypervisorInfo { vendor, max_leaf, apic_frequency })
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
