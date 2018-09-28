use core::fmt;
use crate::uefi::{Event, UefiStatus};
use fixedvec::{ErrorKind::NoSpace, FixedVec};

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => (core::fmt::Write::write_fmt(unsafe { &mut *crate::SYSTEM_TABLE }.console_out, format_args!($($arg)*)).unwrap());
}

#[macro_export]
macro_rules! println {
    () => (print!("\n"));
    ($fmt:expr) => (print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (print!(concat!($fmt, "\n"), $($arg)*));
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct TextOutputMode {
    pub max_mode: i32,
    pub mode: i32,
    pub attribute: i32,
    pub cursor_column: i32,
    pub cursor_row: i32,
    pub cursor_visible: bool,
}

#[repr(C)]
pub struct TextOutput {
    pub reset: extern "win64" fn(&TextOutput, extended_verification: bool) -> UefiStatus,
    output_string: extern "win64" fn(&TextOutput, string: *const u16) -> UefiStatus,
    pub test_string: extern "win64" fn(&TextOutput, string: *const u16) -> UefiStatus,
    query_mode: extern "win64" fn(&TextOutput, usize, &mut usize, &mut usize) -> UefiStatus,
    set_mode: extern "win64" fn(&TextOutput, usize) -> UefiStatus,
    pub set_attribute: extern "win64" fn(&TextOutput, attribute: usize) -> UefiStatus,
    pub clear_screen: extern "win64" fn(&TextOutput) -> UefiStatus,
    pub set_cursor_position:
        extern "win64" fn(&TextOutput, column: usize, row: usize) -> UefiStatus,
    pub enable_cursor: extern "win64" fn(&TextOutput, visible: bool) -> UefiStatus,
    mode: &'static TextOutputMode,
}

impl TextOutput {
    fn print_null_terminated_ucs2(&self, chars: &[u16]) -> fmt::Result {
        match (self.output_string)(self, chars as *const [u16] as *const _) {
            UefiStatus::Success => Ok(()),
            _ => Err(fmt::Error),
        }
    }
}

impl fmt::Write for TextOutput {
    fn write_str(&mut self, string: &str) -> fmt::Result {
        const NULL_CHARACTER: u16 = 0x0000;
        const NULL_TERMINATED_NEWLINE: &[u16] = &[0x000d, 0x000a, NULL_CHARACTER];

        let mut print_new_line = false;

        for line in string.split('\n') {
            if print_new_line {
                self.print_null_terminated_ucs2(NULL_TERMINATED_NEWLINE)?;
            } else {
                print_new_line = true;
            }

            let buffer = &mut [NULL_CHARACTER; 256];
            let mut ucs2_chars_vec = FixedVec::new(&mut buffer[0..255]);

            for character in line.chars() {
                if character != '\r' {
                    let mut ucs2_pair_buffer = [0u16; 2];
                    for ucs2_char in character.encode_utf16(&mut ucs2_pair_buffer) {
                        match ucs2_chars_vec.push(*ucs2_char) {
                            Err(NoSpace) => {
                                // Print the string.
                                // NOTE: The buffer already ends with a null character.
                                self.print_null_terminated_ucs2(ucs2_chars_vec.as_slice())?;

                                ucs2_chars_vec.clear();
                                ucs2_chars_vec.push(*ucs2_char).unwrap();
                            }
                            _ => (),
                        }
                    }
                }
            }

            // End the string with a null character.
            // The result is ignored because the buffer ends with a null character.
            let _ = ucs2_chars_vec.push(NULL_CHARACTER);
            self.print_null_terminated_ucs2(ucs2_chars_vec.as_slice())?;
        }

        Ok(())
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct TextInputKey {
    pub scan_code: u16,
    pub unicode_char: u16,
}

#[repr(C)]
pub struct TextInput {
    pub reset: extern "win64" fn(&TextInput, bool) -> UefiStatus,
    pub read_key_stroke: extern "win64" fn(&TextInput, &mut TextInputKey) -> UefiStatus,
    pub wait_for_key: Event,
}
