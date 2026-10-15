//! The original sixteen overlay effects.

use std::f32::consts::{PI, TAU};

use super::canvas::*;

pub fn rain(seed: u64) -> Draw {
    rain_scaled(seed, 1.0)
}

/// Rain with its density, length and brightness multiplied by `scale`.
pub fn rain_scaled(seed: u64, scale: f32) -> Draw {
    struct Drop {
        x: f32,
        y: f32,
        length: f32,
        width: f32,
        brightness: f32,
        cycles: f32,
        span_x: f32,
    }
    let slope = 0.2f32;
    let mut rng = Rng::new(seed);
    let mut drops = Vec::new();
    // (cycles, drops per million px², length, width, brightness)
    for (cycles, density, length, width, brightness) in [
        (9.0, 60.0, (30.0, 50.0), 0.9, 0.2),
        (11.0, 38.0, (55.0, 90.0), 1.2, 0.32),
        (13.0, 17.0, (100.0, 170.0), 1.8, 0.5),
    ] {
        let span_x = cycles * SPAN_Y * slope;
        let count = (density * scale * 1e-6 * span_x * SPAN_Y) as usize;
        for _ in 0..count {
            drops.push(Drop {
                x: rng.range(0.0, span_x),
                y: rng.range(0.0, SPAN_Y),
                length: rng.range(length.0, length.1) * scale.sqrt().min(1.0),
                width: width * rng.range(0.8, 1.2),
                brightness: brightness * scale.sqrt().min(1.0) * rng.range(0.7, 1.15),
                cycles,
                span_x,
            });
        }
    }
    Box::new(move |canvas, phase| {
        for drop in &drops {
            let y = travel(drop.y, drop.cycles, phase, SPAN_Y) - MARGIN;
            let x = travel(drop.x, 1.0, phase, drop.span_x) - MARGIN;
            let tail = (x - slope * drop.length, y - drop.length);
            canvas.streak(tail, (x, y), drop.width, [0.72, 0.8, 0.95], drop.brightness);
        }
        // A faint cold mist near the bottom keeps the rain from floating on nothing.
        canvas.field(48, 27, |u, v| {
            let haze =
                0.045 * smoothstep(0.55, 1.0, v) * (0.7 + 0.3 * (TAU * (u * 2.0 + phase)).sin());
            [haze * 0.7, haze * 0.8, haze]
        });
    })
}

pub fn snow(seed: u64) -> Draw {
    struct Flake {
        x: f32,
        y: f32,
        radius: f32,
        blur: f32,
        brightness: f32,
        cycles: f32,
        sway: f32,
        sway_cycles: f32,
        offset: f32,
    }
    let mut rng = Rng::new(seed);
    let mut flakes = Vec::new();
    // (count, radius, blur, brightness, cycles, sway)
    for (count, radius, blur, brightness, cycles, sway) in [
        (260, (1.4, 2.6), 0.7, 0.6, 1.0, 22.0),
        (140, (2.8, 4.8), 1.0, 0.75, 2.0, 38.0),
        (40, (8.0, 15.0), 4.5, 0.42, 3.0, 60.0),
    ] {
        for _ in 0..count {
            flakes.push(Flake {
                x: rng.range(0.0, SPAN_X),
                y: rng.range(0.0, SPAN_Y),
                radius: rng.range(radius.0, radius.1),
                blur,
                brightness: brightness * rng.range(0.7, 1.0),
                cycles,
                sway: sway * rng.range(0.6, 1.2),
                sway_cycles: rng.whole(1, 3),
                offset: rng.unit(),
            });
        }
    }
    Box::new(move |canvas, phase| {
        for flake in &flakes {
            let y = travel(flake.y, flake.cycles, phase, SPAN_Y) - MARGIN;
            let x = flake.x - MARGIN
                + flake.sway * (TAU * (flake.sway_cycles * phase + flake.offset)).sin();
            canvas.disc(
                x,
                y,
                flake.radius,
                flake.blur + 0.8,
                [0.92, 0.96, 1.0],
                flake.brightness,
            );
        }
    })
}

