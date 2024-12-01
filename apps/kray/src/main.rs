/*
Ported From: https://essen.osask.jp/?aclib20
Ported From: https://gist.github.com/yhara/ea0e66e0d8bdd114d2401dd133539fa3
Ported From: https://qiita.com/doxas/items/477fda867da467116f8d

*/

#![no_main]
#![no_std]

extern crate libm;
use core::cell::UnsafeCell;
use core::ptr::addr_of_mut;
use libm::{cos, fabs, floor, sqrt};
use megstd::drawing::vec::*;
use megstd::window::*;

const EPS: f64 = 1.0e-4;
const BITMAP_WIDTH: u32 = 512;
const BITMAP_HEIGHT: u32 = 384;
const BITMAP_SIZE: usize = BITMAP_WIDTH as usize * BITMAP_HEIGHT as usize;
static mut DATA: UnsafeCell<[u32; BITMAP_SIZE]> = UnsafeCell::new([0; BITMAP_SIZE]);

#[no_mangle]
fn _start() {
    let window = WindowBuilder::new()
        .size(Size::new(BITMAP_WIDTH, BITMAP_HEIGHT))
        .opaque()
        .bitmap_argb32()
        .build("ray");
    let mut bitmap = BitmapRefMut32::from_bytes(
        unsafe { (&mut *addr_of_mut!(DATA)).get_mut() },
        Size::new(BITMAP_WIDTH, BITMAP_HEIGHT),
    );

    let light = Vec3::new(0.577, 0.577, 0.577);
    let s1 = Sphere::new(Vec3::new(0.0, -0.5, 0.0), 0.5, Vec3::new(1.0, 0.0, 0.0));
    let s2 = Sphere::new(
        Vec3::new(2.0, 0.0, cos(6.66)),
        1.0,
        Vec3::new(1.0, 1.0, 0.0),
    );
    let s3 = Sphere::new(
        Vec3::new(-2.0, 0.5, cos(3.33)),
        1.5,
        Vec3::new(1.0, 0.0, 1.0),
    );
    let p1 = Plane::new(
        Vec3::new(0.0, -1.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(1.0, 1.0, 1.0),
    );
    let items = [
        &s1 as &dyn Intersect,
        &s2 as &dyn Intersect,
        &s3 as &dyn Intersect,
        &p1 as &dyn Intersect,
    ];
    let objects = Objects {
        light,
        objects: &items,
    };

    let chunk_size = 32;
    for y in (0..BITMAP_HEIGHT as i32).step_by(chunk_size as usize) {
        for x in (0..BITMAP_WIDTH as i32).step_by(chunk_size as usize) {
            render(&mut bitmap, &objects, x, y, chunk_size);
        }
    }
    window.draw(|ctx| ctx.blt32(&bitmap, Point::zero()));

    for yb in (0..BITMAP_HEIGHT as i32).step_by(chunk_size as usize) {
        for xb in (0..BITMAP_WIDTH as i32).step_by(chunk_size as usize) {
            for y0 in 0..chunk_size as i32 {
                for x0 in 0..chunk_size as i32 {
                    render(&mut bitmap, &objects, xb + x0, yb + y0, 1);
                }
            }
            window.draw(|ctx| ctx.blt32_sub(&bitmap, Rect::new(xb, yb, chunk_size, chunk_size)))
        }
    }

    window.wait_char();
}

#[inline]
fn render(bitmap: &mut BitmapRefMut32, objects: &Objects, x: i32, y: i32, size: u32) {
    let dest_col = {
        let mut isect = Isect::default();
        let x = x as f64 * (1.0 / 256.0) - 1.0;
        let y = (bitmap.height() as f64 - y as f64) * (1.0 / 256.0) - 1.0;
        let ray_dir = vec_normalize(Vec3::new(x, y, -1.0));
        objects.intersect(&Vec3::new(0.0, 2.0, 6.0), &ray_dir, &mut isect);
        if isect.distance < 1.0e+30 {
            let mut dest_col = isect.color;
            let mut temp_col = isect.color;
            for _ in 0..4 {
                let ray_dir = vec_refrect(ray_dir, isect.normal);
                objects.intersect(&isect.hit_point.clone(), &ray_dir, &mut isect);
                if isect.distance >= 1.0e+30 {
                    break;
                }
                temp_col = temp_col * isect.color;
                dest_col += temp_col;
            }
            dest_col
        } else {
            Vec3::new(1.0, 1.0, 1.0) * ray_dir.y
        }
    };
    if size > 1 {
        bitmap.fill_rect(Rect::new(x, y, size, size), dest_col.into());
    } else {
        bitmap.set_pixel(Point::new(x, y), dest_col.into());
    }
}

#[inline]
fn vec_length(vec: Vec3<f64>) -> f64 {
    sqrt(vec.dot(&vec))
}

#[inline]
fn vec_normalize(vec: Vec3<f64>) -> Vec3<f64> {
    let len = vec_length(vec);
    if len > 1.0e-17 {
        vec * (1.0 / len)
    } else {
        vec
    }
}

#[inline]
fn vec_refrect(vec: Vec3<f64>, normal: Vec3<f64>) -> Vec3<f64> {
    vec + (normal * (-2.0 * normal.dot(&vec)))
}

#[inline]
fn mod2(t: f64) -> f64 {
    let t = t - floor(t * (1.0 / 2.0)) * 2.0;
    if t < 0.0 {
        t + 2.0
    } else {
        t
    }
}

#[derive(Default)]
struct Isect {
    hit_point: Vec3<f64>,
    normal: Vec3<f64>,
    color: Vec3<f64>,
    distance: f64,
}

struct Objects<'a> {
    light: Vec3<f64>,
    objects: &'a [&'a dyn Intersect],
}

