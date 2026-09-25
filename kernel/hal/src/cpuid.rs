//! This module collates information about the CPU we're running on, using the `cpuid`
//! instruction. If we're running under a hypervisor, we use the [Linux standard for
//! interacting with hypervisors][linux-hypervisors] to find out which hypervisor it is.
//!
//! [linux-hypervisors]: https://lwn.net/Articles/301888/

use bnb::BitOps;
use core::{arch::x86_64::CpuidResult, str};

/// Describes information we know about the processor we're running on.
#[derive(Clone)]
pub struct CpuInfo {
    pub max_cpuid_level: u32,
    pub max_extended_cpuid_level: u32,

    pub vendor_string: VendorString,
    pub vendor: Vendor,

    pub family: u8,
    pub model: u8,
    pub stepping: u8,

    pub features: SupportedFeatures,

    pub brand_string: Option<BrandString>,

    /// Information about the hypervisor we're running under, if detected.
    pub hypervisor: Option<HypervisorInfo>,
}

impl CpuInfo {
    pub fn new() -> CpuInfo {
        let (max_cpuid_level, vendor_string) = {
            let cpuid = cpuid(CpuidEntry::VendorId);
            (cpuid.eax, VendorString::from_cpuid(cpuid))
        };
        let vendor = match vendor_string.as_str() {
            "GenuineIntel" => Vendor::Intel,
            "AuthenticAMD" => Vendor::Amd,
            _ => Vendor::Unknown,
        };

        let mut features = SupportedFeatures::new();
        let (family, model, stepping) = if max_cpuid_level >= 0x01 {
            let cpuid = cpuid(CpuidEntry::ProcessorInfo);

            let mut family = cpuid.eax.bits(8..12) as u8;
            if family == 0x0f {
                family += cpuid.eax.bits(20..28) as u8;
            }
            let mut model = cpuid.eax.bits(4..8) as u8;
            if model == 0x06 || model == 0x0f {
                model += (cpuid.eax.bits(16..20) as u8) << 4;
            }
            let stepping = cpuid.eax.bits(0..4) as u8;

            features.leaves[FeatureLeaf::Leaf1C as usize] = cpuid.ecx;
            features.leaves[FeatureLeaf::Leaf1D as usize] = cpuid.edx;

            (family, model, stepping)
        } else {
            (0, 0, 0)
        };

        if max_cpuid_level >= 0x07 {
            let cpuid = cpuid(CpuidEntry::StructuredExtendedFeatureFlags);
            features.leaves[FeatureLeaf::Leaf7_0C as usize] = cpuid.ecx;
            features.leaves[FeatureLeaf::Leaf7_0D as usize] = cpuid.edx;

            if cpuid.eax >= 0x01 {
                let subleaf = cpuid_subleaf(CpuidEntry::StructuredExtendedFeatureFlags, 1);
                features.leaves[FeatureLeaf::Leaf7_1A as usize] = subleaf.eax;
                features.leaves[FeatureLeaf::Leaf7_1B as usize] = subleaf.ebx;
                features.leaves[FeatureLeaf::Leaf7_1D as usize] = subleaf.edx;

                if cpuid.eax >= 0x02 {
                    let subleaf = cpuid_subleaf(CpuidEntry::StructuredExtendedFeatureFlags, 2);
                    features.leaves[FeatureLeaf::Leaf7_2D as usize] = subleaf.edx;
                }
            }
        }

        if max_cpuid_level >= 0x0d {
            let cpuid = cpuid(CpuidEntry::ProcessorExtendedStateEnumerationMain);
            features.leaves[FeatureLeaf::LeafD0A as usize] = cpuid.eax;
            features.leaves[FeatureLeaf::LeafD0B as usize] = cpuid.ebx;
            features.leaves[FeatureLeaf::LeafD0C as usize] = cpuid.ecx;
            features.leaves[FeatureLeaf::LeafD0D as usize] = cpuid.edx;
        }

        let max_extended_cpuid_level = cpuid(CpuidEntry::ExtendedFunctionCpuidInfo).eax;
        let brand_string = (max_extended_cpuid_level >= 0x8000_0004).then(BrandString::from_cpuid);

        let hypervisor = features.hypervisor().then(|| {
            let vendor_cpuid = cpuid(CpuidEntry::HypervisorVendor);
            let max_leaf = vendor_cpuid.eax;

            let vendor = VendorString::from_cpuid(vendor_cpuid);
            let vendor = match vendor.as_str() {
                "KVMKVMKVM\0\0\0" => HypervisorVendor::Kvm,
                "TCGTCGTCGTCG" => HypervisorVendor::Tcg,
                _ => HypervisorVendor::Unknown,
            };

            /*
             * Hypervisors can advertise bus frequencies directly (we convert from KHz to Hz).
             * NOTE: for this to exist under KVM, the `vmware-cpuid-freq` and `invtsc` cpu flags must be set.
             */
            let frequencies =
                (max_leaf >= 0x4000_0010).then(|| cpuid(CpuidEntry::HypervisorFrequencies));
            let tsc_frequency = frequencies.map(|f| f.eax * 1000);
            let apic_frequency = frequencies.map(|f| f.ebx * 1000);

            HypervisorInfo {
                vendor,
                max_leaf,
                tsc_frequency,
                apic_frequency,
            }
        });

        CpuInfo {
            max_cpuid_level,
            max_extended_cpuid_level,

            vendor_string,
            vendor,

            family,
            model,
            stepping,

            features,

            brand_string,

            hypervisor,
        }
    }

