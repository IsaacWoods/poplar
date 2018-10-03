use core::fmt;
use crate::boot_services::{Event, Guid, Protocol};
use crate::types::{Bool, Char16, RuntimeMemory, Status};

/// This protocol is used to obtain input from the ConsoleIn device
// TODO: implement events and use wait_for_key
#[repr(C)]
pub struct SimpleTextInput {
    pub _reset: extern "win64" fn(this: &SimpleTextInput, extended_verification: Bool) -> Status,
    pub _read_key_stroke: extern "win64" fn(this: &SimpleTextInput, key: &mut InputKey) -> Status,
    pub wait_for_key: RuntimeMemory<Event>,
}

impl SimpleTextInput {
    /// Reset the ConsoleIn device
    pub fn reset(&self, extended_verification: bool) -> Result<(), Status> {
        (self._reset)(self, Bool::from(extended_verification)).as_result()?;
        Ok(())
    }

    /// Returns the next input character
    pub fn read_key_stroke(&self) -> Result<InputKey, Status> {
        let mut key = InputKey {
            scan_code: ScanCode::Null,
            unicode_char: 0,
        };
        (self._read_key_stroke)(self, &mut key)
            .as_result()
            .map(|_| key)
    }
}

impl Protocol for SimpleTextInput {
    fn guid() -> &'static Guid {
        &SIMPLE_TEXT_INPUT_GUID
    }
}

impl fmt::Debug for SimpleTextInput {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("SimpleTextInput")
            .field("wait_for_key", &self.wait_for_key)
            .finish()
    }
}

static SIMPLE_TEXT_INPUT_GUID: Guid = Guid {
    data_1: 0x387477c1,
    data_2: 0x69c7,
    data_3: 0x11d2,
    data_4: [0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b],
};

/// Describes a keystroke
#[derive(Clone, Copy, Debug)]
pub struct InputKey {
    pub scan_code: ScanCode,
    pub unicode_char: Char16,
}

/// Represents special keys
#[derive(Clone, Copy, Debug)]
#[repr(u16)]
pub enum ScanCode {
    Null = 0x00,
    CursorUp = 0x01,
    CursorDown = 0x02,
    CursorRight = 0x03,
    CursorLeft = 0x04,
    Home = 0x05,
    End = 0x06,
    Insert = 0x07,
    Delete = 0x08,
    PageUp = 0x09,
    PageDown = 0x0a,
    Function1 = 0x0b,
    Function2 = 0x0c,
    Function3 = 0x0d,
    Function4 = 0x0e,
    Function5 = 0x0f,
    Function6 = 0x10,
    Function7 = 0x11,
    Function8 = 0x12,
    Function9 = 0x13,
    Function10 = 0x14,
    Function11 = 0x15,
    Function12 = 0x16,
    Escape = 0x17,
}

#[repr(C)]
pub struct SimpleTextOutput {
    pub _reset: extern "win64" fn(this: &SimpleTextOutput, extended_verification: Bool) -> Status,
    pub _output_string: extern "win64" fn(this: &SimpleTextOutput, string: *const Char16) -> Status,
    pub _test_string: extern "win64" fn(this: &SimpleTextOutput, string: *const Char16) -> Status,
    pub _query_mode: extern "win64" fn(
        this: &SimpleTextOutput,
        mode_number: usize,
        columns: &mut usize,
        rows: &mut usize,
    ) -> Status,
    pub _set_mode: extern "win64" fn(this: &SimpleTextOutput, mode_number: usize) -> Status,
    pub _set_attribute: extern "win64" fn(this: &SimpleTextOutput, attribute: usize) -> Status,
    pub _clear_screen: extern "win64" fn(this: &SimpleTextOutput) -> Status,
    pub _set_cursor_position:
        extern "win64" fn(this: &SimpleTextOutput, column: usize, row: usize) -> Status,
    pub _enable_cursor: extern "win64" fn(this: &SimpleTextOutput, visible: Bool) -> Status,
    pub mode: RuntimeMemory<SimpleTextOutputMode>,
}

impl SimpleTextOutput {
    pub fn reset(&self, extended_verification: bool) -> Result<(), Status> {
        (self._reset)(self, Bool::from(extended_verification)).as_result()?;
        Ok(())
    }

    /// Displays the string on the device at the current cursor location
    pub fn output_string(&self, string: &str) -> Result<(), Status> {
        exec_with_str(string, |buf| (self._output_string)(self, buf))
    }

    /// Tests to see if the ConsoleOut device supports this string
    pub fn test_string(&self, string: &str) -> Result<(), Status> {
        exec_with_str(string, |buf| (self._test_string)(self, buf))
    }

