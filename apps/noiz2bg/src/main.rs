/*
Ported
Original: UNKNOWN

License
-------
Copyright 2002 Kenta Cho. All rights reserved.
Redistribution and use in source and binary forms,
with or without modification, are permitted provided that
the following conditions are met:
 1. Redistributions of source code must retain the above copyright notice,
    this list of conditions and the following disclaimer.
 2. Redistributions in binary form must reproduce the above copyright notice,
    this list of conditions and the following disclaimer in the documentation
    and/or other materials provided with the distribution.
THIS SOFTWARE IS PROVIDED ``AS IS'' AND ANY EXPRESS OR IMPLIED WARRANTIES,
INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND
FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL
THE REGENTS OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY,
WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR
OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF
ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
*/
/* "test044a.c" : 32bit-gbox, fast */
/*  stack:4k malloc:304k   */
/*  動けばいいんだ、動けば。 */

#![no_main]
#![no_std]

use core::cell::UnsafeCell;
use core::ptr::addr_of_mut;
use megstd::drawing::BitmapRefMut32;
use megstd::sys::syscall::*;
use megstd::window::*;

#[no_mangle]
fn _start() {
    App::new().run();
}

struct App<'a> {
    window: Window,
    bitmap: BitmapRefMut32<'a>,
    board_index: usize,
    boards: [Option<Board>; App::BOARD_MAX],
    scene: Scene,
    scene_count: usize,
    board_mx: i32,
    board_my: i32,
    board_repx: i32,
    board_repy: i32,
    board_rep_xn: i32,
    board_rep_yn: i32,
}

const BITMAP_WIDTH: u32 = 256;
const BITMAP_HEIGHT: u32 = 256;
const BITMAP_SIZE: usize = BITMAP_WIDTH as usize * BITMAP_HEIGHT as usize;
static mut DATA: UnsafeCell<[u32; BITMAP_SIZE]> = UnsafeCell::new([0; BITMAP_SIZE]);

impl<'a> App<'a> {
    #[inline]
    fn new() -> Self {
        let window = WindowBuilder::new()
            .size(Size::new(BITMAP_WIDTH, BITMAP_HEIGHT))
            .opaque()
            .bitmap_argb32()
            .max_fps(50)
            .build("noiz2bg");
        let bitmap = BitmapRefMut32::from_bytes(
            unsafe { (&mut *addr_of_mut!(DATA)).get_mut() },
            Size::new(BITMAP_WIDTH, BITMAP_HEIGHT),
        );
        Self {
            window,
            bitmap,
            board_index: 0,
            boards: [None; Self::BOARD_MAX],
            scene: Scene::default(),
            scene_count: Self::FPS * 10,
            board_mx: 0,
            board_my: 0,
            board_repx: 0,
            board_repy: 0,
            board_rep_xn: 0,
            board_rep_yn: 0,
        }
    }
}