pub fn embers(seed: u64) -> Draw {
    struct Ember {
        x: f32,
        y: f32,
        rise: f32,
        size: f32,
        offset: f32,
        wobble: f32,
        wobble_cycles: f32,
        flicker_cycles: f32,
        drift: f32,
    }
    let mut rng = Rng::new(seed);
    let embers: Vec<Ember> = (0..230)
        .map(|_| {
            let rise = rng.range(380.0, 950.0);
            Ember {
                x: rng.range(0.0, WIDTH as f32),
                y: rng.range(rise * 0.5, HEIGHT as f32 + 80.0),
                rise,
                size: rng.range(2.0, 5.6),
                offset: rng.unit(),
                wobble: rng.range(10.0, 46.0),
                wobble_cycles: rng.whole(1, 3),
                flicker_cycles: rng.whole(3, 9),
                drift: rng.range(-60.0, 90.0),
            }
        })
        .collect();
    let white_hot = [1.0, 0.86, 0.55];
    let orange = [1.0, 0.42, 0.08];
    let red = [0.55, 0.08, 0.02];
    Box::new(move |canvas, phase| {
        for ember in &embers {
            let life = (phase + ember.offset).fract();
            let fade = (PI * life).sin().powf(0.8);
            let heat = 1.0 - life;
            let color = if heat > 0.5 {
                mix(orange, white_hot, (heat - 0.5) * 2.0)
            } else {
                mix(red, orange, heat * 2.0)
            };
            let flicker = 0.65 + 0.35 * (TAU * (ember.flicker_cycles * phase + ember.offset)).sin();
            let x = ember.x
                + ember.drift * life
                + ember.wobble * (TAU * (ember.wobble_cycles * phase + ember.offset)).sin();
            let y = ember.y - ember.rise * life;
            let size = ember.size * (1.0 - 0.45 * life);
            let strength = fade * flicker;
            canvas.glow(x, y, size * 3.6, color, 0.30 * strength);
            canvas.glow(
                x,
                y,
                size * 0.9,
                mix(color, [1.0, 0.95, 0.8], 0.5),
                1.1 * strength,
            );
            canvas.streak(
                (x - ember.drift * 0.05, y + size * 9.0),
                (x, y),
                size * 0.45,
                color,
                0.35 * strength,
            );
        }
    })
}

pub fn bokeh(seed: u64) -> Draw {
    struct Orb {
        x: f32,
        y: f32,
        radius: f32,
        softness: f32,
        brightness: f32,
        rise: f32,
        color: Color,
        pulse_cycles: f32,
        offset: f32,
        sway: f32,
    }
    let palette = [
        [1.0, 0.78, 0.4],
        [1.0, 0.62, 0.45],
        [1.0, 0.9, 0.7],
        [1.0, 0.55, 0.7],
        [0.85, 0.75, 1.0],
    ];
    let mut rng = Rng::new(seed);
    let orbs: Vec<Orb> = (0..46)
        .map(|index| {
            let far = index % 3 == 0;
            Orb {
                x: rng.range(0.0, SPAN_X),
                y: rng.range(0.0, SPAN_Y),
                radius: if far {
                    rng.range(46.0, 90.0)
                } else {
                    rng.range(16.0, 48.0)
                },
                softness: if far {
                    rng.range(14.0, 24.0)
                } else {
                    rng.range(2.5, 8.0)
                },
                brightness: if far {
                    rng.range(0.10, 0.2)
                } else {
                    rng.range(0.2, 0.42)
                },
                rise: if far { 1.0 } else { rng.pick(&[1.0, 2.0]) },
                color: rng.pick(&palette),
                pulse_cycles: rng.whole(1, 3),
                offset: rng.unit(),
                sway: rng.range(15.0, 55.0),
            }
        })
        .collect();
    Box::new(move |canvas, phase| {
        for orb in &orbs {
            let y = travel(orb.y, -orb.rise, phase, SPAN_Y) - MARGIN;
            let x = orb.x - MARGIN + orb.sway * (TAU * (phase + orb.offset)).sin();
            let breathe = 0.55 + 0.45 * pulse(orb.pulse_cycles, phase, orb.offset);
            canvas.disc(
                x,
                y,
                orb.radius,
                orb.softness,
                orb.color,
                orb.brightness * breathe,
            );
        }
    })
}

