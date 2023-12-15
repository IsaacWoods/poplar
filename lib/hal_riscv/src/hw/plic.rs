use bit_field::BitField;
use volatile::Volatile;

const NUM_CONTEXTS: usize = 15872;

/// The Platform-Level Interrupt Controller (PLIC) distributes received interrupts to targets,
/// generally HART contexts. Its interface is described [here](https://github.com/riscv/riscv-plic-spec/blob/master/riscv-plic.adoc).
#[repr(C)]
pub struct Plic {
    source_priorities: Volatile<[u32; 1024]>,
    interrupt_pending: Volatile<[u32; 32]>,
    _padding0: [u8; 0xf80],
    interrupt_enable: [ContextInterruptEnable; NUM_CONTEXTS],
    /// This includes the first reserved field of the context threshold/claim area
    _padding1: [u8; 0xe000],
    threshold_and_claim: [ThresholdAndClaim; NUM_CONTEXTS],
}

impl Plic {
    pub fn init(&self, num_interrupts: usize) {
        for i in 0..num_interrupts {
            self.set_source_priority(i, 0);
        }

        // TODO: for each valid context, disable all the interrupts and set priority thresholds to 0
    }

    pub fn set_source_priority(&self, source: usize, priority: usize) {
        // TODO: check priority is within the platform-specific supported range
        self.source_priorities[source].write(priority as u32);
    }

    pub fn enable_interrupt(&self, context: usize, source: usize) {
        self.interrupt_enable[context].enable(source);
    }

    pub fn set_context_threshold(&self, context: usize, threshold: u32) {
        self.threshold_and_claim[context].priority_threshold.write(threshold);
    }

    /// Claim an interrupt on the given interrupt context. Returns the ID of the highest priority
    /// pending interrupt, or zero if there is no pending interrupt. Automatically clears the
    /// corresponding interrupt pending bit.
    pub fn claim_interrupt(&self, context: usize) -> u32 {
        self.threshold_and_claim[context].claim_complete.read()
    }

    /// Signal to the PLIC that the given interrupt has been handled. This is required to receive
    /// another interrupt of the same type.
    pub fn complete_interrupt(&self, context: usize, interrupt: u32) {
        self.threshold_and_claim[context].claim_complete.write(interrupt);
    }
}

#[repr(C)]
pub struct ContextInterruptEnable(Volatile<[u32; 32]>);

impl ContextInterruptEnable {
    pub fn enable(&self, source: usize) {
        let mut value = self.0[source / 32].read();
        value.set_bit(source % 32, true);
        self.0[source / 32].write(value);
    }

    pub fn disable(&self, source: usize) {
        let mut value = self.0[source / 32].read();
        value.set_bit(source % 32, false);
        self.0[source / 32].write(value);
    }
}

#[repr(C)]
pub struct ThresholdAndClaim {
    priority_threshold: Volatile<u32>,
    claim_complete: Volatile<u32>,
    /// This includes the first reserved field of the next context's area
    _padding0: [u8; 0xff8],
}
