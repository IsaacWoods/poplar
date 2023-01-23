/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

use bit_field::BitField;
use core::arch::asm;
use hal::memory::{PAddr, VAddr};

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
    SupervisorSoftware,
    SupervisorTimer,
    SupervisorExternal,
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

impl Scause {
    pub fn read() -> Scause {
        let value: usize;
        unsafe {
            asm!("csrr {}, scause", out(reg) value);
        }

        if value.get_bit(usize::BITS as usize - 1) {
            // It's an interrupt
            match value.get_bits(0..(usize::BITS as usize - 1)) {
                0 => panic!("Reserved interrupt!"),
                1 => Scause::SupervisorSoftware,
                2..=4 => panic!("Reserved interrupt!"),
                5 => Scause::SupervisorTimer,
                6..=7 => panic!("Reserved interrupt!"),
                8 => Scause::SupervisorExternal,
                10..=15 => panic!("Reserved interrupt!"),
                platform => Scause::PlatformInterrupt(platform),
            }
        } else {
            // It's an exception
            match value.get_bits(0..(usize::BITS as usize - 1)) {
                0 => Scause::InstructionAddressMisaligned,
                1 => Scause::InstructionAccessFault,
                2 => Scause::IllegalInstruction,
                3 => Scause::Breakpoint,
                4 => Scause::LoadAddressMisaligned,
                5 => Scause::LoadAccessFault,
                6 => Scause::StoreAddressMisaligned,
                7 => Scause::StoreAccessFault,
                8 => Scause::UEnvironmentCall,
                9 => Scause::SEnvironmentCall,
                10..=11 => panic!("Reserved exception!"),
                12 => Scause::InstructionPageFault,
                13 => Scause::LoadPageFault,
                14 => panic!("Reserved exception!"),
                15 => Scause::StorePageFault,
                16..=23 => panic!("Reserved exception!"),
                custom @ 24..=31 => Scause::CustomException(custom),
                32..=47 => panic!("Reserved exception!"),
                custom @ 48..=63 => Scause::CustomException(custom),
                _ => panic!("Reserved exception!"),
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Sepc(VAddr);

impl Sepc {
    pub fn read() -> Sepc {
        let value: usize;
        unsafe {
            asm!("csrr {}, sepc", out(reg) value);
        }
        Sepc(VAddr::new(value))
    }
}