pub fn light_leak(_seed: u64) -> Draw {
    Box::new(move |canvas, phase| {
        let blobs: [(f32, f32, f32, Color, f32); 4] = [
            (
                0.05 + 0.10 * (TAU * phase).sin(),
                0.12 + 0.12 * (TAU * phase).cos(),
                0.42,
                [1.0, 0.55, 0.12],
                0.85,
            ),
            (
                0.98 - 0.08 * (TAU * phase).cos(),
                0.90 + 0.06 * (TAU * phase).sin(),
                0.48,
                [1.0, 0.25, 0.4],
                0.7,
            ),
            (
                0.50 + 0.30 * (TAU * (phase + 0.25)).sin(),
                -0.05,
                0.30,
                [1.0, 0.8, 0.35],
                0.55,
            ),
            (
                1.02,
                0.15 + 0.1 * (TAU * 2.0 * phase).sin(),
                0.28,
                [0.95, 0.35, 0.65],
                0.45,
            ),
        ];
        canvas.field(96, 54, |u, v| {
            let mut sum = [0.0f32; 3];
            for (cx, cy, radius, color, strength) in &blobs {
                let distance = ((u - cx) * 16.0 / 9.0).hypot(v - cy);
                let weight = (-(distance / radius).powi(2)).exp() * strength;
                for channel in 0..3 {
                    sum[channel] += color[channel] * weight;
                }
            }
            // A thin warm band that sweeps across once per loop and fades at both ends.
            let envelope = (PI * phase).sin().powi(2);
            let band = ((u + v * 0.35) - (-0.4 + 1.8 * phase)) / 0.09;
            let sweep = (-band * band).exp() * envelope * 0.55;
            [sum[0] + sweep, sum[1] + sweep * 0.8, sum[2] + sweep * 0.45]
        });
    })
}

pub fn sparkles(seed: u64) -> Draw {
    struct Spark {
        x: f32,
        y: f32,
        size: f32,
        color: Color,
        cycles: f32,
        offset: f32,
        drift: f32,
    }
    let palette = [
        [1.0, 1.0, 1.0],
        [1.0, 0.9, 0.6],
        [0.8, 0.9, 1.0],
        [1.0, 0.8, 0.85],
    ];
    let mut rng = Rng::new(seed);
    let sparks: Vec<Spark> = (0..110)
        .map(|_| Spark {
            x: rng.range(0.0, WIDTH as f32),
            y: rng.range(0.0, HEIGHT as f32),
            size: rng.range(8.0, 30.0),
            color: rng.pick(&palette),
            cycles: rng.whole(1, 3),
            offset: rng.unit(),
            drift: rng.range(4.0, 16.0),
        })
        .collect();
    Box::new(move |canvas, phase| {
        for spark in &sparks {
            let twinkle = pulse(spark.cycles, phase, spark.offset).powi(4);
            if twinkle < 0.02 {
                continue;
            }
            let wobble = spark.drift * (TAU * (phase + spark.offset)).sin();
            canvas.star(
                spark.x + wobble,
                spark.y + wobble * 0.5,
                spark.size * (0.55 + 0.45 * twinkle),
                spark.color,
                0.95 * twinkle,
            );
        }
    })
}

