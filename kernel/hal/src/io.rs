use core::{arch::asm, marker::PhantomData};

/// Represents allowable widths of reads and writes to an I/O port on x86_64. Implemented by `u8`,
/// `u16`, and `u32`.
pub trait PortSize {
    unsafe fn read(port: u16) -> Self;
    unsafe fn write(port: u16, value: Self);
}

impl PortSize for u8 {
    unsafe fn read(port: u16) -> Self {
        let result: u8;
        unsafe {
            asm!("in al, dx", in("dx") port, out("al") result);
        }
        result
    }

    unsafe fn write(port: u16, value: u8) {
        unsafe {
            asm!("out dx, al", in("dx") port, in("al") value);
        }
    }
}

impl PortSize for u16 {
    unsafe fn read(port: u16) -> Self {
        let result: u16;
        unsafe {
            asm!("in ax, dx", in("dx") port, out("ax") result);
        }
        result
    }

    unsafe fn write(port: u16, value: u16) {
        unsafe {
            asm!("out dx, ax", in("dx") port, in("ax") value);
        }
    }
}

impl PortSize for u32 {
    unsafe fn read(port: u16) -> Self {
        let result: u32;
        unsafe {
            asm!("in eax, dx", in("dx") port, out("eax") result);
        }
        result
    }

    unsafe fn write(port: u16, value: u32) {
        unsafe {
            asm!("out dx, eax", in("dx") port, in("eax") value);
        }
    }
}

/// A port in the I/O address space. The type parameter `S` defines the size of reads and writes to
/// the port.
pub struct IoPort<S: PortSize>(u16, PhantomData<S>);

impl<S> IoPort<S>
where
    S: PortSize,
{
    pub const unsafe fn new(port: u16) -> IoPort<S> {
        IoPort(port, PhantomData)
    }

    pub unsafe fn read(&self) -> S {
        unsafe { S::read(self.0) }
    }

    pub unsafe fn write(&self, value: S) {
        unsafe {
            S::write(self.0, value);
        }
    }
}