    /// Get the frequency the APIC runs at (in Hz), if we can calculate it. If this returns `None`,
    /// we have to use another timer to work this out.
    pub fn apic_frequency(&self) -> Option<u32> {
        /*
         * If we're running under a hypervisor, see if we've been able to work out the APIC
         * frequency from its leaves.
         */
        if let Some(hypervisor) = self.hypervisor
            && let Some(freq) = hypervisor.apic_frequency
        {
            return Some(freq);
        }

        /*
         * If the `cpuid` info contains a non-zero core crystal clock frequency, return that.
         */
        if self.max_cpuid_level >= 0x15 {
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
        if let Some(hypervisor) = self.hypervisor
            && let Some(freq) = hypervisor.tsc_frequency
        {
            return Some(freq);
        }
        // TODO: we should be able to do core_crystal_clock_frequency * some_ratio to work it out
        // on newer hardware in bare-metal cases

        None
    }
}

/// ``` ignore
///       MSB         LSB
/// EBX = 'u' 'n' 'e' 'G'
/// EDX = 'I' 'e' 'n' 'i'       => "GenuineIntel"
/// ECX = 'l' 'e' 't' 'n'
/// ```
#[derive(Clone, Copy)]
#[repr(C)]
pub union VendorString {
    id: [u32; 3],
    name: [u8; 12],
}

impl VendorString {
    pub fn from_cpuid(cpuid: CpuidResult) -> VendorString {
        VendorString {
            id: [cpuid.ebx, cpuid.edx, cpuid.ecx],
        }
    }

