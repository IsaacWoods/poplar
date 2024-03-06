use super::csr::{Sireg, Siselect, Stopei};
use bit_field::BitField;

/// The Incoming Message-Signalled Interrupt Controller (IMSIC) is a hardware component associated
/// with a hart that coordinates incoming message-signalled interrupts (MSIs), and signals to the
/// hart when pending interrupts need to be serviced.
///
/// Each IMSIC has a register file in memory for devices to write to (to trigger an interrupt), as
/// well as a CSR interface for the hart to configure it via. There are separate interrupt files
/// for each privilege level.
pub struct Imsic {}

impl Imsic {
    pub fn init() {
        unsafe {
            // Enable the IMSIC
            Siselect::write(Siselect::EIDELIVERY);
            Sireg::write(1);

            // Set the priority to see all interrupts
            Siselect::write(Siselect::EITHRESHOLD);
            Sireg::write(0);
        }
    }

    pub fn enable(number: usize) {
        let eie_byte = Siselect::EIE_BASE + 2 * number / 64;
        let eie_bit = number % 64;

        unsafe {
            Siselect::write(eie_byte);
            let mut value = Sireg::read();
            value.set_bit(eie_bit, true);
            Sireg::write(value);
        }
    }

    pub fn pop() -> u16 {
        let stopei = Stopei::read() as u32;
        /*
         * Bits 0..11 = interrupt priority (should actually be the same as the identity)
         * Bits 16..27 = interrupt identity
         */
        stopei.get_bits(16..27) as u16
    }
}