impl App<'_> {
    const BOARD_MAX: usize = 256;
    const FPS: usize = 30;

    #[inline]
    fn run(&mut self) {
        self.set_stage();
        while self.window.read_char().is_none() {
            self.move_bg();
            self.draw_bg();
            self.window
                .draw(|ctx| ctx.blt32(&self.bitmap, Point::default()));
            if self.scene_count > 1 {
                self.scene_count -= 1;
            } else {
                self.scene.next();
                self.scene_count = Self::FPS * 20;
                self.set_stage();
            }
        }
    }

    #[inline]
    fn move_bg(&mut self) {
        for board in self.boards.iter_mut() {
            let Some(board) = board else { break };
            board.x = (board.x + self.board_mx) & (self.board_repx - 1);
            board.y = (board.y + self.board_my) & (self.board_repy - 1);
        }
    }

    #[inline]
    fn draw_bg(&mut self) {
        self.bitmap
            .fill_rect(self.bitmap.bounds(), TrueColor::PRIMARY_WHITE);

        let osx = (0 - self.board_repx) * (self.board_rep_xn / 2);
        let osy = (0 - self.board_repy) * (self.board_rep_yn / 2);
        for board in &self.boards {
            let Some(board) = board else { break };
            let mut ox = osx;
            for _ in 0..self.board_rep_xn {
                let mut oy = osy;
                for _ in 0..self.board_rep_yn {
                    let x =
                        (board.x + ox).checked_div(board.z).unwrap_or(0) + BITMAP_WIDTH as i32 / 2;
                    let y =
                        (board.y + oy).checked_div(board.z).unwrap_or(0) + BITMAP_HEIGHT as i32 / 2;
                    let width = board.width;
                    let height = board.height;
                    if false {
                        self.bitmap
                            .blend_rect(Rect::new(x, y, width, height), board.color);
                    } else {
                        os_blend_rect(
                            &self.bitmap as *const BitmapRefMut32 as usize,
                            x,
                            y,
                            width,
                            height,
                            board.color.argb(),
                        );
                    }
                    oy += self.board_repy;
                }
                ox += self.board_repx;
            }
        }
    }

    fn set_stage(&mut self) {
        self.board_index = 0;
        self.boards.iter_mut().for_each(|p| *p = None);

        match self.scene {
            Scene::Scene0 => {
                self.add_board(9000, 9000, 500, 25000, 25000);
                for i in 0..4 {
                    for j in 0..4 {
                        if i > 1 || j > 1 {
                            self.add_board(
                                i * 16384,
                                j * 16384,
                                500,
                                10000 + (i as u32 * 12345) % 3000,
                                10000 + (j as u32 * 54321) % 3000,
                            );
                        }
                    }
                }
                for i in 0..8 {
                    for j in 0..4 {
                        self.add_board(
                            0,
                            i * 16384,
                            500 - j * 50,
                            20000 - j as u32 * 1000,
                            12000 - j as u32 * 500,
                        );
                    }
                }
                for i in 0..8 {
                    self.add_board(0, i * 8192, 100, 20000, 6400);
                }
                self.board_mx = 40;
                self.board_my = 300;
                self.board_repx = 65536;
                self.board_repy = 65536;
                self.board_rep_xn = 4;
                self.board_rep_yn = 4;
            }
            Scene::Scene1 => {
                self.add_board(12000, 12000, 400, 48000, 48000);
                self.add_board(12000, 44000, 400, 48000, 8000);
                self.add_board(44000, 12000, 400, 8000, 48000);
                for i in 0..16 {
                    self.add_board(0, 0, 400 - i * 10, 16000, 16000);
                    if i < 6 {
                        self.add_board(9600, 16000, 400 - i * 10, 40000, 16000);
                    }
                }
                self.board_mx = 128;
                self.board_my = 512;
                self.board_repx = 65536;
                self.board_repy = 65536;
                self.board_rep_xn = 4;
                self.board_rep_yn = 4;
            }
            Scene::Scene2 => {
                for i in 0..16 {
                    self.add_board(7000 + i * 3000, 0, 1600 - i * 100, 24000, 5000);
                    self.add_board(7000 + i * 3000, 50000, 1600 - i * 100, 4000, 10000);
                    self.add_board(-7000 - i * 3000, 0, 1600 - i * 100, 24000, 5000);
                    self.add_board(-7000 - i * 3000, 50000, 1600 - i * 100, 4000, 10000);
                }
                self.board_mx = 0;
                self.board_my = 1200;
                self.board_repx = 0;
                self.board_repy = 65536;
                self.board_rep_xn = 1;
                self.board_rep_yn = 10;
            }
            Scene::Scene3 => {
                self.add_board(9000, 9000, 500, 30000, 30000);
                for i in 0..4 {
                    for j in 0..4 {
                        if i > 1 || j > 1 {
                            self.add_board(
                                i * 16384,
                                j * 16384,
                                500,
                                12000 + (i as u32 * 12345) % 3000,
                                12000 + (j as u32 * 54321) % 3000,
                            );
                        }
                    }
                }
                for i in 0..4 {
                    for j in 0..4 {
                        if (i > 1 || j > 1) && (i + j) % 3 == 0 {
                            self.add_board(
                                i * 16384,
                                j * 16384,
                                480,
                                9000 + (i as u32 * 12345) % 3000,
                                9000 + (j as u32 * 54321) % 3000,
                            );
                        }
                    }
                }
                self.add_board(9000, 9000, 480, 20000, 20000);
                self.add_board(9000, 9000, 450, 20000, 20000);
                self.add_board(32768, 40000, 420, 65536, 5000);
                self.add_board(30000, 32768, 370, 4800, 65536);
                self.add_board(32768, 0, 8, 65536, 10000);
                self.board_mx = 10;
                self.board_my = 100;
                self.board_repx = 65536;
                self.board_repy = 65536;
                self.board_rep_xn = 4;
                self.board_rep_yn = 4;
            }
            Scene::Scene4 => {
                self.add_board(32000, 12000, 160, 48000, 48000);
                self.add_board(32000, 44000, 160, 48000, 8000);
                self.add_board(64000, 12000, 160, 8000, 48000);
                for i in 0..16 {
                    self.add_board(20000, 0, 160 - i * 10, 16000, 16000);
                    if i < 6 {
                        self.add_board(29600, 16000, 160 - i * 10, 40000, 16000);
                    }
                }
                self.board_mx = 0;
                self.board_my = 128;
                self.board_repx = 65536;
                self.board_repy = 65536;
                self.board_rep_xn = 2;
                self.board_rep_yn = 2;
            }
            Scene::Scene5 => {
                for k in 0..5 {
                    let mut j = 0;
                    for i in 0..16 {
                        self.add_board(j, i * 4096, 200 - k * 10, 16000, 4096);
                        self.add_board(j + 16000 - j * 2, i * 4096, 200 - k * 10, 16000, 4096);
                        if i < 4 {
                            j += 2000;
                        } else if i < 6 {
                            j -= 3500;
                        } else if i < 12 {
                            j += 1500;
                        } else {
                            j -= 2000;
                        }
                    }
                }
                self.board_mx = -64;
                self.board_my = 256;
                self.board_repx = 65536;
                self.board_repy = 65536;
                self.board_rep_xn = 2;
                self.board_rep_yn = 2;
            }
        }
    }

    fn add_board(&mut self, x: i32, y: i32, z: i32, width: u32, height: u32) {
        if self.board_index >= Self::BOARD_MAX {
            return;
        }

        self.boards.get_mut(self.board_index).map(|v| {
            *v = Some(Board::new(
                x,
                y,
                z,
                width.checked_div(z as u32).unwrap_or(0),
                height.checked_div(z as u32).unwrap_or(0),
                ColorComponents {
                    a: Alpha8::new(0x30),
                    r: (0x9900i32).checked_div(1 + z).unwrap_or(0) as u8,
                    g: (0xAA00i32).checked_div(1 + z).unwrap_or(0) as u8,
                    b: 0xDD,
                }
                .into(),
            ))
        });
        self.board_index += 1;
    }
}

#[derive(Debug, Clone, Copy)]
struct Board {
    x: i32,
    y: i32,
    z: i32,
    width: u32,
    height: u32,
    color: TrueColor,
}

impl Board {
    #[inline]
    const fn new(x: i32, y: i32, z: i32, width: u32, height: u32, color: TrueColor) -> Self {
        Self {
            x,
            y,
            z,
            width,
            height,
            color,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Scene {
    Scene0,
    Scene1,
    Scene2,
    Scene3,
    Scene4,
    Scene5,
}

impl Scene {
    #[inline]
    fn next(&mut self) {
        use Scene::*;
        match self {
            Scene0 => *self = Scene1,
            Scene1 => *self = Scene2,
            Scene2 => *self = Scene3,
            Scene3 => *self = Scene4,
            Scene4 => *self = Scene5,
            Scene5 => *self = Scene0,
        }
    }
}

impl Default for Scene {
    #[inline]
    fn default() -> Self {
        Self::Scene1
    }
}
