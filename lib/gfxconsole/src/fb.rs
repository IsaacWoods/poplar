use bit_field::BitField;
use font8x8::UnicodeFonts;

pub type Rgb32 = u32;
pub type PixelFormat = u32;

pub struct Framebuffer {
    fb: *mut PixelFormat,

    pub width: usize,
    pub height: usize,
    pub stride: usize,

    // Pixel format
    red_shift: u8,
    green_shift: u8,
    blue_shift: u8,
}

unsafe impl Send for Framebuffer {}

impl Framebuffer {
    pub fn new(
        fb: *mut u32,
        width: usize,
        height: usize,
        stride: usize,
        red_shift: u8,
        green_shift: u8,
        blue_shift: u8,
    ) -> Framebuffer {
        Framebuffer { fb, width, height, stride, red_shift, green_shift, blue_shift }
    }

    pub fn draw_rect(&mut self, start_x: usize, start_y: usize, width: usize, height: usize, fill: Rgb32) {
        assert!((start_x + width) <= self.width);
        assert!((start_y + height) <= self.height);

        let fill = self.rgb_to_pixel_format(fill);

        for y in start_y..(start_y + height) {
            for x in start_x..(start_x + width) {
                unsafe {
                    *(self.fb.offset((y * self.stride + x) as isize)) = fill;
                }
            }
        }
    }

    pub fn clear(&mut self, fill: Rgb32) {
        self.draw_rect(0, 0, self.width, self.height, fill);
    }

    pub fn draw_glyph(&mut self, key: char, x: usize, y: usize, fill: Rgb32) {
        let fill = self.rgb_to_pixel_format(fill);
        for (line, line_data) in font8x8::BASIC_FONTS.get(key).unwrap().iter().enumerate() {
            // TODO: this is amazingly inefficient. We could replace with a lookup table and multiply by the color
            // if this is too slow.
            for bit in 0..8 {
                if line_data.get_bit(bit) {
                    unsafe {
                        *(self.fb.offset(((y + line) * self.stride + (x + bit)) as isize)) = fill;
                    }
                }
            }
        }
    }

    pub fn draw_string(&mut self, string: &str, start_x: usize, start_y: usize, fill: Rgb32) {
        for (index, c) in string.chars().enumerate() {
            self.draw_glyph(c, start_x + (index * 8), start_y, fill);
        }
    }

    fn rgb_to_pixel_format(&self, color: Rgb32) -> PixelFormat {
        let r = ((color >> 16) & 0xff) as u32;
        let g = ((color >> 8) & 0xff) as u32;
        let b = (color & 0xff) as u32;
        (r << self.red_shift) | (g << self.green_shift) | (b << self.blue_shift)
    }
}
