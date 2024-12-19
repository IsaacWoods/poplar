#![no_std]

extern crate alloc;

pub mod fb;
pub use fb::{Framebuffer, Rgb32};

use alloc::vec::Vec;
use core::fmt;

const GLYPH_SIZE: usize = 8;

pub struct GfxConsole {
    pub framebuffer: Framebuffer,
    bg_color: Rgb32,
    text_color: Rgb32,
    cursor_x: usize,
    cursor_y: usize,
    width: usize,
    height: usize,
    cells: Vec<Cell>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Cell {
    c: char,
    fg: Rgb32,
    bg: Rgb32,
}

impl GfxConsole {
    pub fn new(mut framebuffer: Framebuffer, bg_color: Rgb32, text_color: Rgb32) -> GfxConsole {
        let width = framebuffer.width / GLYPH_SIZE;
        let height = framebuffer.height / GLYPH_SIZE;
        let mut cells = Vec::with_capacity(width * height);

        for _ in 0..(width * height) {
            cells.push(Cell { c: ' ', fg: text_color, bg: bg_color });
        }

        framebuffer.clear(bg_color);
        GfxConsole { framebuffer, bg_color, text_color, cursor_x: 0, cursor_y: 0, width, height, cells }
    }

    pub fn clear(&mut self) {
        self.framebuffer.clear(self.bg_color);
        self.cursor_x = 0;
        self.cursor_y = 0;

        for i in 0..(self.width * self.height) {
            self.cells[i] = Cell { c: ' ', fg: self.text_color, bg: self.bg_color };
        }
    }

    #[inline(always)]
    pub fn put_cell(&mut self, x: usize, y: usize, c: Cell) {
        self.cells[y * self.width + x] = c;
        self.framebuffer.draw_glyph(c.c, x * GLYPH_SIZE, y * GLYPH_SIZE, c.fg);
    }
}

impl fmt::Write for GfxConsole {
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
        /*
         * We include a small font that only includes ASCII characters, which also allows us to take some shortcuts
         * here.
         */
        assert!(s.is_ascii());

        for c in s.chars() {
            match c {
                '\n' => {
                    self.cursor_x = 0;
                    self.cursor_y += 1;
                }
                '\x08' => {
                    // XXX: this is a backspace ('\b'), but Rust doesn't have an escape for it
                    self.cursor_x -= 1;
                }
                '\x7f' => {
                    /*
                     * This is an ASCII `DEL` code, which deletes the last character. It is
                     * produced when backspace on a keyboard is pressed.
                     */
                    self.cursor_x -= 1;
                    self.cells[self.cursor_y * self.width + self.cursor_x] =
                        Cell { c: ' ', fg: self.text_color, bg: self.bg_color };
                    self.framebuffer.draw_rect(
                        self.cursor_x * GLYPH_SIZE,
                        self.cursor_y * GLYPH_SIZE,
                        GLYPH_SIZE,
                        GLYPH_SIZE,
                        self.bg_color,
                    );
                }

                _ => {
                    self.put_cell(
                        self.cursor_x,
                        self.cursor_y,
                        Cell { c, fg: self.text_color, bg: self.bg_color },
                    );
                    self.cursor_x += 1;
                }
            }

            /*
             * If we've reached the end of the line, advance to the next line.
             */
            if self.cursor_x == self.width {
                self.cursor_x = 0;
                self.cursor_y += 1;
            }

            /*
             * If we've reached the end of the screen, scroll the console up.
             */
            if self.cursor_y == self.height {
                self.framebuffer.clear(self.bg_color);

                // Copy each line up one, minus the last line
                for y in 0..(self.height - 1) {
                    for x in 0..self.width {
                        let cell_below = self.cells[(y + 1) * self.width + x];
                        self.put_cell(x, y, cell_below);
                    }
                }

                // Clear the last line
                for x in 0..self.width {
                    self.cells[(self.height - 1) * self.width + x] =
                        Cell { c: ' ', fg: self.text_color, bg: self.bg_color };
                }
                self.cursor_x = 0;
                self.cursor_y -= 1;
            }
        }

        Ok(())
    }
}