pub fn fireflies(seed: u64) -> Draw {
    struct Fly {
        x: f32,
        y: f32,
        amplitude: (f32, f32, f32, f32),
        cycles: (f32, f32, f32, f32),
        offset: (f32, f32, f32),
        pulse_cycles: f32,
        size: f32,
    }
    let mut rng = Rng::new(seed);
    let flies: Vec<Fly> = (0..30)
        .map(|_| Fly {
            x: rng.range(120.0, WIDTH as f32 - 120.0),
            y: rng.range(120.0, HEIGHT as f32 - 120.0),
            amplitude: (
                rng.range(40.0, 150.0),
                rng.range(20.0, 80.0),
                rng.range(30.0, 110.0),
                rng.range(15.0, 70.0),
            ),
            cycles: (
                rng.whole(1, 2),
                rng.whole(2, 4),
                rng.whole(1, 2),
                rng.whole(2, 5),
            ),
            offset: (rng.unit(), rng.unit(), rng.unit()),
            pulse_cycles: rng.whole(1, 3),
            size: rng.range(0.8, 1.6),
        })
        .collect();
    Box::new(move |canvas, phase| {
        for fly in &flies {
            let (ax, bx, ay, by) = fly.amplitude;
            let (c1, c2, c3, c4) = fly.cycles;
            let x = fly.x
                + ax * (TAU * (c1 * phase + fly.offset.0)).sin()
                + bx * (TAU * (c2 * phase + fly.offset.1)).sin();
            let y = fly.y
                + ay * (TAU * (c3 * phase + fly.offset.1)).sin()
                + by * (TAU * (c4 * phase + fly.offset.2)).sin();
            let glow = (0.15 + 0.85 * pulse(fly.pulse_cycles, phase, fly.offset.2)).powi(2);
            canvas.glow(x, y, 26.0 * fly.size, [0.55, 1.0, 0.25], 0.22 * glow);
            canvas.glow(x, y, 8.0 * fly.size, [0.85, 1.0, 0.45], 0.55 * glow);
            canvas.glow(x, y, 2.4 * fly.size, [1.0, 1.0, 0.85], 1.2 * glow);
        }
    })
}

pub fn dust(seed: u64) -> Draw {
    struct Mote {
        x: f32,
        y: f32,
        sigma: f32,
        brightness: f32,
        cycles: f32,
        sway: f32,
        offset: f32,
        twinkle: f32,
    }
    let mut rng = Rng::new(seed);
    let mut motes = Vec::new();
    for (count, sigma, brightness, cycles, sway) in [
        (260, (0.8, 1.2), 0.45, 1.0, 14.0),
        (140, (1.3, 2.0), 0.6, 1.0, 26.0),
        (40, (3.0, 6.0), 0.22, 2.0, 40.0),
    ] {
        for _ in 0..count {
            motes.push(Mote {
                x: rng.range(0.0, SPAN_X),
                y: rng.range(0.0, SPAN_Y),
                sigma: rng.range(sigma.0, sigma.1),
                brightness: brightness * rng.range(0.6, 1.0),
                cycles: -cycles,
                sway: sway * rng.range(0.5, 1.2),
                offset: rng.unit(),
                twinkle: rng.whole(1, 4),
            });
        }
    }
    Box::new(move |canvas, phase| {
        for mote in &motes {
            let y = travel(mote.y, mote.cycles, phase, SPAN_Y) - MARGIN;
            let x = mote.x - MARGIN + mote.sway * (TAU * (phase + mote.offset)).sin();
            let shimmer = 0.45 + 0.55 * pulse(mote.twinkle, phase, mote.offset);
            canvas.glow(
                x,
                y,
                mote.sigma,
                [1.0, 0.86, 0.55],
                mote.brightness * shimmer,
            );
        }
    })
}

pub fn stars(seed: u64) -> Draw {
    struct Star {
        x: f32,
        y: f32,
        size: f32,
        cycles: f32,
        offset: f32,
        color: Color,
    }
    let palette = [[1.0, 1.0, 1.0], [0.8, 0.88, 1.0], [1.0, 0.92, 0.8]];
    let mut rng = Rng::new(seed);
    let field: Vec<Star> = (0..360)
        .map(|_| Star {
            x: rng.range(0.0, WIDTH as f32),
            y: rng.range(0.0, HEIGHT as f32 * 0.85),
            size: rng.range(0.7, 2.0).powi(2),
            cycles: rng.whole(1, 4),
            offset: rng.unit(),
            color: rng.pick(&palette),
        })
        .collect();
    let shooters: Vec<(f32, f32, f32, f32)> = (0..3)
        .map(|index| {
            (
                index as f32 / 3.0 + rng.range(0.0, 0.2),
                rng.range(0.1, 0.7) * WIDTH as f32,
                rng.range(0.05, 0.4) * HEIGHT as f32,
                rng.range(0.9, 1.3),
            )
        })
        .collect();
    Box::new(move |canvas, phase| {
        for star in &field {
            let twinkle = 0.35 + 0.65 * pulse(star.cycles, phase, star.offset);
            if star.size > 2.6 {
                canvas.star(star.x, star.y, star.size * 4.5, star.color, 0.9 * twinkle);
            } else {
                canvas.glow(
                    star.x,
                    star.y,
                    star.size * 0.55 + 0.35,
                    star.color,
                    0.9 * twinkle,
                );
            }
        }
        for &(start, x, y, speed) in &shooters {
            let progress = (phase - start).rem_euclid(1.0) / 0.09;
            if progress < 1.0 {
                let travelled = progress * 620.0 * speed;
                let head = (x + travelled, y + travelled * 0.45);
                let tail = (head.0 - 190.0, head.1 - 85.0);
                let fade = (PI * progress).sin();
                canvas.streak(tail, head, 1.5, [0.85, 0.92, 1.0], 1.1 * fade);
                canvas.glow(head.0, head.1, 3.0, [1.0, 1.0, 1.0], 1.0 * fade);
            }
        }
    })
}

