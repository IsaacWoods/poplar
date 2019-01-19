use super::port::Port;

pub struct Pic {
    master_command: Port<u8>,
    master_data: Port<u8>,
    slave_command: Port<u8>,
    slave_data: Port<u8>,
}

impl Pic {
    pub const unsafe fn new() -> Pic {
        Pic {
            master_command: Port::new(0x20),
            master_data: Port::new(0x21),
            slave_command: Port::new(0xa0),
            slave_data: Port::new(0xa1),
        }
    }

    /// Remap and disable the PIC. It is necessary to remap the PIC even if we don't want to use it
    /// because otherwise spurious interrupts can cause exceptions.
    pub fn remap_and_disable(&mut self, master_vector_offset: u8, slave_vector_offset: u8) {
        unsafe {
            /*
             * 0x80 is a port used by POST. It shouldn't do anything, but it'll take long enough
             * to execute writes to it that we should block for long enough for the
             * PICs to actually do what we ask them to.
             */
            let mut wait_port: Port<u8> = Port::new(0x80);
            let mut wait = || wait_port.write(0);

            // Tell the PICs to start their initialization sequences in cascade mode
            self.master_command.write(0x11);
            self.slave_command.write(0x11);
            wait();

            // Tell the PICs their new interrupt vectors
            self.master_data.write(master_vector_offset);
            self.slave_data.write(slave_vector_offset);
            wait();

            // Tell the master PIC that the slave is at IRQ2
            self.master_data.write(0b100);
            wait();

            // Tell the slave PIC its cascade identity
            self.slave_data.write(0b10);
            wait();

            // Tell the PICs to go into 8086/88 MCS-80/85 mode
            self.master_data.write(0x1);
            self.slave_data.write(0x1);
            wait();

            // Mask both PICs
            self.master_data.write(0xff);
            self.slave_data.write(0xff);
        }
    }
}
