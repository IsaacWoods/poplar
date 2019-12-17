use crate::uefi::{Char16, Guid, Status};

const CONSOLE_OUT_GUID: Guid =
    Guid { a: 0x387477c2, b: 0x69c7, c: 0x11d2, d: [0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b] };

#[repr(C)]
pub struct ConsoleOut {
    reset: extern "win64" fn(this: &ConsoleOut, extended: bool) -> Status,
    output_string: unsafe extern "win64" fn(this: &ConsoleOut, string: *const Char16) -> Status,
    // TODO: more stuff here that we don't need
}

impl ConsoleOut {
    pub fn write_str(&self, s: &str) -> Result<(), ()> {
        const BUFFER_SIZE: usize = 128;
        // Add one to the buffer size to leave space for the null-terminator.
        let mut buffer = [0; BUFFER_SIZE + 1];
        let mut i = 0;

        let mut add_char = |c: u16| {
            /*
             * UEFI only supports the UCS-2 subset, so we don't need to deal with characters that use multiple
             * code-points.
             */
            buffer[i] = c;
            i += 1;

            if i == BUFFER_SIZE {
                buffer[i] = 0;
                self.write_buffer(&buffer[0..=i]).as_result().map_err(|_| ucs2::Error::BufferOverflow)?;
                i = 0;
            }
            Ok(())
        };

        ucs2::encode_with(s, |c| {
            if c == '\n' as u16 {
                add_char('\r' as u16)?;
            }
            add_char(c)
        })
        .map_err(|_| ())?;

        self.write_buffer(&buffer);
        Ok(())
    }

    fn write_buffer(&self, buffer: &[u16]) -> Status {
        unsafe { (self.output_string)(self, buffer as *const [u16] as *const Char16) }
    }
}
