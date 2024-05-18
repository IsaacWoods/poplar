use bit_field::BitField;
use core::marker::PhantomData;
use font8x8::UnicodeFonts;

pub trait Format: Clone + Copy {
    fn pixel(r: u8, g: u8, b: u8, a: u8) -> Pixel<Self>;
}

/// Represents a pixel format where each pixel is of the form `0xAABBGGRR`.
#[derive(Clone, Copy)]
pub enum Rgb32 {}
impl Format for Rgb32 {
    fn pixel(r: u8, g: u8, b: u8, a: u8) -> Pixel<Self> {
        let mut color = 0;
        color.set_bits(0..8, r as u32);
        color.set_bits(8..16, g as u32);
        color.set_bits(16..24, b as u32);
        color.set_bits(24..32, a as u32);
        Pixel(color, PhantomData)
    }
}

/// Represents a pixel format where each pixel is of the form `0xAARRGGBB`.
#[derive(Clone, Copy)]
pub enum Bgr32 {}
impl Format for Bgr32 {
    fn pixel(r: u8, g: u8, b: u8, a: u8) -> Pixel<Self> {
        let mut color = 0;
        color.set_bits(0..8, b as u32);
        color.set_bits(8..16, g as u32);
        color.set_bits(16..24, r as u32);
        color.set_bits(24..32, a as u32);
        Pixel(color, PhantomData)
    }
}

/// We only support formats with a BPP of `4`, so pixels can always be represented as a `u32`.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Pixel<F>(u32, PhantomData<F>)
where
    F: Format;

pub struct Framebuffer<F>
where
    F: Format,
{
    pub ptr: *mut Pixel<F>,
    pub width: usize,
    pub height: usize,
    pub stride: usize,
}

impl<F> Framebuffer<F>
where
    F: Format,
{
    pub fn draw_rect(&self, start_x: usize, start_y: usize, width: usize, height: usize, fill: Pixel<F>) {
        assert!((start_x + width) <= self.width);
        assert!((start_y + height) <= self.height);

        for y in start_y..(start_y + height) {
            for x in start_x..(start_x + width) {
                unsafe {
                    *(self.ptr.offset((y * self.stride + x) as isize)) = fill;
                }
            }
        }
    }

    pub fn clear(&self, clear: Pixel<F>) {
        self.draw_rect(0, 0, self.width, self.height, clear);
    }

    pub fn draw_glyph(&self, key: char, x: usize, y: usize, fill: Pixel<F>) {
        for (line, line_data) in font8x8::BASIC_FONTS.get(key).unwrap().iter().enumerate() {
            // TODO: this is amazingly inefficient. We could replace with a lookup table and multiply by the color
            // if this is too slow.
            for bit in 0..8 {
                if line_data.get_bit(bit) {
                    unsafe {
                        *(self.ptr.offset(((y + line) * self.stride + (x + bit)) as isize)) = fill;
                    }
                }
            }
        }
    }

    pub fn draw_string(&self, string: &str, start_x: usize, start_y: usize, fill: Pixel<F>) {
        for (index, c) in string.chars().enumerate() {
            self.draw_glyph(c, start_x + (index * 8), start_y, fill);
        }
    }
}

unsafe impl<F> Send for Framebuffer<F> where F: Format {}