    /// Queries information concerning the output device's supported text mode
    pub fn query_mode(&self, mode_number: usize) -> Result<ModeDescriptor, Status> {
        let mut desc = ModeDescriptor::default();
        (self._query_mode)(self, mode_number, &mut desc.columns, &mut desc.rows)
            .as_result()
            .map(|_| desc)
    }

    /// Sets the current mode of the output device
    pub fn set_mode(&self, mode_number: usize) -> Result<(), Status> {
        (self._set_mode)(self, mode_number).as_result()?;
        Ok(())
    }

    pub fn set_attribute(&self, foreground: Color, background: Color) -> Result<(), Status> {
        // TODO: is this necessary, or will the implementation correctly report an error?
        if !background.is_background() {
            return Err(Status::InvalidParameter);
        }

        let attribute = ((background as usize) << 4) | (foreground as usize);

        (self._set_attribute)(self, attribute).as_result()?;
        Ok(())
    }

    pub fn clear_screen(&self) -> Result<(), Status> {
        (self._clear_screen)(self).as_result()?;
        Ok(())
    }

    pub fn set_cursor_position(&self, column: usize, row: usize) -> Result<(), Status> {
        (self._set_cursor_position)(self, column, row).as_result()?;
        Ok(())
    }

    pub fn enable_cursor(&self, visible: bool) -> Result<(), Status> {
        (self._enable_cursor)(self, Bool::from(visible)).as_result()?;
        Ok(())
    }
}

impl fmt::Debug for SimpleTextOutput {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("SimpleTextOutput")
            .field("mode", &self.mode)
            .finish()
    }
}

impl<'a> fmt::Write for &'a SimpleTextOutput {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.output_string(s).map_err(|_| fmt::Error)
    }
}

/// Converts string to Char16 and calls the given function
///
/// The UEFI spec represents strings using UTF-16, so Rust's `&str` type is not directly compatible.
/// This function properly converts a `&str` to UTF-16, then calls the given function with a pointer
/// to the UTF-16 string.
///
/// Since this is UEFI, there is no dynamic allocation, so the conversion actually happens 127
/// characters at a time using a stack-allocated buffer. Because of this, `f` may actually be called
/// more than one time, once for every 127 characters.
fn exec_with_str<F>(string: &str, f: F) -> Result<(), Status>
where
    F: Fn(*const Char16) -> Status,
{
    // Allocate a buffer to encode the string piece by piece (can't do it all at once since
    // there is no dynamic allocation in this environment)
    const BUFSIZE: usize = 128;
    let mut buf: [u16; BUFSIZE] = [0u16; BUFSIZE];
    let mut i = 0;

    // Interpret the string as UTF-16 and fill the buffer
    for c in string.chars() {
        let mut char_buf: [u16; 2] = [0u16; 2];
        buf[i] = c.encode_utf16(&mut char_buf)[0]; // TODO: this drops the second character
        i += 1;

        if i == BUFSIZE - 1 {
            // Write out the string
            // BUFSIZE - 1 ensures at least one null character is present
            // TODO: what if this returns an error code?
            f(buf.as_ptr()).as_result()?;

            // Fill the buffer back up with null characters
            for j in 0..BUFSIZE {
                buf[j] = 0u16;
            }

            // Finally, reset our iterator
            i = 0;
        }
    }

    // Flush whatever remains in the buffer
    if i != 0 {
        f(buf.as_ptr()).as_result()?;
    }

    Ok(())
}

/// Colors supported by the UEFI console
#[derive(Clone, Copy, Debug)]
#[repr(usize)]
pub enum Color {
    Black = 0x00,
    Blue = 0x01,
    Green = 0x02,
    Cyan = 0x03,
    Red = 0x04,
    Magenta = 0x05,
    Brown = 0x06,
    LightGray = 0x07,
    DarkGray = 0x08,
    LightBlue = 0x09,
    LightGreen = 0x0a,
    LightCyan = 0x0b,
    LightRed = 0x0c,
    LightMagenta = 0x0d,
    Yellow = 0x0e,
    White = 0x0f,
}

impl Color {
    /// Tells whether this is a legal background color
    ///
    /// According to the UEFI spec, only certain colors may be legally used for the console's
    /// background.
    pub fn is_background(&self) -> bool {
        match *self as usize {
            0x00...0x07 => true,
            _ => false,
        }
    }
}

/// Describes the current attributes of the output device
#[derive(Debug)]
#[repr(C)]
pub struct SimpleTextOutputMode {
    pub max_mode: i32,
    pub mode: i32,
    pub attribute: i32,
    pub cursor_column: i32,
    pub cursor_row: i32,
    pub cursor_visible: Bool,
}

/// Describes the dimensions of an output device mode
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct ModeDescriptor {
    pub columns: usize,
    pub rows: usize,
}
