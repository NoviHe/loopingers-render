//! Shared drawing primitives for the effect generators.

use std::f32::consts::TAU;

pub const WIDTH: usize = 1920;
pub const HEIGHT: usize = 1080;
pub const FPS: u32 = 30;
pub const SECONDS: u32 = 8;
pub const FRAMES: u32 = FPS * SECONDS;
/// Particles live slightly outside the frame so they never pop in or out.
pub const MARGIN: f32 = 120.0;
pub const SPAN_Y: f32 = HEIGHT as f32 + 2.0 * MARGIN;
pub const SPAN_X: f32 = WIDTH as f32 + 2.0 * MARGIN;

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.unit()
    }

    pub fn whole(&mut self, low: u32, high: u32) -> f32 {
        (low + (self.unit() * (high - low + 1) as f32) as u32).min(high) as f32
    }

    pub fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[((self.unit() * items.len() as f32) as usize).min(items.len() - 1)]
    }
}

pub type Color = [f32; 3];

pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn mix(a: Color, b: Color, t: f32) -> Color {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Position inside `[0, span)` after `cycles` full trips per loop (negative cycles go backwards).
pub fn travel(start: f32, cycles: f32, phase: f32, span: f32) -> f32 {
    (start + cycles * phase * span).rem_euclid(span)
}

/// Value in `0..=1` that oscillates a whole number of times per loop.
pub fn pulse(cycles: f32, phase: f32, offset: f32) -> f32 {
    0.5 + 0.5 * (TAU * (cycles * phase + offset)).sin()
}

pub struct Canvas {
    pixels: Vec<f32>,
}

impl Canvas {
    pub fn new() -> Self {
        Self {
            pixels: vec![0.0; WIDTH * HEIGHT * 3],
        }
    }

    pub fn clear(&mut self) {
        self.pixels.fill(0.0);
    }

    pub fn add(&mut self, x: usize, y: usize, color: Color, amount: f32) {
        let index = (y * WIDTH + x) * 3;
        for (channel, value) in color.iter().enumerate() {
            self.pixels[index + channel] += value * amount;
        }
    }

    /// Paints `shade(dx, dy)` for every pixel within `reach` of the centre.
    pub fn paint(&mut self, cx: f32, cy: f32, reach: f32, shade: impl Fn(f32, f32) -> Color) {
        let x0 = ((cx - reach).floor() as i32).max(0);
        let x1 = ((cx + reach).ceil() as i32).min(WIDTH as i32 - 1);
        let y0 = ((cy - reach).floor() as i32).max(0);
        let y1 = ((cy + reach).ceil() as i32).min(HEIGHT as i32 - 1);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let color = shade(x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                self.add(x as usize, y as usize, color, 1.0);
            }
        }
    }

    pub fn glow(&mut self, cx: f32, cy: f32, sigma: f32, color: Color, intensity: f32) {
        let inverse = 1.0 / (2.0 * sigma * sigma);
        self.paint(cx, cy, sigma * 3.4, |dx, dy| {
            let weight = (-(dx * dx + dy * dy) * inverse).exp() * intensity;
            [color[0] * weight, color[1] * weight, color[2] * weight]
        });
    }

    /// A lens-blur disc with a brighter rim, like a bokeh highlight.
    pub fn disc(
        &mut self,
        cx: f32,
        cy: f32,
        radius: f32,
        softness: f32,
        color: Color,
        intensity: f32,
    ) {
        self.paint(cx, cy, radius + softness * 2.0 + 2.0, |dx, dy| {
            let distance = (dx * dx + dy * dy).sqrt();
            let coverage = smoothstep(radius + softness, radius - softness, distance);
            let body = 0.55 + 0.45 * (distance / radius).min(1.0).powi(3);
            let weight = coverage * body * intensity;
            [color[0] * weight, color[1] * weight, color[2] * weight]
        });
    }

    /// A line segment with a bright head at `(x1, y1)` fading towards the tail.
    pub fn streak(
        &mut self,
        from: (f32, f32),
        to: (f32, f32),
        width: f32,
        color: Color,
        intensity: f32,
    ) {
        self.line(from, to, width, color, intensity, 1.6);
    }

    /// A line segment of even brightness.
    pub fn segment(
        &mut self,
        from: (f32, f32),
        to: (f32, f32),
        width: f32,
        color: Color,
        intensity: f32,
    ) {
        self.line(from, to, width, color, intensity, 0.0);
    }

    /// A soft line from `(x0, y0)` to `(x1, y1)`; `taper` > 0 fades it towards the start.
    pub fn line(
        &mut self,
        (x0, y0): (f32, f32),
        (x1, y1): (f32, f32),
        width: f32,
        color: Color,
        intensity: f32,
        taper: f32,
    ) {
        let (vx, vy) = (x1 - x0, y1 - y0);
        let length_squared = (vx * vx + vy * vy).max(1e-3);
        let reach = width * 3.0;
        let min_x = (x0.min(x1) - reach).floor().max(0.0) as usize;
        let max_x = ((x0.max(x1) + reach).ceil().max(0.0) as usize).min(WIDTH - 1);
        let min_y = (y0.min(y1) - reach).floor().max(0.0) as usize;
        let max_y = ((y0.max(y1) + reach).ceil().max(0.0) as usize).min(HEIGHT - 1);
        let inverse = 1.0 / (2.0 * width * width);
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let (px, py) = (x as f32 + 0.5 - x0, y as f32 + 0.5 - y0);
                let along = ((px * vx + py * vy) / length_squared).clamp(0.0, 1.0);
                let (ox, oy) = (px - along * vx, py - along * vy);
                let fade = if taper > 0.0 { along.powf(taper) } else { 1.0 };
                let weight = (-(ox * ox + oy * oy) * inverse).exp() * fade * intensity;
                if weight > 1e-4 {
                    self.add(x, y, color, weight);
                }
            }
        }
    }

    /// Paints `shade(x, y)` for every pixel in the rectangle (clipped to the frame).
    pub fn rect(
        &mut self,
        (x0, y0): (f32, f32),
        (x1, y1): (f32, f32),
        shade: impl Fn(f32, f32) -> Color,
    ) {
        let min_x = x0.floor().max(0.0) as usize;
        let max_x = (x1.ceil().max(0.0) as usize).min(WIDTH - 1);
        let min_y = y0.floor().max(0.0) as usize;
        let max_y = (y1.ceil().max(0.0) as usize).min(HEIGHT - 1);
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let color = shade(x as f32 + 0.5, y as f32 + 0.5);
                self.add(x, y, color, 1.0);
            }
        }
    }

    /// A four point twinkle: a soft core plus thin rays along both axes.
    pub fn star(&mut self, cx: f32, cy: f32, size: f32, color: Color, intensity: f32) {
        let ray_width = (size * 0.035).max(0.55);
        self.paint(cx, cy, size * 1.4, |dx, dy| {
            let core = (-(dx * dx + dy * dy) / (2.0 * (size * 0.1).powi(2))).exp();
            let horizontal = (-(dy * dy) / (2.0 * ray_width * ray_width)).exp()
                * (-dx.abs() / (size * 0.4)).exp();
            let vertical = (-(dx * dx) / (2.0 * ray_width * ray_width)).exp()
                * (-dy.abs() / (size * 0.4)).exp();
            let weight = (core * 1.2 + horizontal * 0.8 + vertical * 0.8) * intensity;
            [color[0] * weight, color[1] * weight, color[2] * weight]
        });
    }

    /// Adds a smooth colour field computed on a coarse grid and enlarged bilinearly.
    pub fn field(&mut self, cols: usize, rows: usize, shade: impl Fn(f32, f32) -> Color) {
        let mut grid = vec![[0.0f32; 3]; (cols + 1) * (rows + 1)];
        for row in 0..=rows {
            for col in 0..=cols {
                grid[row * (cols + 1) + col] =
                    shade(col as f32 / cols as f32, row as f32 / rows as f32);
            }
        }
        for y in 0..HEIGHT {
            let fy = y as f32 / HEIGHT as f32 * rows as f32;
            let (row, ty) = (fy as usize, fy.fract());
            for x in 0..WIDTH {
                let fx = x as f32 / WIDTH as f32 * cols as f32;
                let (col, tx) = (fx as usize, fx.fract());
                let at = |c: usize, r: usize| grid[r * (cols + 1) + c];
                let top = mix(at(col, row), at(col + 1, row), tx);
                let bottom = mix(at(col, row + 1), at(col + 1, row + 1), tx);
                self.add(x, y, mix(top, bottom, ty), 1.0);
            }
        }
    }

    pub fn to_rgb8(&self) -> Vec<u8> {
        self.pixels
            .iter()
            .map(|value| ((1.0 - (-value * 1.25).exp()) * 255.0 + 0.5).clamp(0.0, 255.0) as u8)
            .collect()
    }
}

pub type Draw = Box<dyn Fn(&mut Canvas, f32) + Send + Sync>;
