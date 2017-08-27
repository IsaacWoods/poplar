/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use core::fmt;
use core::ptr::Unique;
use volatile::Volatile;
use spin::Mutex;

macro_rules! println
{
    ($fmt:expr) => (print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (print!(concat!($fmt, "\n"), $($arg)*));
}

macro_rules! print
{
    ($($arg:tt)*) => ({
        $crate::vga_buffer::print(format_args!($($arg)*));
    });
}

pub fn print(args : fmt::Arguments)
{
    use core::fmt::Write;
    WRITER.lock().write_fmt(args).unwrap();
}

pub fn clear_screen()
{
    for _ in 0..BUFFER_HEIGHT
    {
        println!("");
    }
}

#[allow(dead_code)]
#[repr(u8)]
#[derive(Debug,Clone,Copy)]
pub enum Color {
    Black      = 0,
    Blue       = 1,
    Green      = 2,
    Cyan       = 3,
    Red        = 4,
    Magenta    = 5,
    Brown      = 6,
    LightGray  = 7,
    DarkGray   = 8,
    LightBlue  = 9,
    LightGreen = 10,
    LightCyan  = 11,
    LightRed   = 12,
    Pink       = 13,
    Yellow     = 14,
    White      = 15,
}

#[derive(Debug,Clone,Copy)]
struct ColorCode(u8);

impl ColorCode
{
    const fn new(foreground : Color, background : Color) -> ColorCode
    {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}

#[repr(C)]
#[derive(Debug,Clone,Copy)]
struct ScreenChar
{
    ascii_char : u8,
    color_code : ColorCode,
}

const BUFFER_WIDTH  : usize = 80;
const BUFFER_HEIGHT : usize = 25;

struct Buffer
{
    chars : [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT]
}

pub struct Writer
{
    col_position : usize,
    color_code : ColorCode,
    buffer : Unique<Buffer>,
}

pub static WRITER : Mutex<Writer> = Mutex::new(Writer
                                               {
                                                    col_position : 0,
                                                    color_code : ColorCode::new(Color::LightGreen, Color::Black),
                                                    buffer : unsafe { Unique::new_unchecked(0xb8000 as *mut _) },
                                               });

impl fmt::Write for Writer
{
    fn write_str(&mut self, s : &str) -> fmt::Result
    {
        for byte in s.bytes()
        {
            self.write_byte(byte);
        }
        Ok(())
    }
}

impl Writer
{
/*    pub fn write_str(&mut self, s : &str)
    {
        for byte in s.bytes()
        {
            self.write_byte(byte);
        }
    }*/

    pub fn write_byte(&mut self, byte : u8)
    {
        match byte
        {
            b'\n' => self.new_line(),
            byte =>
            {
                if (self.col_position >= BUFFER_WIDTH)
                {
                    self.new_line();
                }

                let row = BUFFER_HEIGHT - 1;
                let col = self.col_position;
                let color_code = self.color_code;

                self.buffer().chars[row][col].write(ScreenChar
                                                    {
                                                        ascii_char : byte,
                                                        color_code : color_code,
                                                    });
                self.col_position += 1;
            }
        }
    }

    fn buffer(&mut self) -> &mut Buffer
    {
        unsafe
        {
            self.buffer.as_mut()
        }
    }

    fn new_line(&mut self)
    {
        for row in 1..BUFFER_HEIGHT
        {
            for col in 0..BUFFER_WIDTH
            {
                let buffer = self.buffer();
                let character = buffer.chars[row][col].read();
                buffer.chars[row-1][col].write(character);
            }
        }

        self.clear_row(BUFFER_HEIGHT-1);
        self.col_position = 0;
    }

    fn clear_row(&mut self, row : usize)
    {
        let blank = ScreenChar
                    {
                        ascii_char: b' ',
                        color_code : self.color_code,
                    };
        for col in 0..BUFFER_WIDTH
        {
            self.buffer().chars[row][col].write(blank);
        }
    }
}
