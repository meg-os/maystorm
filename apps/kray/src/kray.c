#include <acl.c>

#define EPS		1.0e-4

typedef struct Vec_ {
	double x, y, z;
} Vec;

Vec Vec_new(double x, double y, double z)
{
	Vec v = { x, y, z };
	return v;
}

double Util_clamp(double t, double min, double max)
{
	if (t < min) t = min;
	if (t > max) t = max;
	return t;
}

Vec Vec_add(Vec a, Vec b)    { return Vec_new(a.x + b.x, a.y + b.y, a.z + b.z); }
Vec Vec_sub(Vec a, Vec b)    { return Vec_new(a.x - b.x, a.y - b.y, a.z - b.z); }
Vec Vec_mul(double t, Vec v) { return Vec_new(t * v.x,   t * v.y,   t * v.z);   }
Vec Vec_multi(Vec a, Vec b)  { return Vec_new(a.x * b.x, a.y * b.y, a.z * b.z); }
double Vec_dot(Vec a, Vec b) { return a.x * b.x + a.y * b.y + a.z * b.z;        }
double Vec_length(Vec v)     { return sqrt(Vec_dot(v, v));                      }
Vec Vec_reflect(Vec v, Vec normal) { return Vec_add(v, Vec_mul(-2.0 * Vec_dot(normal, v), normal)); }
AInt32 Util_color(double t) { return (AInt32) (255.99999 * Util_clamp(t, 0.0, 1.0)); }
AInt32 Util_rgb(Vec v) { return aRgb8(Util_color(v.x), Util_color(v.y), Util_color(v.z)); }

Vec Vec_normalize(Vec v)
{
	double len = Vec_length(v);
	if (len > 1.0e-17)
		v = Vec_mul(1.0 / len, v);
	return v;
}

typedef struct Isect_ {
	Vec hit_point, normal, color;
	double distance;
} Isect;

double Util_mod2(double t)
{
	t -= (AInt32) (t * (1.0 / 2.0)) * 2.0;
	if (t < 0.0)
		t += 2.0;
	return t;
}

typedef struct Object_ {
	Vec pos, col, nor;
	double rad; // radius
} Object;	// Sphere(pos, col, rad) or Plane(pos, col, nor).

void Sphere_intersect(Object s, Vec ray_origin, Vec ray_dir, Vec light, Isect *i)
{
	Vec rs = Vec_sub(ray_origin, s.pos);
	double b = Vec_dot(rs, ray_dir);
	double c = Vec_dot(rs, rs) - s.rad * s.rad;
	double d = b * b - c;
	if (d < 0.0) return;
	double t = - b - sqrt(d);
	if (t < EPS || t > i->distance) return;
	i->hit_point = Vec_add(ray_origin, Vec_mul(t, ray_dir));
	i->normal = Vec_normalize(Vec_sub(i->hit_point, s.pos));
	i->color = Vec_mul(Util_clamp(Vec_dot(light, i->normal), 0.1, 1.0), s.col);
	i->distance = t;
}

void Plane_intersect(Object p, Vec ray_origin, Vec ray_dir, Vec light, Isect *i)
{
	double d = - Vec_dot(p.pos, p.nor);
	double v = Vec_dot(ray_dir, p.nor);
	if (v * v < 1.0e-30) return;
	double t = - (Vec_dot(ray_origin, p.nor) + d) / v;
	if (t < EPS || t > i->distance) return;
	i->hit_point = Vec_add(ray_origin, Vec_mul(t, ray_dir));
	i->normal = p.nor;
	double d2 = Util_clamp(Vec_dot(light, i->normal), 0.1, 1.0);
	if ((Util_mod2(i->hit_point.x) - 1.0) * (Util_mod2(i->hit_point.z) - 1.0) > 0.0)
		d2 *= 0.5;
	i->color = Vec_mul(d2 * (1.0 - Util_clamp(fabs(i->hit_point.z) * 0.04, 0.0, 1.0)), p.col);
	i->distance = t;
}

typedef struct Util_ {
	Vec light;
	Object s1, s2, s3, p;
} Util;

void Util_intersect(Util u, Vec ray_origin, Vec ray_dir, Isect *i)
{
	i->distance = 1.0e+30;
	Sphere_intersect(u.s1, ray_origin, ray_dir, u.light, i);
	Sphere_intersect(u.s2, ray_origin, ray_dir, u.light, i);
	Sphere_intersect(u.s3, ray_origin, ray_dir, u.light, i);
	Plane_intersect (u.p,  ray_origin, ray_dir, u.light, i);
}

void aMain()
{
	Util u;
	Isect i;
	u.light = Vec_new(0.577, 0.577, 0.577);
	u.s1.rad = 0.5; u.s1.pos = Vec_new( 0.0, -0.5, 0.0);       u.s1.col = Vec_new(1.0, 0.0, 0.0);
	u.s2.rad = 1.0; u.s2.pos = Vec_new( 2.0,  0.0, cos(6.66)); u.s2.col = Vec_new(0.0, 1.0, 0.0);
	u.s3.rad = 1.5; u.s3.pos = Vec_new(-2.0,  0.5, cos(3.33)); u.s3.col = Vec_new(0.0, 0.0, 1.0);
	u.p.pos = Vec_new(0.0, -1.0, 0.0); u.p.nor = Vec_new(0.0, 1.0, 0.0); u.p.col = Vec_new(1.0, 1.0, 1.0);
	AWindow *win = aOpenWin(512, 384, "kray", 1);
	AInt16 ix, iy, j;
	for (iy = 0; iy < 384; iy++) {
		for (ix = 0; ix < 512; ix++) {
			double x = ix * (1.0 / 256.0) - 1.0;
			double y = (384 - iy) * (1.0 / 256.0) - 1.0;
			Vec ray_dir = Vec_normalize(Vec_new(x, y, -1.0));
			Util_intersect(u, Vec_new(0.0, 2.0, 6.0), ray_dir, &i);
			Vec dest_col = Vec_mul(ray_dir.y, Vec_new(1.0, 1.0, 1.0));
			if (i.distance < 1.0e+30) {
				Vec temp_col = dest_col = i.color;
				for (j = 1; j < 4; j++) {
					ray_dir = Vec_reflect(ray_dir, i.normal);
					Util_intersect(u, i.hit_point, ray_dir, &i);
					if (i.distance >= 1.0e+30) break;
					temp_col = Vec_multi(temp_col, i.color);
					dest_col = Vec_add(dest_col, temp_col);
				}
			}
			aSetPix(win, ix, iy, Util_rgb(dest_col));
		}
		aLeapFlushAll(win, 100);
	}
	aWait(-1);
}
