use core::fmt::Write;
use core::mem::MaybeUninit;
use core::ptr::addr_of_mut;
use core::slice;

include!("megh0816.rs");

static mut CONSOLE: MaybeUninit<Console> = MaybeUninit::zeroed();

static BIT_MASKS: [u8; 8] = [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x01];

#[allow(dead_code)]
pub struct Console {
    base: usize,
    width: usize,
    height: usize,
    stride: usize,
    cursor: (usize, usize),
    cols: usize,
    rows: usize,
}

impl Console {
    const PADDING_X: usize = 4;
    const PADDING_Y: usize = 4;
    const BG_COLOR: u32 = 0x000000;
    const FG_COLOR: u32 = 0xAAAAAA;

    #[inline]
    pub fn shared<'a>() -> &'a mut Self {
        unsafe { (&mut *addr_of_mut!(CONSOLE)).assume_init_mut() }
    }

    #[inline]
    fn get_fb<'a>(&self) -> &mut [u32] {
        unsafe { slice::from_raw_parts_mut(self.base as *mut u32, self.stride * self.height) }
    }

    pub unsafe fn init(base: usize, width: usize, height: usize, stride: usize) {
        let mut shared = Self {
            base,
            width,
            height,
            stride,
            cursor: (0, 0),
            cols: (width - Self::PADDING_X * 2) / FONT_MEGH0816_WIDTH,
            rows: (height - Self::PADDING_Y * 2) / FONT_MEGH0816_HEIGHT,
        };
        shared.fill_rect(0, 0, width, height, 0x000000);

        unsafe {
            (&mut *addr_of_mut!(CONSOLE)).write(shared);
        }
    }

    pub fn put_char(&mut self, c: char) {
        match c {
            '\r' => self.update_cursor(|_, y| (0, y)),
            '\n' => self.update_cursor(|_, y| (0, y + 1)),
            _ => {
                let (mut x, mut y) = self.cursor;
                if x >= self.cols {
                    x = 0;
                    y += 1;
                }
                let y = usize::min(y, self.rows - 1);
                self.draw_char(x, y, c, Self::FG_COLOR, Self::BG_COLOR);
                self.cursor = (x + 1, y);
            }
        }
    }

    #[inline]
    fn update_cursor<F>(&mut self, f: F)
    where
        F: FnOnce(usize, usize) -> (usize, usize),
    {
        self.cursor = f(self.cursor.0, self.cursor.1);
    }

    pub fn draw_char(&mut self, x: usize, y: usize, c: char, fg_color: u32, bg_color: u32) {
        let width = FONT_MEGH0816_WIDTH;
        let height = FONT_MEGH0816_HEIGHT;
        let c = c as u32 as usize;
        let x = x * width + Self::PADDING_X;
        let y = y * height + Self::PADDING_Y;
        if c >= 0x20 && c < 0x80 {
            let stride = ((width + 7) / 8) * height;
            let offset = (c - 0x20) * stride;
            let pattern = &FONT_MEGH0816_DATA[offset..offset + stride];
            self.fill_rect(x, y, width, height, bg_color);
            self.draw_pattern(x, y, width, height, pattern, fg_color);
        } else {
            self.fill_rect(x + 1, y, width - 2, height, fg_color);
        }
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, width: usize, height: usize, color: u32) {
        let stride = self.stride;
        let fb = self.get_fb();
        let mut cursor = x + y * stride;
        if stride == width {
            Self::memset_colors(fb, cursor, width * height, color);
        } else {
            for _ in 0..height {
                Self::memset_colors(fb, cursor, width, color);
                cursor += stride;
            }
        }
    }

    pub fn draw_pattern(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        pattern: &[u8],
        color: u32,
    ) {
        let fb = self.get_fb();
        let w8 = (width + 7) / 8;

        let mut src_cursor = 0;
        let mut cursor = x + y * self.stride;
        let stride = self.stride - 8 * w8 as usize;
        for _ in 0..height {
            for _ in 0..w8 {
                let data = pattern[src_cursor];
                for mask in BIT_MASKS.iter() {
                    if (data & mask) != 0 {
                        fb[cursor] = color;
                    }
                    cursor += 1;
                }
                src_cursor += 1;
            }
            cursor += stride;
        }
    }

    fn memset_colors(fb: &mut [u32], cursor: usize, size: usize, color: u32) {
        let slice = &mut fb[cursor..cursor + size];
        unsafe {
            let color32 = color;
            let mut ptr: *mut u32 = core::mem::transmute(&slice[0]);
            let mut remain = size;

            while (ptr as usize & 0xF) != 0 && remain > 0 {
                ptr.write_volatile(color32);
                ptr = ptr.add(1);
                remain -= 1;
            }

            if remain > 4 {
                let color128 = color32 as u128
                    | (color32 as u128) << 32
                    | (color32 as u128) << 64
                    | (color32 as u128) << 96;
                let count = remain / 4;
                let mut ptr2 = ptr as *mut u128;

                for _ in 0..count {
                    ptr2.write_volatile(color128);
                    ptr2 = ptr2.add(1);
                }

                ptr = ptr2 as *mut u32;
                remain -= count * 4;
            }

            for _ in 0..remain {
                ptr.write_volatile(color32);
                ptr = ptr.add(1);
            }
        }
    }
}

impl Write for Console {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.chars() {
            self.put_char(c);
        }
        Ok(())
    }
}
