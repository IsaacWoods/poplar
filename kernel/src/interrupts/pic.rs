/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use rustos_common::port::Port;

struct Pic
{
    vector_offset    : u8,
    command_port     : Port<u8>,
    data_port        : Port<u8>
}

impl Pic
{
    unsafe fn send_eoi(&mut self)
    {
        self.command_port.write(0x20);
    }
}

pub struct PicPair(Pic,Pic);

impl PicPair
{
    pub const unsafe fn new(master_offset : u8, slave_offset : u8) -> PicPair
    {
        PicPair
        (
            Pic  // Master PIC
            {
                vector_offset    : master_offset,
                command_port     : Port::new(0x20),
                data_port        : Port::new(0x21),
            },

            Pic  // Slave PIC
            {
                vector_offset    : slave_offset,
                command_port     : Port::new(0xA0),
                data_port        : Port::new(0xA1),
            }
        )
    }

    pub unsafe fn remap(&mut self)
    {
        /*
         * 0x80 is a port used by POST, that theoretically shouldn't actually do anything but block
         * for long enough for the PICs to actually do what we ask them to.
         */
        let mut wait_port : Port<u8> = Port::new(0x80);
        let mut wait = || { wait_port.write(0) };

        // Save the masks of the master and slave PICS before initialisation
        let master_mask = self.0.data_port.read();
        let slave_mask  = self.1.data_port.read();

        // Tell the PICs to start their initialisation sequences (in cascade mode)
        self.0.command_port.write(0x11);                wait();
        self.1.command_port.write(0x11);                wait();

        // Tell the PICs their interrupt vector offsets
        self.0.data_port.write(self.0.vector_offset);   wait();
        self.1.data_port.write(self.1.vector_offset);   wait();
        
        // Tell the master PIC that the slave is at IRQ2 (0000 0100)
        self.0.data_port.write(0x4);                    wait();

        // Tell the slave its cascade identity (0000 0010)
        self.1.data_port.write(0x2);                    wait();

        // Tell the PICs to be in 8086/88 MCS-80/85 mode
        self.0.data_port.write(0x1);                    wait();
        self.1.data_port.write(0x1);                    wait();

        // Restore the masks
        self.0.data_port.write(master_mask);
        self.1.data_port.write(slave_mask);
    }

    pub unsafe fn send_eoi(&mut self, id : u8)
    {
        if id >= self.1.vector_offset
        {
            self.1.send_eoi();
        }

        self.0.send_eoi();
    }
}
