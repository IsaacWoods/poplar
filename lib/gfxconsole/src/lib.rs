#![no_std]

pub mod fb;

pub use fb::{Bgr32, Format, Framebuffer, Pixel, Rgb32};

use core::fmt;

type Cell = usize;

const GLYPH_SIZE: usize = 8;

pub struct GfxConsole<F>
where
    F: Format,
{
    pub framebuffer: Framebuffer<F>,
    bg_color: Pixel<F>,
    text_color: Pixel<F>,
    cursor_x: Cell,
    cursor_y: Cell,
    width: Cell,
    height: Cell,
}

impl<F> GfxConsole<F>
where
    F: Format,
{
    pub fn new(framebuffer: Framebuffer<F>, bg_color: Pixel<F>, text_color: Pixel<F>) -> GfxConsole<F> {
        let width = framebuffer.width / GLYPH_SIZE;
        let height = framebuffer.height / GLYPH_SIZE;

        framebuffer.clear(bg_color);
        GfxConsole { framebuffer, bg_color, text_color, cursor_x: 0, cursor_y: 0, width, height }
    }

    pub fn clear(&mut self) {
        self.framebuffer.clear(self.bg_color);
        self.cursor_x = 0;
        self.cursor_y = 0;
    }
}

impl<F> fmt::Write for GfxConsole<F>
where
    F: Format,
{
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
                    self.framebuffer.draw_rect(
                        self.cursor_x * GLYPH_SIZE,
                        self.cursor_y * GLYPH_SIZE,
                        GLYPH_SIZE,
                        GLYPH_SIZE,
                        self.bg_color,
                    );
                }

                _ => {
                    self.framebuffer.draw_glyph(
                        c,
                        self.cursor_x * GLYPH_SIZE,
                        self.cursor_y * GLYPH_SIZE,
                        self.text_color,
                    );

                    self.cursor_x += 1;
                }
            }

            /*
             * If we've reached the end of the line, or if the character is '\n', advance to the next line.
             */
            if self.cursor_x == self.width {
                self.cursor_x = 0;
                self.cursor_y += 1;
            }

            /*
             * If we've reached the end of the screen, scroll the console up.
             */
            if self.cursor_y == self.height {
                // TODO: scrolling is somehow too hard for us rn so just clear the screen and start again
                self.clear();

                // // TODO: uhh how do we do this non-badly - the naive way would be need to read from video memory
                // // which is meant to be really terrible to do.
                // // TODO: this also falls over badly if stride!=width; we should probably do it line by line
                // let dest_pixels: &mut [Pixel<F>] = unsafe {
                //     // TODO: lmao this is actually UB because we end up aliasing the framebuffer data. Def do line
                //     // by line
                //     slice::from_raw_parts_mut(
                //         self.framebuffer.ptr,
                //         self.framebuffer.stride * self.framebuffer.height,
                //     )
                // };
                // let source_pixels: &[Pixel<F>] = {
                //     let start = self.framebuffer.stride;
                //     let num_pixels = (self.framebuffer.stride * self.framebuffer.height) - start;
                //     unsafe { slice::from_raw_parts(self.framebuffer.ptr.offset(start as isize), num_pixels) }
                // };
                // dest_pixels.copy_from_slice(source_pixels);

                // // Clear the last line
                // self.framebuffer.draw_rect(
                //     0,
                //     self.framebuffer.width,
                //     self.framebuffer.height - GLYPH_SIZE,
                //     GLYPH_SIZE,
                //     self.bg_color,
                // );
            }
        }

        Ok(())
    }
}