pub fn confetti(seed: u64) -> Draw {
    struct Piece {
        x: f32,
        y: f32,
        half_width: f32,
        half_height: f32,
        color: Color,
        cycles: f32,
        spin: f32,
        flip: f32,
        offset: f32,
        sway: f32,
    }
    let palette = [
        [1.0, 0.25, 0.35],
        [1.0, 0.8, 0.15],
        [0.2, 0.75, 1.0],
        [0.45, 0.95, 0.4],
        [0.85, 0.4, 1.0],
        [1.0, 0.55, 0.15],
    ];
    let mut rng = Rng::new(seed);
    let pieces: Vec<Piece> = (0..170)
        .map(|_| Piece {
            x: rng.range(0.0, SPAN_X),
            y: rng.range(0.0, SPAN_Y),
            half_width: rng.range(8.0, 17.0),
            half_height: rng.range(5.0, 10.0),
            color: rng.pick(&palette),
            cycles: rng.pick(&[1.0, 2.0]),
            spin: rng.whole(1, 3) * rng.pick(&[-1.0, 1.0]),
            flip: rng.whole(2, 5),
            offset: rng.unit(),
            sway: rng.range(25.0, 80.0),
        })
        .collect();
    Box::new(move |canvas, phase| {
        for piece in &pieces {
            let y = travel(piece.y, piece.cycles, phase, SPAN_Y) - MARGIN;
            let x = piece.x - MARGIN + piece.sway * (TAU * (phase + piece.offset)).sin();
            let angle = TAU * (piece.spin * phase + piece.offset);
            let (sin, cos) = angle.sin_cos();
            let flip = (TAU * (piece.flip * phase + piece.offset)).cos();
            let shade = 0.45 + 0.55 * flip.abs();
            let (hw, hh) = (piece.half_width, (piece.half_height * flip.abs()).max(0.6));
            canvas.paint(x, y, hw * 1.5 + 2.0, |dx, dy| {
                let (u, v) = (dx * cos + dy * sin, -dx * sin + dy * cos);
                let edge = (u.abs() - hw).max(v.abs() - hh);
                let coverage = smoothstep(0.8, -0.8, edge) * shade * 0.8;
                [
                    piece.color[0] * coverage,
                    piece.color[1] * coverage,
                    piece.color[2] * coverage,
                ]
            });
        }
    })
}