    pub fn as_str(&self) -> &str {
        str::from_utf8(unsafe { &self.name }).unwrap()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Vendor {
    Intel,
    Amd,
    Unknown,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub union BrandString {
    id: [u32; 12],
    name: [u8; 48],
}

impl BrandString {
    fn from_cpuid() -> BrandString {
        let a = cpuid(CpuidEntry::BrandString0);
        let b = cpuid(CpuidEntry::BrandString1);
        let c = cpuid(CpuidEntry::BrandString2);

        BrandString {
            id: [
                a.eax, a.ebx, a.ecx, a.edx, b.eax, b.ebx, b.ecx, b.edx, c.eax, c.ebx, c.ecx, c.edx,
            ],
        }
    }

    pub fn as_str(&self) -> &str {
        // Trim to remove padding whitespace (including trailing spaces)
        str::from_utf8(unsafe { &self.name })
            .unwrap()
            .trim_matches([' ', '\t', '\n', '\0'])
    }
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

#[repr(u32)]
pub enum CpuidEntry {
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
    /// C = feature info
    /// D = feature info
    ProcessorInfo = 0x01,

    StructuredExtendedFeatureFlags = 0x07,

    ProcessorExtendedStateEnumerationMain = 0x0d,

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

    ExtendedFunctionCpuidInfo = 0x8000_0000,
    ExtendedFeatureBits = 0x8000_0001,
    BrandString0 = 0x8000_0002,
    BrandString1 = 0x8000_0003,
    BrandString2 = 0x8000_0004,
}

pub fn cpuid(entry: CpuidEntry) -> CpuidResult {
    core::arch::x86_64::__cpuid(entry as u32)
}

pub fn cpuid_subleaf(entry: CpuidEntry, sub_leaf: u32) -> CpuidResult {
    core::arch::x86_64::__cpuid_count(entry as u32, sub_leaf)
}

#[derive(Clone)]
pub struct SupportedFeatures {
    pub leaves: [u32; FeatureLeaf::_NumLeaves as usize],
}

pub enum FeatureLeaf {
    Leaf1C,
    Leaf1D,

    Leaf7_0B,
    Leaf7_0C,
    Leaf7_0D,

    Leaf7_1A,
    Leaf7_1B,
    Leaf7_1D,

    Leaf7_2D,

    LeafD0A,
    LeafD0B,
    LeafD0C,
    LeafD0D,

    _NumLeaves,
}

macro_rules! feature_getters {
    {$(($name:ident, $leaf:ident, $bit:expr)),*} => {
    $(
        pub fn $name(&self) -> bool {
            self.leaves[FeatureLeaf::$leaf as usize].bit($bit)
        }
    )*
}}

impl SupportedFeatures {
    fn new() -> SupportedFeatures {
        SupportedFeatures {
            leaves: [0; FeatureLeaf::_NumLeaves as usize],
        }
    }

    feature_getters! {
        /*
         * Leaf 1C
         */
        (sse3,                  Leaf1C,     0),
        (pclmulqdq,             Leaf1C,     1),
        (dtes64,                Leaf1C,     2),
        (monitor_mwait,         Leaf1C,     3),
        (ds_cpl,                Leaf1C,     4),
        (vmx,                   Leaf1C,     5),
        (smx,                   Leaf1C,     6),
        (eist,                  Leaf1C,     7),
        (tm2,                   Leaf1C,     8),
        (ssse3,                 Leaf1C,     9),
        (cnxt_id,               Leaf1C,     10),
        (sdbg,                  Leaf1C,     11),
        (fma,                   Leaf1C,     12),
        (cmpxchg16b,            Leaf1C,     13),
        (xtpr_ctrl,             Leaf1C,     14),
        (pdcm,                  Leaf1C,     15),
        // Bit 16 is reserved
        (pcid,                  Leaf1C,     17),
        (dca,                   Leaf1C,     18),
        (sse4_1,                Leaf1C,     19),
        (sse4_2,                Leaf1C,     20),
        (x2apic,                Leaf1C,     21),
        (movbe,                 Leaf1C,     22),
        (popcnt,                Leaf1C,     23),
        (tsc_deadline,          Leaf1C,     24),
        (aesni,                 Leaf1C,     25),
        (xsave,                 Leaf1C,     26),
        (osxsave,               Leaf1C,     27),
        (avx,                   Leaf1C,     28),
        (f16c,                  Leaf1C,     29),
        (rdrand,                Leaf1C,     30),
        (hypervisor,            Leaf1C,     31),

        /*
         * Leaf 1D
         */
        (fpu,                   Leaf1D,     0),
        (vme,                   Leaf1D,     1),
        (de,                    Leaf1D,     2),
        (pse,                   Leaf1D,     3),
        (tsc,                   Leaf1D,     4),
        (msr,                   Leaf1D,     5),
        (pae,                   Leaf1D,     6),
        (mce,                   Leaf1D,     7),
        (cx8,                   Leaf1D,     8),
        (apic,                  Leaf1D,     9),
        // Bit 10 is reserved
        (sep,                   Leaf1D,     11),
        (mtrr,                  Leaf1D,     12),
        (pge,                   Leaf1D,     13),
        (mca,                   Leaf1D,     14),
        (cmov,                  Leaf1D,     15),
        (pat,                   Leaf1D,     16),
        (pse36,                 Leaf1D,     17),
        (psn,                   Leaf1D,     18),
        (clfsh,                 Leaf1D,     19),
        // Bit 20 is reserved
        (ds,                    Leaf1D,     21),
        (acpi,                  Leaf1D,     22),
        (mmx,                   Leaf1D,     23),
        (fxsr,                  Leaf1D,     24),
        (sse,                   Leaf1D,     25),
        (sse2,                  Leaf1D,     26),
        (ss,                    Leaf1D,     27),
        (htt,                   Leaf1D,     28),
        (tm,                    Leaf1D,     29),
        // Bit 30 is reserved
        (pbe,                   Leaf1D,     31),

        /*
         * Leaf 7 (ECX = 0) B
         */
        (fsgsbase,              Leaf7_0B,   0),
        (ia32_tsc_adj,          Leaf7_0B,   1),
        (sgx,                   Leaf7_0B,   2),
        (bmi1,                  Leaf7_0B,   3),
        (hle,                   Leaf7_0B,   4),
        (avx2,                  Leaf7_0B,   5),
        (fdp_excptn,            Leaf7_0B,   6),
        (smep,                  Leaf7_0B,   7),
        (bmi2,                  Leaf7_0B,   8),
        (enh_rep_movsb,         Leaf7_0B,   9),
        (invpcid,               Leaf7_0B,   10),
        (rtm,                   Leaf7_0B,   11),
        (rdt_m,                 Leaf7_0B,   12),
        (dep_fpu_csds,          Leaf7_0B,   13),
        (mpx,                   Leaf7_0B,   14),
        (rdt_a,                 Leaf7_0B,   15),
        (avx512f,               Leaf7_0B,   16),
        (avx512dq,              Leaf7_0B,   17),
        (rdseed,                Leaf7_0B,   18),
        (adx,                   Leaf7_0B,   19),
        (smap,                  Leaf7_0B,   20),
        (avx512ifma,            Leaf7_0B,   21),
        // Bit 22 is reserved
        (clflushopt,            Leaf7_0B,   23),
        (clwb,                  Leaf7_0B,   24),
        (intel_p_trace,         Leaf7_0B,   25),
        (avx512pf,              Leaf7_0B,   26),
        (avx512er,              Leaf7_0B,   27),
        (avx512cd,              Leaf7_0B,   28),
        (sha,                   Leaf7_0B,   29),
        (avx512bw,              Leaf7_0B,   30),
        (avx512vl,              Leaf7_0B,   31),

        /*
         * Leaf 7 (ECX = 0) C
         */
        (prefetchwt1,           Leaf7_0C,   0),
        (avx512vbmi,            Leaf7_0C,   1),
        (umip,                  Leaf7_0C,   2),
        (pku,                   Leaf7_0C,   3),
        (ospke,                 Leaf7_0C,   4),
        (waitpkg,               Leaf7_0C,   5),
        (avx512vbmi2,           Leaf7_0C,   6),
        (cet_ss,                Leaf7_0C,   7),
        (gfni,                  Leaf7_0C,   8),
        (vaes,                  Leaf7_0C,   9),
        (vpclmulqdq,            Leaf7_0C,   10),
        (avx512vnni,            Leaf7_0C,   11),
        (avx512bitalg,          Leaf7_0C,   12),
        (tme_en,                Leaf7_0C,   13),
        (avx512vpopcntdq,       Leaf7_0C,   14),
        // Bit 15 is reserved
        (la57,                  Leaf7_0C,   16),
        // Bits 17-21 form the value MAWAU used by BNDLX and BNDSTX in 64-bit modes
        (rdpid,                 Leaf7_0C,   22),
        (kl,                    Leaf7_0C,   23),
        (bus_lock_detect,       Leaf7_0C,   24),
        (cldemote,              Leaf7_0C,   25),
        // Bit 26 is reserved
        (movdiri,               Leaf7_0C,   27),
        (modir64b,              Leaf7_0C,   28),
        (enqcmd,                Leaf7_0C,   29),
        (sgx_lc,                Leaf7_0C,   30),
        (pks,                   Leaf7_0C,   31),

        /*
         * Leaf 7 (ECX = 0) D
         */
        // Bit 0 is reserved
        (sgx_keys,              Leaf7_0D,   1),
        (avx512_4vnniw,         Leaf7_0D,   2),
        (avx512_4fmaps,         Leaf7_0D,   3),
        (fast_short_rep_mov,    Leaf7_0D,   4),
        (uintr,                 Leaf7_0D,   5),
        // Bits 6-7 are reserved
        (avx512vp2intersect,    Leaf7_0D,   8),
        (srbds_ctrl,            Leaf7_0D,   9),
        (md_clear,              Leaf7_0D,   10),
        (rtm_always_abort,      Leaf7_0D,   11),
        // Bit 12 is reserved
        (rtm_force_abort,       Leaf7_0D,   13),
        (serialize,             Leaf7_0D,   14),
        (hybrid_part,           Leaf7_0D,   15),
        (tsxldtrk,              Leaf7_0D,   16),
        // Bit 17 is reserved
        (pconfig,               Leaf7_0D,   18),
        (arch_lbrs,             Leaf7_0D,   19),
        (cet_ibt,               Leaf7_0D,   20),
        // Bit 21 is reserved
        (amx_bf16,              Leaf7_0D,   22),
        (avx512fp16,            Leaf7_0D,   23),
        (amx_tile,              Leaf7_0D,   24),
        (amx_int8,              Leaf7_0D,   25),
        (ibrs,                  Leaf7_0D,   26),
        (stibp,                 Leaf7_0D,   27),
        (l1d_flush,             Leaf7_0D,   28),
        (ia32_arch_caps_msr,    Leaf7_0D,   29),
        (ia32_core_caps_msr,    Leaf7_0D,   30),
        (ssbd,                  Leaf7_0D,   31),

        /*
         * Leaf 7 (ECX = 1) A
         */
        (sha512,                Leaf7_1A,   0),
        (sm3,                   Leaf7_1A,   1),
        (sm4,                   Leaf7_1A,   2),
        // Bit 3 is reserved
        (avxvnni,               Leaf7_1A,   4),
        (avx512bf16,            Leaf7_1A,   5),
        (lass,                  Leaf7_1A,   6),
        (cmpccxadd,             Leaf7_1A,   7),
        (arch_perfmon_ext,      Leaf7_1A,   8),
        // Bit 9 is reserved
        (fast_0len_rep_movsb,   Leaf7_1A,   10),
        (fast_short_rep_stosb,  Leaf7_1A,   11),
        (fast_short_rep_cmpsb,  Leaf7_1A,   12),
        // Bits 13-18 are reserved
        (wrmsrns,               Leaf7_1A,   19),
        // Bit 20 is reserved
        (amxfp16,               Leaf7_1A,   21),
        (hreset,                Leaf7_1A,   22),
        (avx_ifma,              Leaf7_1A,   23),
        // Bits 24-25 are reserved
        (lam,                   Leaf7_1A,   26),
        (msrlist,               Leaf7_1A,   27),
        // Bits 28-29 are reserved
        (invd_disable_post_bios,Leaf7_1A,   30),
        // Bit 31 is reserved

        /*
         * Leaf 7 (ECX = 1) B
         */
        (ia32_ppin,             Leaf7_1B,   0),
        // Bits 1-2 are reserved
        (cpuidmaxval_lim_rmv,   Leaf7_1B,   3),
        // Bits 4-31 are reserved

        /*
         * Leaf 7 (ECX = 1) D
         */
        // Bits 0-3 are reserved
        (avx_vnni_int8,         Leaf7_1D,   4),
        (avx_ne_convert,        Leaf7_1D,   5),
        // Bits 6-9 are reserved
        (avx_vnni_int16,        Leaf7_1D,   10),
        // Bits 11-13 are reserved
        (prefetchi,             Leaf7_1D,   14),
        // Bits 15-16 are reserved
        (uiret_uif,             Leaf7_1D,   17),
        (cet_sss,               Leaf7_1D,   18),
        (avx10,                 Leaf7_1D,   19),
        // Bits 20-31 are reserved

        /*
         * Leaf 7 (ECX = 2) D
         */
        (psfd,                  Leaf7_2D,   0),
        (ipred_ctrl,            Leaf7_2D,   1),
        (rrsba_ctrl,            Leaf7_2D,   2),
        (ddpd_u,                Leaf7_2D,   3),
        (bhi_ctrl,              Leaf7_2D,   4),
        (mcdt_n0,               Leaf7_2D,   5),
        (uc_lock_disable,       Leaf7_2D,   6),
        (monitor_mitg_n0,       Leaf7_2D,   7)
        // Bits 8-31 are reserved
    }

    pub fn mawau(&self) -> u8 {
        self.leaves[FeatureLeaf::Leaf7_0C as usize].bits(17..21) as u8
    }
}
