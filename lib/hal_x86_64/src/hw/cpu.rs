/// This module gets and decodes information about the CPU we're running on, using the `cpuid`
/// instruction. If we're running under a hypervisor, we use the [Linux standard for
/// interacting with hypervisors][linux-hypervisors] to find out which hypervisor it is.
///
/// [linux-hypervisors]: https://lwn.net/Articles/301888/
use bit_field::BitField;
use core::{arch::x86_64::CpuidResult, str};

#[derive(Clone, Copy, Debug)]
pub struct SupportedFeatures {
    pub xsave: bool,
    pub x2apic: bool,
}

/// Describes information we know about the system we're running on.
#[derive(Clone, Debug)]
pub struct CpuInfo {
    pub max_supported_standard_level: u32,
    pub vendor: Vendor,
    pub model_info: ModelInfo,
    pub supported_features: SupportedFeatures,

    /// Information about the hypervisor we're running under, if we are. `None` if we're not
    /// running on virtualised hardware.
    pub hypervisor_info: Option<HypervisorInfo>,
}

impl CpuInfo {
    pub fn new() -> CpuInfo {
        let processor_cpuid = cpuid(CpuidEntry::ProcessorInfo);
        let vendor_id_cpuid = cpuid(CpuidEntry::VendorId);
        let vendor = decode_vendor(&vendor_id_cpuid);
        let model_info = decode_model_info(processor_cpuid.eax);
        let supported_features = decode_supported_features(processor_cpuid.ecx, processor_cpuid.edx);
        let hypervisor_info = decode_hypervisor_info();

        CpuInfo {
            max_supported_standard_level: vendor_id_cpuid.eax,
            vendor,
            model_info,
            supported_features,
            hypervisor_info,
        }
    }

