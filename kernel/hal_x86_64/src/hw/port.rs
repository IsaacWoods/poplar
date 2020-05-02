use core::marker::PhantomData;

/// Implemented by the types used to represent 8-bit, 16-bit, and 32-bit IO ports. Should not be
/// implemented for any types apart from `u8`, `u16`, and `u32`.
pub trait PortSize {
    unsafe fn port_read(port: u16) -> Self;
    unsafe fn port_write(port: u16, value: Self);
}

impl PortSize for u8 {
    unsafe fn port_read(port: u16) -> u8 {
        let result: u8;
        llvm_asm!("in al, dx"
         : "={al}"(result)
         : "{dx}"(port)
         :
         : "volatile", "intel"
        );
        result
    }

    unsafe fn port_write(port: u16, value: u8) {
        llvm_asm!("out dx, al"
             :
             : "{dx}"(port), "{al}"(value)
             :
             : "volatile", "intel");
    }
}

impl PortSize for u16 {
    unsafe fn port_read(port: u16) -> u16 {
        let result: u16;
        llvm_asm!("in ax, dx"
         : "={ax}"(result)
         : "{dx}"(port)
         :
         : "volatile", "intel"
        );
        result
    }

    unsafe fn port_write(port: u16, value: u16) {
        llvm_asm!("out dx, ax"
         :
         : "{dx}"(port), "{ax}"(value)
         :
         : "volatile", "intel"
        );
    }
}

impl PortSize for u32 {
    unsafe fn port_read(port: u16) -> u32 {
        let result: u32;
        llvm_asm!("in eax, dx"
         : "={eax}"(result)
         : "{dx}"(port)
         :
         : "volatile", "intel"
        );
        result
    }

    unsafe fn port_write(port: u16, value: u32) {
        llvm_asm!("out dx, eax"
         :
         : "{dx}"(port), "{eax}"(value)
         :
         : "volatile", "intel"
        );
    }
}

/// Represents an IO port that can be read and written to using the `in` and `out` instructions.
pub struct Port<T: PortSize> {
    port: u16,
    phantom: PhantomData<T>,
}

impl<T: PortSize> Port<T> {
    /// Create a new `Port` at the specified I/O address. Unsafe because writing to random IO ports
    /// is bad.
    pub const unsafe fn new(port: u16) -> Port<T> {
        Port { port, phantom: PhantomData }
    }

    pub unsafe fn read(&self) -> T {
        T::port_read(self.port)
    }

    pub unsafe fn write(&mut self, value: T) {
        T::port_write(self.port, value);
    }
}