pub fn hearts(seed: u64) -> Draw {
    struct Heart {
        x: f32,
        y: f32,
        size: f32,
        color: Color,
        sway: f32,
        offset: f32,
        cycles: f32,
        tilt: f32,
    }
    let palette = [
        [1.0, 0.3, 0.5],
        [1.0, 0.45, 0.62],
        [1.0, 0.2, 0.32],
        [1.0, 0.7, 0.8],
    ];
    let mut rng = Rng::new(seed);
    let hearts: Vec<Heart> = (0..30)
        .map(|_| Heart {
            x: rng.range(0.0, SPAN_X),
            y: rng.range(0.0, SPAN_Y),
            size: rng.range(14.0, 46.0),
            color: rng.pick(&palette),
            sway: rng.range(25.0, 70.0),
            offset: rng.unit(),
            cycles: rng.pick(&[1.0, 1.0, 2.0]),
            tilt: rng.range(0.1, 0.35),
        })
        .collect();
    Box::new(move |canvas, phase| {
        for heart in &hearts {
            let y = travel(heart.y, -heart.cycles, phase, SPAN_Y) - MARGIN;
            let x = heart.x - MARGIN + heart.sway * (TAU * (phase + heart.offset)).sin();
            let beat = 1.0 + 0.06 * (TAU * (3.0 * phase + heart.offset)).sin();
            let size = heart.size * beat;
            let angle = heart.tilt * (TAU * (phase + heart.offset)).sin();
            let (sin, cos) = angle.sin_cos();
            canvas.glow(x, y, size * 1.3, heart.color, 0.16);
            canvas.paint(x, y, size * 1.5, |dx, dy| {
                let (u, v) = (dx * cos + dy * sin, -dx * sin + dy * cos);
                let (hx, hy) = (u / size, -v / size);
                let value = (hx * hx + hy * hy - 1.0).powi(3) - hx * hx * hy.powi(3);
                let coverage = smoothstep(0.05, -0.05, value);
                let shine =
                    0.75 + 0.45 * (-(hx + 0.35).powi(2) * 8.0 - (hy - 0.35).powi(2) * 8.0).exp();
                let weight = coverage * shine * 0.9;
                [
                    heart.color[0] * weight,
                    heart.color[1] * weight,
                    heart.color[2] * weight,
                ]
            });
        }
    })
}

pub fn petals(seed: u64) -> Draw {
    petals_colored(seed, [1.0, 0.93, 0.95], [1.0, 0.62, 0.74], 80)
}

/// Falling petals fading from `pale` at the base to `pink` at the tip.
pub fn petals_colored(seed: u64, pale: Color, pink: Color, count: u32) -> Draw {
    struct Petal {
        x: f32,
        y: f32,
        length: f32,
        sway: f32,
        offset: f32,
        spin: f32,
        tumble: f32,
        cycles: f32,
        tone: f32,
    }
    let mut rng = Rng::new(seed);
    let petals: Vec<Petal> = (0..count)
        .map(|_| Petal {
            x: rng.range(0.0, SPAN_X),
            y: rng.range(0.0, SPAN_Y),
            length: rng.range(15.0, 32.0),
            sway: rng.range(60.0, 160.0),
            offset: rng.unit(),
            spin: rng.whole(1, 2) * rng.pick(&[-1.0, 1.0]),
            tumble: rng.whole(2, 4),
            cycles: rng.pick(&[1.0, 1.0, 2.0]),
            tone: rng.unit(),
        })
        .collect();
    Box::new(move |canvas, phase| {
        for petal in &petals {
            let y = travel(petal.y, petal.cycles, phase, SPAN_Y) - MARGIN;
            let x = petal.x - MARGIN + petal.sway * (TAU * (phase + petal.offset)).sin();
            let angle = TAU * (petal.spin * phase + petal.offset);
            let (sin, cos) = angle.sin_cos();
            let squash = 0.35 + 0.65 * (TAU * (petal.tumble * phase + petal.offset)).cos().abs();
            let (a, b) = (petal.length, petal.length * 0.55 * squash);
            canvas.paint(x, y, a * 1.4, |dx, dy| {
                let (u, v) = (
                    (dx * cos + dy * sin) / a,
                    (-dx * sin + dy * cos) / b.max(0.5),
                );
                // An egg shaped body, fatter towards the tip, with a small notch at the end.
                let body = u * u * (1.0 + 0.35 * u) + v * v;
                let notch = (-(v * v) * 18.0 - (u - 1.0).powi(2) * 18.0).exp() * 0.5;
                let coverage = smoothstep(1.0, 0.82, body + notch);
                let tint = mix(pale, pink, (0.35 + 0.5 * petal.tone) * (0.5 + 0.5 * u));
                let weight = coverage * 0.8;
                [tint[0] * weight, tint[1] * weight, tint[2] * weight]
            });
        }
    })
}

pub fn mist(_seed: u64) -> Draw {
    Box::new(move |canvas, phase| {
        canvas.field(64, 36, |u, v| {
            let a = (TAU * (1.7 * u + 0.9 * v + phase)).sin();
            let b = (TAU * (3.1 * u - 1.3 * v - phase + 0.3)).sin();
            let c = (TAU * (0.6 * u + 2.2 * v + 2.0 * phase + 0.7)).sin();
            let d = (TAU * (5.3 * u + 0.7 * v - 3.0 * phase)).sin();
            let density = (0.5 + 0.5 * (a * 0.4 + b * 0.3 + c * 0.2 + d * 0.1)).powi(2);
            let amount = density * (0.25 + 0.75 * v) * 0.34;
            [amount * 0.78, amount * 0.85, amount * 0.98]
        });
    })
}