impl Objects<'_> {
    #[inline]
    fn intersect(&self, ray_origin: &Vec3<f64>, ray_dir: &Vec3<f64>, isect: &mut Isect) {
        isect.distance = 1.0e+30;
        for object in self.objects {
            object.intersect(ray_origin, ray_dir, &self.light, isect);
        }
    }
}

trait Intersect {
    fn intersect(
        &self,
        ray_origin: &Vec3<f64>,
        ray_dir: &Vec3<f64>,
        light: &Vec3<f64>,
        isect: &mut Isect,
    );
}

struct Sphere {
    position: Vec3<f64>,
    radius: f64,
    color: Vec3<f64>,
}

impl Sphere {
    #[inline]
    fn new(position: Vec3<f64>, radius: f64, color: Vec3<f64>) -> Self {
        Self {
            position,
            radius,
            color,
        }
    }
}

impl Intersect for Sphere {
    fn intersect(
        &self,
        ray_origin: &Vec3<f64>,
        ray_dir: &Vec3<f64>,
        light: &Vec3<f64>,
        isect: &mut Isect,
    ) {
        let rs = *ray_origin - self.position;
        let b = rs.dot(ray_dir);
        let c = rs.dot(&rs) - self.radius * self.radius;
        let d = b * b - c;
        if d < 0.0 {
            return;
        }
        let t = -b - sqrt(d);
        if t < EPS || t > isect.distance {
            return;
        }
        isect.hit_point = *ray_origin + (*ray_dir * t);
        isect.normal = vec_normalize(isect.hit_point - self.position);
        isect.color = self.color * light.dot(&isect.normal).clamp(0.1, 1.0);
        isect.distance = t;
    }
}

struct Plane {
    position: Vec3<f64>,
    normal: Vec3<f64>,
    color: Vec3<f64>,
}

impl Plane {
    #[inline]
    fn new(position: Vec3<f64>, normal: Vec3<f64>, color: Vec3<f64>) -> Self {
        Self {
            position,
            normal,
            color,
        }
    }
}

impl Intersect for Plane {
    fn intersect(
        &self,
        ray_origin: &Vec3<f64>,
        ray_dir: &Vec3<f64>,
        light: &Vec3<f64>,
        isect: &mut Isect,
    ) {
        let d = -self.position.dot(&self.normal);
        let v = ray_dir.dot(&self.normal);
        if v * v < 1.0e-30 {
            return;
        }
        let t = -(ray_origin.dot(&self.normal) + d) / v;
        if t < EPS || t > isect.distance {
            return;
        }
        isect.hit_point = *ray_origin + (*ray_dir * t);
        isect.normal = self.normal;
        let d2 = light.dot(&isect.normal).clamp(0.1, 1.0);
        let d3 = if (mod2(isect.hit_point.x) - 1.0) * (mod2(isect.hit_point.z) - 1.0) > 0.0 {
            d2 * 0.5
        } else {
            d2
        };
        isect.color = self.color * (d3 * (1.0 - (fabs(isect.hit_point.z) * 0.04).clamp(0.0, 1.0)));
        isect.distance = t;
    }
}
