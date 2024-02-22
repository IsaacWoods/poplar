/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

use bit_field::BitField;
use core::arch::asm;
use hal::memory::{PAddr, VAddr};

pub struct Time;

impl Time {
    pub fn read() -> usize {
        let value: usize;
        unsafe {
            asm!("csrr {}, time", out(reg) value);
        }
        value
    }
}

pub struct Sstatus;

impl Sstatus {
    pub fn enable_interrupts() {
        unsafe {
            asm!("csrsi sstatus, 2");
        }
    }

    pub fn disable_interrupts() {
        unsafe {
            asm!("csrci status, 2");
        }
    }

    /// Set the `SUM` bit of `sstatus`, allowing kernel code to access user-accessible memory.
    pub fn enable_user_memory_access() {
        unsafe {
            asm!("csrs sstatus, {}", in(reg) 1 << 18);
        }
    }

    /// Clear the `SUM` bit of `sstatus`, denying kernel code access to user-accessible memory.
    /// Kernel code accessing user-accessible memory will fault.
    pub fn disable_user_memory_access() {
        unsafe {
            asm!("csrc sstatus, {}", in(reg) 1 << 18);
        }
    }
}

pub struct Sip(pub usize);

impl Sip {
    pub fn read() -> Self {
        let value: usize;
        unsafe {
            asm!("csrr {}, sip", out(reg) value);
        }
        Sip(value)
    }

    pub unsafe fn write(self) {
        unsafe {
            asm!("csrw sip, {}", in(reg) self.0);
        }
    }
}

pub struct Sie(pub usize);

impl Sie {
    pub fn read() -> Self {
        let value: usize;
        unsafe {
            asm!("csrr {}, sie", out(reg) value);
        }
        Sie(value)
    }

    pub unsafe fn write(self) {
        unsafe {
            asm!("csrw sie, {}", in(reg) self.0);
        }
    }

    pub unsafe fn enable_all() {
        unsafe {
            asm!("csrw sie, {}", in(reg) (1 << 1) | (1 << 5) | (1 << 9));
        }
    }
}

/// The Supervisor Address Translation and Protection (`satp`) register controls supervisor-mode address
/// translation and protection. It contains the physical address of the root page table, plus an associated Address
/// Space Identified (ASID), which allows translation fences on an per-address-space basis.
///
/// It also specifies a mode, which dictates how addresses are translated. Available modes are `Bare`, `Sv39`,
/// `Sv48`, and `Sv57`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Satp {
    Bare,
    Sv39 { asid: u16, root: PAddr },
    Sv48 { asid: u16, root: PAddr },
    Sv57 { asid: u16, root: PAddr },
}

impl Satp {
    pub fn read() -> Self {
        let value: usize;
        unsafe {
            asm!("csrr {}, satp", out(reg) value);
        }

        let ppn = value.get_bits(0..44);
        let asid = value.get_bits(44..60) as u16;
        let mode = value.get_bits(60..64);

        let root = PAddr::new(ppn << 12).unwrap();

        match mode {
            0 => Satp::Bare,
            1..=7 => panic!("Read SATP has a reserved mode!"),
            8 => Satp::Sv39 { asid, root },
            9 => Satp::Sv48 { asid, root },
            10 => Satp::Sv57 { asid, root },
            11..=15 => panic!("Read SATP has a reserved mode!"),
            _ => unreachable!(),
        }
    }

    pub fn raw(self) -> u64 {
        match self {
            Self::Bare => 0,
            Self::Sv39 { asid, root } => {
                let mut value: u64 = 0;
                value.set_bits(0..44, usize::from(root) as u64 >> 12);
                value.set_bits(44..60, asid as u64);
                value.set_bits(60..64, 8);
                value
            }
            Self::Sv48 { asid, root } => {
                let mut value: u64 = 0;
                value.set_bits(0..44, usize::from(root) as u64 >> 12);
                value.set_bits(44..60, asid as u64);
                value.set_bits(60..64, 9);
                value
            }
            Self::Sv57 { asid, root } => {
                let mut value: u64 = 0;
                value.set_bits(0..44, usize::from(root) as u64 >> 12);
                value.set_bits(44..60, asid as u64);
                value.set_bits(60..64, 10);
                value
            }
        }
    }

