use bnb::BitOps;
use core::{arch::asm, mem};
use hal::{
    cpu::{
        interrupt::{Exception, NUM_IDT_ENTRIES, RawHandler},
        tables::{DescriptorTablePtr, IdtEntry},
    },
    sync::Spinlock,
};

pub static IDT: Idt = Idt::new();

pub struct Idt(Spinlock<[IdtEntry; NUM_IDT_ENTRIES]>);

impl Idt {
    const fn new() -> Idt {
        Idt(Spinlock::new([IdtEntry::new(); NUM_IDT_ENTRIES]))
    }

    pub fn install_exception_handler(&self, exception: Exception, handler: RawHandler) {
        self.install_handler(exception as usize, handler);
    }

    pub fn install_handler(&self, i: usize, handler: RawHandler) {
        let addr = handler as usize;
        let entry = IdtEntry::new()
            .with(IdtEntry::OFFSET_LO, addr.bits(0..16) as u16)
            .with(IdtEntry::OFFSET_MI, addr.bits(16..32) as u16)
            .with(IdtEntry::OFFSET_HI, addr.bits(32..64) as u32)
            .with(IdtEntry::SEGMENT_SELECTOR, 0x08)
            .with(IdtEntry::IST, 0)
            .with(IdtEntry::TYPE, 0b1110)   // 64-bit interrupt gate
            .with(IdtEntry::DPL, 0)
            .with(IdtEntry::PRESENT, true);

        self.0.lock()[i] = entry;
    }

    pub fn install(&self) {
        let addr = self.0.lock().as_ptr() as usize;
        let idt_ptr = DescriptorTablePtr {
            limit: (mem::size_of::<IdtEntry>() * NUM_IDT_ENTRIES - 1) as u16,
            base: addr as u64,
        };
        unsafe { asm!("lidt [{}]", in(reg) &idt_ptr) };
    }
}