    pub fn microarch(&self) -> Option<Microarch> {
        /*
         * This was patched together from a bunch of sources, and isn't tested on actual processors at all, so is
         * probably wrong/incomplete.
         */
        match self.vendor {
            Vendor::Intel if self.model_info.family == 0x6 => match self.model_info.extended_model {
                0x1a | 0x1e | 0x1f | 0x2e => Some(Microarch::Nehalem),
                0x25 | 0x2c | 0x2f => Some(Microarch::Westmere),
                0x2a | 0x2d => Some(Microarch::SandyBridge),
                0x3a | 0x3e => Some(Microarch::IvyBridge),
                0x3c | 0x3f | 0x45 | 0x46 => Some(Microarch::Haswell),
                0x3d | 0x47 | 0x56 | 0x4f => Some(Microarch::Broadwell),
                0x4e | 0x5e | 0x55 => Some(Microarch::Skylake),
                0x8e if self.model_info.stepping == 0x9 => Some(Microarch::KabyLake),
                0x8e if self.model_info.stepping == 0xa => Some(Microarch::CoffeeLake),
                0x9e if self.model_info.stepping == 0x9 => Some(Microarch::KabyLake),
                0x9e if let (0xa..=0xd) = self.model_info.stepping => Some(Microarch::CoffeeLake),
                _ => None,
            },

            Vendor::Amd if self.model_info.family == 0xf => match self.model_info.extended_family {
                0x15 => Some(Microarch::Bulldozer),
                0x16 => Some(Microarch::Jaguar),
                0x17 => match self.model_info.extended_model {
                    0x1 => Some(Microarch::Zen),  // Naples, Whitehaven, Summit Ridge, Snowy Owl
                    0x11 => Some(Microarch::Zen), // Raven Ridge, Great Horned Owl
                    0x18 => Some(Microarch::Zen), // Banded Kestrel (or Zen+ Picasso)
                    0x20 => Some(Microarch::Zen), // Dali
                    0x08 => Some(Microarch::Zen), // Colfax (Zen+), Pinnacle Ridge (Zen+)

                    0x31 => Some(Microarch::Zen2), // Rome, Castle Peak
                    0x60 => Some(Microarch::Zen2), // Renoir
                    0x71 => Some(Microarch::Zen2), // Matisse
                    0x90 => Some(Microarch::Zen2), // Van Gogh
                    _ => None,
                },
                // Family 0x18 is used for joint ventures between AMD and Chinese companies (e.g. Hygon)
                0x18 => Some(Microarch::Zen),
                0x19 => Some(Microarch::Zen3),
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

            if tsc_entry.ecx != 0 {
                return Some(tsc_entry.ecx);
            }
        }

        // TODO: if that leaf is not present, we need to work it out based on what microarch we're
        // running on.
        None
    }

    /// Calculate the TSC frequency from CPUID. This will return `None` if the TSC is not
    /// invariant, or if the required CPUID leaves are not present.
    pub fn tsc_frequency(&self) -> Option<u32> {
        /*
         * If we're running under a hypervisor, see if we've been able to work out the TSC
         * frequency from its leaves.
         */
        if let Some(ref hypervisor_info) = self.hypervisor_info {
            if let Some(apic_freq) = hypervisor_info.tsc_frequency {
                return Some(apic_freq);
            }
        }

        // TODO: we should be able to do core_crystal_clock_frequency * some_ratio to work it out
        // on newer hardware in bare-metal cases

        None
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Vendor {
    Unknown,
    Intel,
    Amd,
}

/// Intel and AMD microarchitectures we can expect processors we're running on to be. This doesn't include Intel
/// Atom microarchs, or microarches we consider (slightly arbitrarily in some cases) to be die shrinks or process
/// changes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

    /*
     * AMD
     */
    /// Bulldozer, Piledriver, Steamroller, and Excavator.
    Bulldozer,
    Jaguar,
    /// Zen, and Zen+.
    Zen,
    Zen2,
    Zen3,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ModelInfo {
    pub family: u8,
    pub model: u8,
    pub stepping: u8,

    pub extended_family: u8,
    pub extended_model: u8,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HypervisorVendor {
    Unknown,
    Kvm,
    Tcg,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HypervisorInfo {
    pub vendor: HypervisorVendor,
    pub max_leaf: u32,
    pub tsc_frequency: Option<u32>,
    pub apic_frequency: Option<u32>,
}

/// This is used to reinterpret the bytes of the vendor strings that are spread across the three
/// registers into a byte slice that can be fed to `str::from_utf8`.
///
/// For example:
///
/// ``` ignore
///       MSB         LSB
/// EBX = 'u' 'n' 'e' 'G'
/// EDX = 'I' 'e' 'n' 'i'
/// ECX = 'l' 'e' 't' 'n'
/// ```
///
/// turns into "GenuineIntel".
union VendorRepr {
    vendor_id: [u32; 3],
    vendor_name: [u8; 12],
}

#[repr(u32)]
enum CpuidEntry {
    /// A = maximum supported standard level
    /// B,D,C = vendor ID string
    VendorId = 0x00,

    /// A = Type, Family, Model, Stepping
    ///
    /// B(bits 0-7) = Brand Index
    /// B(bits 8-15) = CLFLUSH line size
    /// B(bits 16-23) = max number of addressible IDs for logical processors in this package
    /// B(bits 24-31) = initial APIC ID
    ///
    /// C = feature info (below are for individual bits. 1 = support)
    ///     0 = SSE3
    ///     19 = SSE4.1
    ///     20 = SSE4.2
    ///     21 = x2APIC
    ///     26 = XSAVE
    ///     28 = AVX
    ///     30 = RDRAND
    /// (this list only includes things we are currently interested in. Refer to
    /// https://www.felixcloutier.com/x86/cpuid#fig-3-7 for a full list)
    ///
    /// D = feature info (below are for individual bits. 1 = support)
    ///     0 = x87 FPU
    ///     4 = RDTSC and CR4.TSC
    ///     15 = CMOV
    ///     19 = CLFLUSH
    ProcessorInfo = 0x01,

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
    let vendor_repr = VendorRepr { vendor_id: [vendor_id.ebx, vendor_id.edx, vendor_id.ecx] };

    match str::from_utf8(unsafe { &vendor_repr.vendor_name }) {
        Ok("GenuineIntel") => Vendor::Intel,
        Ok("AuthenticAMD") => Vendor::Amd,
        _ => Vendor::Unknown,
    }
}

fn decode_model_info(model_info: u32) -> ModelInfo {
    let family = model_info.get_bits(8..12) as u8;
    let model = model_info.get_bits(4..8) as u8;
    let stepping = model_info.get_bits(0..4) as u8;

    let extended_family = if family == 0xf { family + model_info.get_bits(20..28) as u8 } else { family };

    let extended_model =
        if family == 0xf || family == 0x6 { model + ((model_info.get_bits(16..20) as u8) << 4) } else { model };

    ModelInfo { family, model, stepping, extended_family, extended_model }
}

fn decode_supported_features(processor_info_ecx: u32, _processor_info_edx: u32) -> SupportedFeatures {
    SupportedFeatures { xsave: processor_info_ecx.get_bit(26), x2apic: processor_info_ecx.get_bit(21) }
}

fn decode_hypervisor_info() -> Option<HypervisorInfo> {
    /*
     * First, we detect if we're running under a hypervisor at all. This is done by checking bit
     * 31 of ECX of the 0x1 cpuid leaf, which the hypervisor intercepts the access to and
     * advertises its presence.
     */
    if !cpuid(CpuidEntry::ProcessorInfo).ecx.get_bit(31) {
        return None;
    }

    /*
     * Next, we detect how many hypervisor leaves are present, and the hypervisor vendor.
     */
    let hypervisor_vendor_cpuid = cpuid(CpuidEntry::HypervisorVendor);
    let max_leaf = hypervisor_vendor_cpuid.eax;

    let vendor_repr = VendorRepr {
        vendor_id: [hypervisor_vendor_cpuid.ebx, hypervisor_vendor_cpuid.ecx, hypervisor_vendor_cpuid.edx],
    };

    let vendor = match str::from_utf8(unsafe { &vendor_repr.vendor_name }) {
        Ok("KVMKVMKVM\0\0\0") => HypervisorVendor::Kvm,
        Ok("TCGTCGTCGTCG") => HypervisorVendor::Tcg,
        _ => HypervisorVendor::Unknown,
    };

    /*
     * If cpuid has the hypervisor timing leaf, use the bus frequency of that.
     * NOTE: this is in kHz, so we convert to Hz
     * NOTE: for this to exist under KVM, the `vmware-cpuid-freq` and `invtsc` cpu flags must be
     * set.
     */
    let hypervisor_frequencies =
        if max_leaf >= 0x4000_0010 { Some(cpuid(CpuidEntry::HypervisorFrequencies)) } else { None };
    let tsc_frequency = hypervisor_frequencies.map(|f| f.eax * 1000);
    let apic_frequency = hypervisor_frequencies.map(|f| f.ebx * 1000);

    Some(HypervisorInfo { vendor, max_leaf, tsc_frequency, apic_frequency })
}

fn cpuid(entry: CpuidEntry) -> CpuidResult {
    unsafe { core::arch::x86_64::__cpuid(entry as u32) }
}
