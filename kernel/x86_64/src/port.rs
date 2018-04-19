/*
 * Copyright (C) 2017, Pebble Developers.
 * See LICENCE.md
 */

use core::marker::PhantomData;

pub trait PortSize
{
    unsafe fn port_read(port : u16) -> Self;
    unsafe fn port_write(port : u16, value : Self);
}

impl PortSize for u8
{
    unsafe fn port_read(port : u16) -> u8
    {
        let result : u8;
        asm!("inb %dx, %al" : "={al}"(result)
                            : "{dx}"(port)
                            :: "volatile");
        result
    }

    unsafe fn port_write(port : u16, value : u8)
    {
        asm!("outb %al, %dx" :: "{dx}"(port)
                              , "{al}"(value)
                             :: "volatile");
    }
}

impl PortSize for u16
{
    unsafe fn port_read(port : u16) -> u16
    {
        let result : u16;
        asm!("inw %dx, %ax" : "={ax}"(result)
                            : "{dx}"(port)
                            :: "volatile");
        result
    }

    unsafe fn port_write(port : u16, value : u16)
    {
        asm!("outw %ax, %dx" :: "{dx}"(port)
                              , "{ax}"(value)
                             :: "volatile");
    }
}

impl PortSize for u32
{
    unsafe fn port_read(port : u16) -> u32
    {
        let result : u32;
        asm!("inl %dx, %eax" : "={eax}"(result)
                             : "{dx}"(port)
                             :: "volatile");
        result
    }

    unsafe fn port_write(port : u16, value : u32)
    {
        asm!("outl %eax, %dx" :: "{dx}"(port)
                               , "{eax}"(value)
                              :: "volatile");
    }
}

pub struct Port<T : PortSize>
{
    port : u16,
    phantom : PhantomData<T>
}

impl<T : PortSize> Port<T>
{
    /// Create a new `Port` at the specified I/O address. Unsafe because writing to random IO ports
    /// is bad.
    pub const unsafe fn new(port : u16) -> Port<T>
    {
        Port
        {
            port,
            phantom : PhantomData,
        }
    }

    pub unsafe fn read(&self) -> T
    {
        T::port_read(self.port)
    }

    pub unsafe fn write(&mut self, value : T)
    {
        T::port_write(self.port, value);
    }
}
