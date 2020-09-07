use super::port::Port;

pub struct Pic {
    primary_command: Port<u8>,
    primary_data: Port<u8>,
    secondary_command: Port<u8>,
    secondary_data: Port<u8>,
}

impl Pic {
    pub const unsafe fn new() -> Pic {
        Pic {
            primary_command: Port::new(0x20),
            primary_data: Port::new(0x21),
            secondary_command: Port::new(0xa0),
            secondary_data: Port::new(0xa1),
        }
    }

    /// Remap and disable the PIC. It is necessary to remap the PIC even if we don't want to use it because
    /// otherwise spurious interrupts can cause exceptions.
    pub fn remap_and_disable(&mut self, primary_vector_offset: u8, secondary_vector_offset: u8) {
        unsafe {
            /*
             * 0x80 is a port used by POST. It shouldn't do anything, but it'll take long enough to execute writes
             * to it that we should block for long enough for the PICs to actually do what we ask them to.
             */
            let mut wait_port: Port<u8> = Port::new(0x80);
            let mut wait = || wait_port.write(0);

            // Tell the PICs to start their initialization sequences in cascade mode
            self.primary_command.write(0x11);
            self.secondary_command.write(0x11);
            wait();

            // Tell the PICs their new interrupt vectors
            self.primary_data.write(primary_vector_offset);
            self.secondary_data.write(secondary_vector_offset);
            wait();

            // Tell the primary PIC that the secondary is at IRQ2
            self.primary_data.write(0b100);
            wait();

            // Tell the secondary PIC its cascade identity
            self.secondary_data.write(0b10);
            wait();

            // Tell the PICs to go into 8086/88 MCS-80/85 mode
            self.primary_data.write(0x1);
            self.secondary_data.write(0x1);
            wait();

            // Mask both PICs
            self.primary_data.write(0xff);
            self.secondary_data.write(0xff);
        }
    }
}