pub fn aurora(_seed: u64) -> Draw {
    Box::new(move |canvas, phase| {
        canvas.field(160, 90, |u, v| {
            let mut total = [0.0f32; 3];
            for (index, (base, hue)) in [(0.30, 0.0f32), (0.42, 0.5)].into_iter().enumerate() {
                let shift = index as f32 * 0.37;
                let curve = base
                    + 0.08 * (TAU * (1.3 * u + phase + shift)).sin()
                    + 0.045 * (TAU * (2.7 * u - 2.0 * phase + shift)).sin();
                let reach = if v < curve { 0.22 } else { 0.09 };
                let band = (-((v - curve) / reach).powi(2)).exp();
                let rays = 0.55
                    + 0.45
                        * (TAU * (11.0 * u + 0.5 * (TAU * (2.0 * u + phase)).sin() + shift)).sin();
                let color = mix(
                    mix([0.1, 1.0, 0.5], [0.1, 0.8, 0.9], hue),
                    [0.65, 0.3, 1.0],
                    smoothstep(curve, curve + 0.2, v),
                );
                let weight = band * rays * 0.55;
                for channel in 0..3 {
                    total[channel] += color[channel] * weight;
                }
            }
            total
        });
    })
}

pub fn rays(_seed: u64) -> Draw {
    Box::new(move |canvas, phase| {
        canvas.field(160, 90, |u, v| {
            let (dx, dy) = (u * 16.0 / 9.0 - 0.1, v + 0.12);
            let angle = dy.atan2(dx);
            let distance = dx.hypot(dy);
            let wide = (0.5 + 0.5 * (angle * 17.0 + 0.9 * (TAU * phase).sin()).sin()).powi(3);
            let fine = (0.5 + 0.5 * (angle * 41.0 - 0.7 * (TAU * phase + 1.0).sin()).sin()).powi(4);
            let strength = (wide * 0.7 + fine * 0.4) * (-distance * 1.1).exp() * 0.75;
            [strength, strength * 0.84, strength * 0.55]
        });
    })
}

pub fn lens_flare(_seed: u64) -> Draw {
    Box::new(move |canvas, phase| {
        let envelope = (PI * phase).sin();
        let head_x = (-0.15 + 1.3 * phase) * WIDTH as f32;
        let y = HEIGHT as f32 * 0.46;
        canvas.field(160, 90, |u, v| {
            let (x, py) = (u * WIDTH as f32, v * HEIGHT as f32);
            let band = (-((py - y) / 5.0).powi(2)).exp();
            let reach = if x < head_x { 420.0 } else { 90.0 };
            let along = (-((x - head_x).abs() / reach)).exp();
            let streak = band * along * 0.8 * envelope;
            let halo = (-(((py - y) / 90.0).powi(2) + ((x - head_x) / 260.0).powi(2))).exp()
                * 0.28
                * envelope;
            [
                streak * 0.55 + halo * 0.6,
                streak * 0.75 + halo * 0.7,
                streak + halo,
            ]
        });
        canvas.glow(head_x, y, 14.0, [0.9, 0.95, 1.0], 1.0 * envelope);
        canvas.glow(head_x, y, 46.0, [0.5, 0.7, 1.0], 0.5 * envelope);
        // Ghost reflections mirrored around the middle of the frame.
        let centre = (WIDTH as f32 / 2.0, HEIGHT as f32 / 2.0);
        for (scale, radius, color) in [
            (-0.35, 34.0, [1.0, 0.6, 0.3]),
            (-0.7, 20.0, [0.5, 1.0, 0.7]),
            (-1.2, 52.0, [0.7, 0.5, 1.0]),
        ] {
            let ghost_x = centre.0 + (head_x - centre.0) * scale;
            canvas.disc(
                ghost_x,
                centre.1 + (y - centre.1) * scale,
                radius,
                6.0,
                color,
                0.22 * envelope,
            );
        }
    })
}
