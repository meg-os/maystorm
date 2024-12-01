/*
Ported From: https://github.com/uchan-nos/mikanos/blob/master/apps/cube/cube.cpp
From?: https://essen.osask.jp/?aclib12

Copyright 2018-2022 Kota Uchida

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

#![no_main]
#![no_std]

extern crate libm;
use core::cell::UnsafeCell;
use core::f64::consts::PI;
use core::ptr::addr_of_mut;
use libm::{ceil, cos, floor, sin};
use megstd::drawing::vec::*;
use megstd::window::*;

#[no_mangle]
fn _start() {
    App::new().run();
}

const SCALE: i32 = 50;
const SCALE_F: f64 = SCALE as f64;
const MARGIN: i32 = 10;
const CANVAS_SIZE: i32 = 3 * SCALE + MARGIN;
const CANVAS_SIZE_U: u32 = CANVAS_SIZE as u32;

const N_VERTICES: usize = 8;
const N_SURFACES: usize = 6;
const VERTICES: [Vec3<f64>; N_VERTICES] = [
    Vec3::new(1.0, 1.0, 1.0),
    Vec3::new(1.0, 1.0, -1.0),
    Vec3::new(1.0, -1.0, 1.0),
    Vec3::new(1.0, -1.0, -1.0),
    Vec3::new(-1.0, 1.0, 1.0),
    Vec3::new(-1.0, 1.0, -1.0),
    Vec3::new(-1.0, -1.0, 1.0),
    Vec3::new(-1.0, -1.0, -1.0),
];
const SURFACES: [[usize; 4]; N_SURFACES] = [
    [0, 4, 6, 2],
    [1, 3, 7, 5],
    [0, 2, 3, 1],
    [0, 1, 5, 4],
    [4, 5, 7, 6],
    [6, 7, 3, 2],
];
const COLORS: [u32; N_SURFACES] = [0xff0000, 0x00ff00, 0xffff00, 0x0000ff, 0xff00ff, 0x00ffff];

const BITMAP_SIZE: usize = CANVAS_SIZE as usize * CANVAS_SIZE as usize;
static mut DATA: UnsafeCell<[u32; BITMAP_SIZE]> = UnsafeCell::new([0; BITMAP_SIZE]);

struct App<'a> {
    window: Window,
    bitmap: BitmapRefMut32<'a>,
    thx: isize,
    thy: isize,
    thz: isize,
    scr: [Vec2<i32>; N_VERTICES],
}

impl<'a> App<'a> {
    #[inline]
    fn new() -> Self {
        let window = WindowBuilder::new()
            .size(Size::new(CANVAS_SIZE_U, CANVAS_SIZE_U))
            .bg_color(WindowColor::BLACK)
            .opaque()
            .bitmap_argb32()
            .max_fps(20)
            .build("cube");
        let bitmap = BitmapRefMut32::from_bytes(
            unsafe { (&mut *addr_of_mut!(DATA)).get_mut() },
            Size::new(CANVAS_SIZE_U, CANVAS_SIZE_U),
        );
        Self {
            window,
            bitmap,
            thx: 0,
            thy: 0,
            thz: 0,
            scr: [Vec2::new(0, 0); N_VERTICES],
        }
    }
}

impl App<'_> {
    fn run(&mut self) {
        while self.window.read_char().is_none() {
            self.update();
            self.window
                .draw(|ctx| ctx.blt32(&self.bitmap, Point::default()));
        }
    }

    fn update(&mut self) {
        // 立方体を X, Y, Z 軸回りに回転
        self.thx = (self.thx + 182) & 0xffff;
        self.thy = (self.thy + 273) & 0xffff;
        self.thz = (self.thz + 364) & 0xffff;

        let to_rad = PI / 32768.0;
        let xp = cos(self.thx as f64 * to_rad);
        let xa = sin(self.thx as f64 * to_rad);
        let yp = cos(self.thy as f64 * to_rad);
        let ya = sin(self.thy as f64 * to_rad);
        let zp = cos(self.thz as f64 * to_rad);
        let za = sin(self.thz as f64 * to_rad);

        let mut vert = [Vec3::new(0.0, 0.0, 0.0); N_VERTICES];
        for (cv, vert) in VERTICES.iter().zip(vert.iter_mut()) {
            let zt = SCALE_F * cv.z * xp + SCALE_F * cv.y * xa;
            let yt = SCALE_F * cv.y * xp - SCALE_F * cv.z * xa;
            let xt = SCALE_F * cv.x * yp + zt * ya;
            vert.z = zt * yp - SCALE_F * cv.x * ya;
            vert.x = xt * zp - yt * za;
            vert.y = yt * zp + xt * za;
        }

        // オブジェクト座標 vert を スクリーン座標 scr に変換（画面奥が Z+）
        for (vert, scr) in vert.iter().zip(self.scr.iter_mut()) {
            let t = 6.0 * SCALE_F / (vert.z + 8.0 * SCALE_F);
            scr.x = (vert.x * t) as i32 + CANVAS_SIZE / 2;
            scr.y = (vert.y * t) as i32 + CANVAS_SIZE / 2;
        }

        // 面中心の Z 座標（を 4 倍した値）を 6 面について計算
        let mut centerz4 = [(0usize, 0.0f64); N_SURFACES];
        for (i, v) in centerz4.iter_mut().enumerate() {
            v.0 = i;
        }
        for (v, surface) in centerz4.iter_mut().zip(SURFACES.iter()) {
            v.1 = surface.iter().fold(0.0, |a, v| a + vert[*v].z);
        }

        // 奥にある（= Z 座標が大きい）オブジェクトから順に描画
        centerz4.sort_by(|a, b| {
            if a.1 > b.1 {
                core::cmp::Ordering::Less
            } else {
                core::cmp::Ordering::Greater
            }
        });

        self.bitmap
            .fill_rect(self.bitmap.bounds(), TrueColor::PRIMARY_BLACK);

        // 法線ベクトルがこっちを向いてる面だけ描画
        for (surface_index, _) in centerz4 {
            let surface = SURFACES[surface_index];
            let v0 = vert[surface[0]];
            let v1 = vert[surface[1]];
            let v2 = vert[surface[2]];
            let e0x = v1.x - v0.x;
            let e0y = v1.y - v0.y; // v0 --> v1
            let e1x = v2.x - v1.x;
            let e1y = v2.y - v1.y; // v1 --> v2
            if e0x * e1y <= e0y * e1x {
                self.draw_surface(surface, COLORS[surface_index]);
            }
        }
    }

    fn draw_surface(&mut self, surface: [usize; 4], color: u32) {
        // 描画する面
        // 画面の描画範囲 [ymin, ymax]
        let mut ymin = CANVAS_SIZE;
        let mut ymax = 0;
        // Y, X 座標の組
        let mut y2x_up = [0; CANVAS_SIZE as usize];
        let mut y2x_down = [0; CANVAS_SIZE as usize];

        for (i, p1) in surface.iter().enumerate() {
            let p0 = self.scr[surface[(i + 3) % 4]];
            let p1 = self.scr[*p1];
            ymin = ymin.min(p1.y);
            ymax = ymax.max(p1.y);
            if p0.y == p1.y {
                continue;
            }

            let (y2x, x0, y0, y1, dx) = if p0.y < p1.y {
                // p0 --> p1 は上る方向
                (&mut y2x_up, p0.x, p0.y, p1.y, p1.x - p0.x)
            } else {
                // p0 --> p1 は下る方向
                (&mut y2x_down, p1.x, p1.y, p0.y, p0.x - p1.x)
            };

            let dx = dx as f64;
            let y0 = y0 as f64;
            let y1 = y1 as f64;
            let x0 = x0 as f64;
            let m = dx / (y1 - y0);
            if dx >= 0.0 {
                for y in (y0 as usize)..=(y1 as usize) {
                    y2x[y as usize] = floor(m * (y as f64 - y0) + x0) as i32;
                }
            } else {
                for y in (y0 as usize)..=(y1 as usize) {
                    y2x[y as usize] = ceil(m * (y as f64 - y0) + x0) as i32;
                }
            }
        }

        for y in ymin..=ymax {
            let p0x = y2x_up[y as usize].min(y2x_down[y as usize]);
            let p1x = y2x_up[y as usize].max(y2x_down[y as usize]);
            self.bitmap.draw_hline(
                Point::new(p0x, y),
                (p1x - p0x + 1).max(0) as u32,
                TrueColor::from_rgb(color),
            );
        }
    }
}