    pub unsafe fn write(self) {
        unsafe {
            asm!("csrw satp, {}", in(reg) self.raw());
        }
    }
}

pub struct Stvec;

impl Stvec {
    pub fn set(trap_address: VAddr) {
        // Trap handlers on RISC-V must be aligned to `4`.
        assert!(usize::from(trap_address) % 4 == 0);

        unsafe {
            asm!("csrw stvec, {}", in(reg) usize::from(trap_address));
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scause {
    /*
     * Interrupts
     */
    SupervisorSoftwareInterrupt,
    SupervisorTimerInterrupt,
    SupervisorExternalInterrupt,
    PlatformInterrupt(usize),

    /*
     * Exceptions
     */
    InstructionAddressMisaligned,
    InstructionAccessFault,
    IllegalInstruction,
    Breakpoint,
    LoadAddressMisaligned,
    LoadAccessFault,
    StoreAddressMisaligned,
    StoreAccessFault,
    UEnvironmentCall,
    SEnvironmentCall,
    InstructionPageFault,
    LoadPageFault,
    StorePageFault,
    CustomException(usize),
}

impl TryFrom<usize> for Scause {
    type Error = ();

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        let is_interrupt = value.get_bit(usize::BITS as usize - 1);
        if is_interrupt {
            match value.get_bits(0..(usize::BITS as usize - 1)) {
                0 => Err(()),
                1 => Ok(Self::SupervisorSoftwareInterrupt),
                2..=4 => Err(()),
                5 => Ok(Self::SupervisorTimerInterrupt),
                6..=8 => Err(()),
                9 => Ok(Self::SupervisorExternalInterrupt),
                10..=15 => Err(()),
                interrupt @ 16.. => Ok(Self::PlatformInterrupt(interrupt)),
            }
        } else {
            match value.get_bits(0..(usize::BITS as usize - 1)) {
                0 => Ok(Self::InstructionAddressMisaligned),
                1 => Ok(Self::InstructionAccessFault),
                2 => Ok(Self::IllegalInstruction),
                3 => Ok(Self::Breakpoint),
                4 => Ok(Self::LoadAddressMisaligned),
                5 => Ok(Self::LoadAccessFault),
                6 => Ok(Self::StoreAddressMisaligned),
                7 => Ok(Self::StoreAccessFault),
                8 => Ok(Self::UEnvironmentCall),
                9 => Ok(Self::SEnvironmentCall),
                10..=11 => Err(()),
                12 => Ok(Self::InstructionPageFault),
                13 => Ok(Self::LoadPageFault),
                14 => Err(()),
                15 => Ok(Self::StorePageFault),
                16..=23 => Err(()),
                exception @ 24..=31 => Ok(Self::CustomException(exception)),
                32..=47 => Err(()),
                exception @ 48..=63 => Ok(Self::CustomException(exception)),
                64.. => Err(()),
            }
        }
    }
}

impl Scause {
    pub fn read() -> Scause {
        let value: usize;
        unsafe {
            asm!("csrr {}, scause", out(reg) value);
        }
        Scause::try_from(value).unwrap()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Sepc(pub VAddr);

impl Sepc {
    pub fn read() -> Sepc {
        let value: usize;
        unsafe {
            asm!("csrr {}, sepc", out(reg) value);
        }
        Sepc(VAddr::new(value))
    }
}

/// A dedicated register for the supervisor to hold whatever data it would like to. Generally used
/// to hold a pointer to a hart-local supervisor context - it can be swapped with a user register
/// at the beginning of a trap handler to provide an initial working register.
#[derive(Clone, Copy, Debug)]
pub struct Sscratch(pub VAddr);

impl Sscratch {
    pub fn read() -> Sscratch {
        let value: usize;
        unsafe {
            asm!("csrr {}, sscratch", out(reg) value);
        }
        Sscratch(VAddr::new(value))
    }

    pub unsafe fn write(addr: VAddr) {
        unsafe {
            asm!("csrw sscratch, {}", in(reg) usize::from(addr));
        }
    }
}
