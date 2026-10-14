//! Additional overlay effects: weather, celebration, music and stylised looks.

use std::f32::consts::{PI, TAU};

use super::canvas::{
    mix, pulse, smoothstep, travel, Color, Draw, Rng, FRAMES, HEIGHT, MARGIN, SPAN_X, SPAN_Y, WIDTH,
};

fn spectrum(t: f32) -> Color {
    const STOPS: [Color; 7] = [
        [1.0, 0.15, 0.15],
        [1.0, 0.55, 0.1],
        [1.0, 0.95, 0.2],
        [0.25, 0.9, 0.3],
        [0.2, 0.7, 1.0],
        [0.3, 0.3, 1.0],
        [0.65, 0.25, 0.95],
    ];
    let scaled = t.clamp(0.0, 1.0) * (STOPS.len() - 1) as f32;
    let index = (scaled as usize).min(STOPS.len() - 2);
    mix(STOPS[index], STOPS[index + 1], scaled - index as f32)
}

fn scaled(color: Color, factor: f32) -> Color {
    [color[0] * factor, color[1] * factor, color[2] * factor]
}

fn segment_distance(point: (f32, f32), from: (f32, f32), to: (f32, f32)) -> f32 {
    let (vx, vy) = (to.0 - from.0, to.1 - from.1);
    let (px, py) = (point.0 - from.0, point.1 - from.1);
    let along = ((px * vx + py * vy) / (vx * vx + vy * vy).max(1e-6)).clamp(0.0, 1.0);
    (px - along * vx).hypot(py - along * vy)
}

pub fn shooting_stars(seed: u64) -> Draw {
    struct Meteor {
        start: f32,
        x: f32,
        y: f32,
        length: f32,
        reach: f32,
        color: Color,
    }
    let palette = [[0.9, 0.95, 1.0], [1.0, 0.92, 0.75], [0.75, 0.85, 1.0]];
    let mut rng = Rng::new(seed);
    let meteors: Vec<Meteor> = (0..9)
        .map(|index| Meteor {
            start: (index as f32 + rng.range(0.0, 0.6)) / 9.0,
            x: rng.range(0.0, 0.75) * WIDTH as f32,
            y: rng.range(0.0, 0.45) * HEIGHT as f32,
            length: rng.range(240.0, 460.0),
            reach: rng.range(550.0, 900.0),
            color: rng.pick(&palette),
        })
        .collect();
    Box::new(move |canvas, phase| {
        for meteor in &meteors {
            let progress = (phase - meteor.start).rem_euclid(1.0) / 0.12;
            if progress >= 1.0 {
                continue;
            }
            let travelled = progress * meteor.reach;
            let head = (meteor.x + travelled * 0.87, meteor.y + travelled * 0.5);
            let tail = (head.0 - meteor.length * 0.87, head.1 - meteor.length * 0.5);
            let fade = (PI * progress).sin();
            canvas.streak(tail, head, 2.0, meteor.color, 1.2 * fade);
            canvas.glow(head.0, head.1, 5.0, [1.0, 1.0, 1.0], 1.2 * fade);
            canvas.glow(head.0, head.1, 18.0, meteor.color, 0.3 * fade);
        }
    })
}

pub fn fireworks(seed: u64) -> Draw {
    struct Burst {
        start: f32,
        x: f32,
        y: f32,
        radius: f32,
        colors: [Color; 2],
        sparks: Vec<(f32, f32)>,
    }
    let palettes = [
        [[1.0, 0.35, 0.3], [1.0, 0.8, 0.4]],
        [[0.3, 0.7, 1.0], [0.8, 0.95, 1.0]],
        [[1.0, 0.85, 0.3], [1.0, 0.55, 0.15]],
        [[0.9, 0.4, 1.0], [1.0, 0.7, 0.9]],
        [[0.4, 1.0, 0.6], [0.9, 1.0, 0.5]],
    ];
    let mut rng = Rng::new(seed);
    let bursts: Vec<Burst> = (0..4)
        .map(|index| Burst {
            start: index as f32 / 4.0 + rng.range(0.0, 0.05),
            x: rng.range(0.2, 0.8) * WIDTH as f32,
            y: rng.range(0.15, 0.45) * HEIGHT as f32,
            radius: rng.range(240.0, 340.0),
            colors: rng.pick(&palettes),
            sparks: (0..90)
                .map(|_| (rng.range(0.0, TAU), rng.range(0.35, 1.0)))
                .collect(),
        })
        .collect();
    const ROCKET: f32 = 0.07;
    const BURST: f32 = 0.30;
    Box::new(move |canvas, phase| {
        for burst in &bursts {
            let time = (phase - burst.start).rem_euclid(1.0);
            if time < ROCKET {
                let progress = time / ROCKET;
                let y = HEIGHT as f32 - progress * (HEIGHT as f32 - burst.y);
                canvas.streak(
                    (burst.x, y + 110.0),
                    (burst.x, y),
                    1.6,
                    [1.0, 0.85, 0.6],
                    0.9,
                );
                canvas.glow(burst.x, y, 4.0, [1.0, 0.95, 0.8], 1.0);
                continue;
            }
            let life = (time - ROCKET) / BURST;
            if life >= 1.0 {
                continue;
            }
            let position = |u: f32, angle: f32, speed: f32| {
                let distance = burst.radius * speed * (1.0 - (-3.4 * u).exp());
                (
                    burst.x + angle.cos() * distance,
                    burst.y + angle.sin() * distance + 360.0 * u * u,
                )
            };
            let fade = (1.0 - life).powf(1.4);
            if life < 0.12 {
                canvas.glow(
                    burst.x,
                    burst.y,
                    90.0 * (1.0 - life / 0.12),
                    [1.0, 0.95, 0.85],
                    0.6,
                );
            }
            for (index, &(angle, speed)) in burst.sparks.iter().enumerate() {
                let head = position(life, angle, speed);
                let tail = position((life - 0.04).max(0.0), angle, speed);
                let color = burst.colors[index % 2];
                canvas.streak(tail, head, 1.8, color, 1.5 * fade);
                canvas.glow(head.0, head.1, 3.4, color, 1.2 * fade);
            }
        }
    })
}

pub fn lightning(seed: u64) -> Draw {
    struct Strike {
        start: f32,
        bolt: Vec<(f32, f32)>,
        branches: Vec<Vec<(f32, f32)>>,
    }
    fn jagged(
        rng: &mut Rng,
        from: (f32, f32),
        to: (f32, f32),
        parts: usize,
        jitter: f32,
    ) -> Vec<(f32, f32)> {
        (0..=parts)
            .map(|index| {
                let t = index as f32 / parts as f32;
                let shake = if index == 0 || index == parts {
                    0.0
                } else {
                    rng.range(-jitter, jitter)
                };
                (
                    from.0 + (to.0 - from.0) * t + shake,
                    from.1 + (to.1 - from.1) * t,
                )
            })
            .collect()
    }
    let mut rng = Rng::new(seed);
    let strikes: Vec<Strike> = [0.18, 0.62]
        .into_iter()
        .map(|start| {
            let x = rng.range(0.25, 0.75) * WIDTH as f32;
            let end_x = x + rng.range(-160.0, 160.0);
            let bolt = jagged(
                &mut rng,
                (x, -10.0),
                (end_x, HEIGHT as f32 * 0.82),
                16,
                48.0,
            );
            let branches = (0..3)
                .map(|_| {
                    let anchor = bolt[rng.whole(3, 11) as usize];
                    let side = rng.pick(&[-1.0, 1.0]);
                    let end = (
                        anchor.0 + side * rng.range(120.0, 260.0),
                        anchor.1 + rng.range(150.0, 300.0),
                    );
                    jagged(&mut rng, anchor, end, 6, 28.0)
                })
                .collect();
            Strike {
                start,
                bolt,
                branches,
            }
        })
        .collect();
    Box::new(move |canvas, phase| {
        for strike in &strikes {
            let time = (phase - strike.start).rem_euclid(1.0);
            let flash = (-time / 0.012).exp()
                + if time > 0.04 {
                    0.6 * (-(time - 0.04) / 0.02).exp()
                } else {
                    0.0
                };
            if flash < 0.02 {
                continue;
            }
            canvas.field(48, 27, |_, v| {
                let sky = 0.28 * flash * (1.0 - v).powf(1.5);
                [sky * 0.7, sky * 0.8, sky]
            });
            let paths = std::iter::once((&strike.bolt, 1.0))
                .chain(strike.branches.iter().map(|branch| (branch, 0.6)));
            for (path, strength) in paths {
                for pair in path.windows(2) {
                    canvas.segment(
                        pair[0],
                        pair[1],
                        7.0,
                        [0.55, 0.65, 1.0],
                        0.3 * flash * strength,
                    );
                    canvas.segment(
                        pair[0],
                        pair[1],
                        1.7,
                        [0.95, 0.97, 1.0],
                        1.4 * flash * strength,
                    );
                }
            }
        }
    })
}

pub fn bubbles(seed: u64) -> Draw {
    struct Bubble {
        x: f32,
        y: f32,
        radius: f32,
        cycles: f32,
        sway: f32,
        offset: f32,
        tint: Color,
    }
    let tints = [
        [0.7, 0.9, 1.0],
        [0.85, 0.8, 1.0],
        [0.75, 1.0, 0.95],
        [1.0, 0.85, 0.95],
    ];
    let mut rng = Rng::new(seed);
    let bubbles: Vec<Bubble> = (0..46)
        .map(|_| Bubble {
            x: rng.range(0.0, SPAN_X),
            y: rng.range(0.0, SPAN_Y),
            radius: rng.range(10.0, 62.0),
            cycles: rng.pick(&[1.0, 1.0, 2.0]),
            sway: rng.range(20.0, 70.0),
            offset: rng.unit(),
            tint: rng.pick(&tints),
        })
        .collect();
    Box::new(move |canvas, phase| {
        for bubble in &bubbles {
            let y = travel(bubble.y, -bubble.cycles, phase, SPAN_Y) - MARGIN;
            let x = bubble.x - MARGIN + bubble.sway * (TAU * (phase + bubble.offset)).sin();
            let r = bubble.radius;
            canvas.paint(x, y, r + 4.0, |dx, dy| {
                let d = dx.hypot(dy);
                let rim = (-((d - r) / 1.7).powi(2)).exp()
                    * (0.45 + 0.35 * ((dx + dy) / r).clamp(-1.0, 1.0).abs());
                let film = smoothstep(r, r - 3.0, d) * 0.07;
                let spot = (-((dx + 0.4 * r).powi(2) + (dy + 0.42 * r).powi(2))
                    / (2.0 * (0.14 * r).powi(2)))
                .exp();
                let glint = (-((dx - 0.45 * r).powi(2) + (dy - 0.5 * r).powi(2))
                    / (2.0 * (0.08 * r).powi(2)))
                .exp()
                    * 0.4;
                scaled(bubble.tint, rim * 0.9 + film)
                    .map(|c| c + (spot * 0.95 + glint) * 0.8 * smoothstep(r, r - 3.0, d))
            });
        }
    })
}

pub fn leaves(seed: u64) -> Draw {
    struct Leaf {
        x: f32,
        y: f32,
        length: f32,
        sway: f32,
        offset: f32,
        spin: f32,
        tumble: f32,
        cycles: f32,
        color: Color,
    }
    let palette = [
        [1.0, 0.5, 0.1],
        [0.9, 0.2, 0.08],
        [1.0, 0.75, 0.15],
        [0.75, 0.4, 0.08],
        [0.95, 0.6, 0.2],
    ];
    let mut rng = Rng::new(seed);
    let leaves: Vec<Leaf> = (0..64)
        .map(|_| Leaf {
            x: rng.range(0.0, SPAN_X),
            y: rng.range(0.0, SPAN_Y),
            length: rng.range(20.0, 44.0),
            sway: rng.range(70.0, 170.0),
            offset: rng.unit(),
            spin: rng.whole(1, 2) * rng.pick(&[-1.0, 1.0]),
            tumble: rng.whole(2, 4),
            cycles: rng.pick(&[1.0, 1.0, 2.0]),
            color: rng.pick(&palette),
        })
        .collect();
    Box::new(move |canvas, phase| {
        for leaf in &leaves {
            let y = travel(leaf.y, leaf.cycles, phase, SPAN_Y) - MARGIN;
            let x = leaf.x - MARGIN + leaf.sway * (TAU * (phase + leaf.offset)).sin();
            let (sin, cos) = (TAU * (leaf.spin * phase + leaf.offset)).sin_cos();
            let squash = 0.3 + 0.7 * (TAU * (leaf.tumble * phase + leaf.offset)).cos().abs();
            let size = leaf.length;
            canvas.paint(x, y, size * 1.2, |dx, dy| {
                let (u, v) = (
                    (dx * cos + dy * sin) / size,
                    (-dx * sin + dy * cos) / (size * squash),
                );
                if u.abs() >= 1.0 {
                    return [0.0; 3];
                }
                let half = 0.5 * (1.0 - u * u).powf(0.7);
                let coverage = smoothstep(half + 0.05, half - 0.05, v.abs());
                let vein = if v.abs() < 0.035 { 0.65 } else { 1.0 };
                scaled(
                    leaf.color,
                    coverage * vein * (0.55 + 0.45 * (1.0 - v.abs() / half.max(0.05))) * 0.85,
                )
            });
        }
    })
}

pub fn smoke(_seed: u64) -> Draw {
    Box::new(move |canvas, phase| {
        canvas.field(96, 54, |u, v| {
            let a = (TAU * (1.9 * u + 1.1 * v + 2.0 * phase)).sin();
            let b = (TAU * (3.4 * u - 0.8 * v + 3.0 * phase + 0.2)).sin();
            let c = (TAU * (0.8 * u + 2.1 * v + 4.0 * phase + 0.6)).sin();
            let d = (TAU * (6.2 * u + 1.6 * v + 3.0 * phase)).sin();
            let density = (0.5 + 0.5 * (a * 0.35 + b * 0.3 + c * 0.25 + d * 0.1)).powi(3);
            let amount = density * (0.15 + 0.85 * v) * 0.55;
            [amount * 0.82, amount * 0.84, amount * 0.9]
        });
    })
}

/// Coverage of a music note at `(x, y)`, measured in note heights with y pointing up.
fn note_coverage(kind: u8, x: f32, y: f32) -> f32 {
    let head = |hx: f32, hy: f32| {
        let (sin, cos) = 0.35f32.sin_cos();
        let (dx, dy) = (x - hx, y - hy);
        let (u, v) = ((dx * cos + dy * sin) / 0.36, (-dx * sin + dy * cos) / 0.25);
        smoothstep(1.0, 0.78, u * u + v * v)
    };
    let stem = |sx: f32, top: f32, bottom: f32| {
        smoothstep(0.07, 0.035, (x - sx).abs())
            * smoothstep(bottom - 0.05, bottom + 0.03, y)
            * smoothstep(top + 0.05, top - 0.03, y)
    };
    match kind {
        0 => head(0.0, 0.0).max(stem(0.32, 1.5, 0.0)),
        1 => {
            let flag_x = x - 0.32;
            let centre = 1.4 - 0.8 * flag_x;
            let thickness = 0.13 * (1.0 - flag_x / 0.6).max(0.0);
            let flag = smoothstep(thickness + 0.03, thickness - 0.03, (y - centre).abs())
                * smoothstep(-0.02, 0.03, flag_x)
                * smoothstep(0.6, 0.5, flag_x);
            head(0.0, 0.0).max(stem(0.32, 1.5, 0.0)).max(flag)
        }
        _ => {
            let beam = smoothstep(0.12, 0.07, (y - 1.4).abs())
                * smoothstep(0.25, 0.32, x)
                * smoothstep(1.05, 0.98, x);
            head(0.0, 0.0)
                .max(head(0.72, 0.12))
                .max(stem(0.32, 1.5, 0.0))
                .max(stem(1.04, 1.5, 0.12))
                .max(beam)
        }
    }
}

pub fn music_notes(seed: u64) -> Draw {
    struct Note {
        x: f32,
        y: f32,
        size: f32,
        kind: u8,
        sway: f32,
        offset: f32,
        cycles: f32,
        color: Color,
        tilt: f32,
    }
    let palette = [
        [1.0, 1.0, 1.0],
        [1.0, 0.85, 0.4],
        [1.0, 0.5, 0.7],
        [0.5, 0.9, 1.0],
        [0.75, 0.6, 1.0],
    ];
    let mut rng = Rng::new(seed);
    let notes: Vec<Note> = (0..26)
        .map(|_| Note {
            x: rng.range(0.0, SPAN_X),
            y: rng.range(0.0, SPAN_Y),
            size: rng.range(26.0, 56.0),
            kind: rng.whole(0, 2) as u8,
            sway: rng.range(35.0, 90.0),
            offset: rng.unit(),
            cycles: rng.pick(&[1.0, 1.0, 2.0]),
            color: rng.pick(&palette),
            tilt: rng.range(0.1, 0.3),
        })
        .collect();
    Box::new(move |canvas, phase| {
        for note in &notes {
            let y = travel(note.y, -note.cycles, phase, SPAN_Y) - MARGIN;
            let x = note.x - MARGIN + note.sway * (TAU * (phase + note.offset)).sin();
            let (sin, cos) = (note.tilt * (TAU * (phase + note.offset)).sin()).sin_cos();
            let size = note.size;
            canvas.glow(x, y, size * 0.9, note.color, 0.12);
            canvas.paint(x, y, size * 1.6, |dx, dy| {
                let (u, v) = (dx * cos + dy * sin, -dx * sin + dy * cos);
                let coverage = note_coverage(note.kind, u / size + 0.4, -v / size + 0.7);
                scaled(note.color, coverage * 0.95)
            });
        }
    })
}

pub fn ripples(seed: u64) -> Draw {
    struct Source {
        x: f32,
        y: f32,
        offset: f32,
        color: Color,
    }
    let palette = [[0.5, 0.9, 1.0], [0.8, 0.95, 1.0], [0.6, 0.7, 1.0]];
    let mut rng = Rng::new(seed);
    let sources: Vec<Source> = (0..6)
        .map(|_| Source {
            x: rng.range(0.1, 0.9) * WIDTH as f32,
            y: rng.range(0.15, 0.85) * HEIGHT as f32,
            offset: rng.unit(),
            color: rng.pick(&palette),
        })
        .collect();
    Box::new(move |canvas, phase| {
        for source in &sources {
            for ring in 0..3 {
                let life = (phase + source.offset + ring as f32 / 3.0).fract();
                let radius = life * 300.0;
                let width = 2.0 + 3.5 * life;
                let strength = (1.0 - life).powf(1.6) * smoothstep(0.0, 0.06, life) * 0.8;
                canvas.paint(source.x, source.y, radius + width * 3.0, |dx, dy| {
                    let band = (-((dx.hypot(dy) - radius) / width).powi(2)).exp() * strength;
                    scaled(source.color, band)
                });
            }
        }
    })
}

pub fn film_dust(seed: u64) -> Draw {
    Box::new(move |canvas, phase| {
        // Dust holds for two frames, as it does on real film.
        let step = (phase * FRAMES as f32).round() as u64 % FRAMES as u64 / 2;
        let mut rng = Rng::new(seed.wrapping_add(step * 7919));
        for _ in 0..rng.whole(8, 20) as u32 {
            let (x, y) = (rng.range(0.0, WIDTH as f32), rng.range(0.0, HEIGHT as f32));
            canvas.glow(
                x,
                y,
                rng.range(0.9, 3.2),
                [1.0, 0.97, 0.9],
                rng.range(0.35, 0.9),
            );
        }
        for _ in 0..rng.whole(0, 2) as u32 {
            let from = (rng.range(0.0, WIDTH as f32), rng.range(0.0, HEIGHT as f32));
            let to = (
                from.0 + rng.range(-70.0, 70.0),
                from.1 + rng.range(-40.0, 40.0),
            );
            canvas.segment(from, to, 0.8, [1.0, 0.97, 0.9], 0.5);
        }
        if rng.unit() < 0.4 {
            let x = rng.range(0.0, WIDTH as f32);
            canvas.segment(
                (x, 0.0),
                (x + rng.range(-6.0, 6.0), HEIGHT as f32),
                0.7,
                [1.0, 0.97, 0.9],
                0.3,
            );
        }
    })
}

pub fn matrix(seed: u64) -> Draw {
    struct Column {
        x: f32,
        y: f32,
        cycles: f32,
        length: f32,
        blips: [(f32, f32); 3],
    }
    let mut rng = Rng::new(seed);
    let columns: Vec<Column> = (0..74)
        .map(|index| Column {
            x: index as f32 * 26.0 + 13.0,
            y: rng.range(0.0, SPAN_Y),
            cycles: rng.whole(3, 6),
            length: rng.range(140.0, 420.0),
            blips: [
                (rng.range(20.0, 120.0), rng.whole(2, 5)),
                (rng.range(60.0, 220.0), rng.whole(2, 5)),
                (rng.range(100.0, 320.0), rng.whole(2, 5)),
            ],
        })
        .collect();
    Box::new(move |canvas, phase| {
        for column in &columns {
            let head = travel(column.y, column.cycles, phase, SPAN_Y) - MARGIN;
            canvas.streak(
                (column.x, head - column.length),
                (column.x, head),
                1.7,
                [0.15, 1.0, 0.35],
                0.75,
            );
            canvas.glow(column.x, head, 4.0, [0.85, 1.0, 0.9], 1.1);
            for (distance, cycles) in column.blips {
                let flicker = pulse(cycles, phase, distance / 300.0).powi(2);
                canvas.glow(
                    column.x,
                    head - distance,
                    2.4,
                    [0.3, 1.0, 0.5],
                    0.8 * flicker,
                );
            }
        }
    })
}

fn crystal_coverage(x: f32, y: f32, line_width: f32) -> f32 {
    let mut nearest = f32::MAX;
    for arm in 0..6 {
        let angle = arm as f32 * PI / 3.0;
        let (sin, cos) = angle.sin_cos();
        nearest = nearest.min(segment_distance((x, y), (0.0, 0.0), (cos, sin)));
        for along in [0.42, 0.7] {
            let base = (cos * along, sin * along);
            let length = 0.34 * (1.1 - along);
            for side in [-1.0f32, 1.0] {
                let branch = angle + side * PI / 3.0;
                let tip = (
                    base.0 + branch.cos() * length,
                    base.1 + branch.sin() * length,
                );
                nearest = nearest.min(segment_distance((x, y), base, tip));
            }
        }
    }
    smoothstep(line_width + 0.03, line_width - 0.02, nearest)
}

pub fn snowflakes(seed: u64) -> Draw {
    struct Flake {
        x: f32,
        y: f32,
        size: f32,
        sway: f32,
        offset: f32,
        spin: f32,
        cycles: f32,
    }
    let mut rng = Rng::new(seed);
    let flakes: Vec<Flake> = (0..38)
        .map(|_| Flake {
            x: rng.range(0.0, SPAN_X),
            y: rng.range(0.0, SPAN_Y),
            size: rng.range(14.0, 36.0),
            sway: rng.range(30.0, 80.0),
            offset: rng.unit(),
            spin: rng.whole(1, 2) * rng.pick(&[-1.0, 1.0]),
            cycles: rng.pick(&[1.0, 1.0, 2.0]),
        })
        .collect();
    Box::new(move |canvas, phase| {
        for flake in &flakes {
            let y = travel(flake.y, flake.cycles, phase, SPAN_Y) - MARGIN;
            let x = flake.x - MARGIN + flake.sway * (TAU * (phase + flake.offset)).sin();
            let (sin, cos) = (TAU * (flake.spin * phase + flake.offset)).sin_cos();
            let size = flake.size;
            let line_width = (1.3 / size).max(0.045);
            canvas.glow(x, y, size * 0.8, [0.7, 0.85, 1.0], 0.1);
            canvas.paint(x, y, size * 1.1, |dx, dy| {
                let (u, v) = ((dx * cos + dy * sin) / size, (-dx * sin + dy * cos) / size);
                scaled([0.9, 0.96, 1.0], crystal_coverage(u, v, line_width) * 0.9)
            });
        }
    })
}

pub fn gold_rain(seed: u64) -> Draw {
    struct Streak {
        x: f32,
        y: f32,
        length: f32,
        cycles: f32,
        brightness: f32,
        offset: f32,
    }
    let mut rng = Rng::new(seed);
    let streaks: Vec<Streak> = (0..230)
        .map(|_| Streak {
            x: rng.range(0.0, SPAN_X),
            y: rng.range(0.0, SPAN_Y),
            length: rng.range(30.0, 110.0),
            cycles: rng.whole(3, 6),
            brightness: rng.range(0.4, 1.0),
            offset: rng.unit(),
        })
        .collect();
    Box::new(move |canvas, phase| {
        for streak in &streaks {
            let y = travel(streak.y, streak.cycles, phase, SPAN_Y) - MARGIN;
            let x = streak.x - MARGIN;
            let glitter = 0.6 + 0.4 * pulse(5.0, phase, streak.offset);
            canvas.streak(
                (x, y - streak.length),
                (x, y),
                1.1,
                [1.0, 0.78, 0.3],
                0.55 * streak.brightness,
            );
            canvas.glow(
                x,
                y,
                2.2,
                [1.0, 0.92, 0.6],
                1.0 * streak.brightness * glitter,
            );
        }
    })
}

pub fn stage_lights(_seed: u64) -> Draw {
    const COLORS: [Color; 6] = [
        [1.0, 0.2, 0.7],
        [0.2, 0.9, 1.0],
        [1.0, 0.9, 0.3],
        [0.6, 0.4, 1.0],
        [0.3, 1.0, 0.5],
        [1.0, 0.4, 0.3],
    ];
    Box::new(move |canvas, phase| {
        let beams: Vec<(f32, f32, f32, Color)> = (0..6)
            .map(|index| {
                let source_x = (index as f32 + 0.5) / 6.0 * WIDTH as f32;
                let sweep =
                    0.38 * (TAU * ((1 + index % 2) as f32 * phase + index as f32 / 6.0)).sin();
                (
                    source_x,
                    sweep,
                    0.05 + 0.02 * pulse(2.0, phase, index as f32 * 0.17),
                    COLORS[index],
                )
            })
            .collect();
        canvas.field(160, 90, |u, v| {
            let (x, y) = (u * WIDTH as f32, v * HEIGHT as f32 + 40.0);
            let mut total = [0.0f32; 3];
            for &(source_x, sweep, width, color) in &beams {
                let angle = (x - source_x).atan2(y);
                let diff = (angle - sweep) / width;
                let reach = smoothstep(0.0, 120.0, y) * (-(y / 1300.0)).exp();
                let weight = (-diff * diff).exp() * reach * 0.6;
                for channel in 0..3 {
                    total[channel] += color[channel] * weight;
                }
            }
            total
        });
        for &(source_x, _, _, color) in &beams {
            canvas.glow(source_x, 14.0, 26.0, color, 0.8);
        }
    })
}

pub fn equalizer(seed: u64) -> Draw {
    const BARS: usize = 56;
    struct Bar {
        cycles: (f32, f32, f32),
        offsets: (f32, f32, f32),
    }
    let mut rng = Rng::new(seed);
    let bars: Vec<Bar> = (0..BARS)
        .map(|_| Bar {
            cycles: (rng.whole(2, 4), rng.whole(3, 7), rng.whole(5, 11)),
            offsets: (rng.unit(), rng.unit(), rng.unit()),
        })
        .collect();
    Box::new(move |canvas, phase| {
        let floor = HEIGHT as f32 - 18.0;
        for (index, bar) in bars.iter().enumerate() {
            let level = 0.5 * pulse(bar.cycles.0, phase, bar.offsets.0)
                + 0.3 * pulse(bar.cycles.1, phase, bar.offsets.1)
                + 0.2 * pulse(bar.cycles.2, phase, bar.offsets.2);
            let envelope = 0.55 + 0.45 * (PI * (index as f32 + 0.5) / BARS as f32).sin();
            let height = 50.0 + 330.0 * level * envelope;
            let left = 8.0 + index as f32 * 34.0;
            let base = mix(
                [0.1, 0.9, 1.0],
                [1.0, 0.2, 0.8],
                index as f32 / (BARS - 1) as f32,
            );
            canvas.rect((left, floor - height), (left + 24.0, floor), |x, y| {
                let into_bar = (x - left) / 24.0;
                let block = (floor - y) % 18.0 < 14.0;
                if !block || !(0.0..=1.0).contains(&into_bar) {
                    return [0.0; 3];
                }
                let lift = 0.65 + 0.5 * (1.0 - (floor - y) / height.max(1.0));
                scaled(base, 0.85 * lift)
            });
            canvas.glow(left + 12.0, floor - height, 22.0, base, 0.35);
        }
    })
}

pub fn rainbow(_seed: u64) -> Draw {
    Box::new(move |canvas, phase| {
        let breathe = 0.85 + 0.15 * pulse(1.0, phase, 0.0);
        canvas.field(160, 90, |u, v| {
            let (x, y) = (u * 16.0 / 9.0, v);
            let radius = (x - 8.0 / 9.0).hypot(y - 1.45);
            let t = (radius - 1.0) / 0.26;
            let band = smoothstep(-0.05, 0.12, t) * smoothstep(1.05, 0.88, t);
            let haze = (-((radius - 1.13) / 0.32).powi(2)).exp() * 0.06;
            let color = spectrum(1.0 - t);
            let weight = band * 0.62 * breathe;
            [
                color[0] * weight + haze,
                color[1] * weight + haze,
                color[2] * weight + haze,
            ]
        });
    })
}

pub fn balloons(seed: u64) -> Draw {
    struct Balloon {
        x: f32,
        y: f32,
        size: f32,
        color: Color,
        sway: f32,
        offset: f32,
        cycles: f32,
    }
    let palette = [
        [1.0, 0.25, 0.3],
        [1.0, 0.75, 0.15],
        [0.25, 0.7, 1.0],
        [0.4, 0.9, 0.4],
        [0.85, 0.4, 1.0],
        [1.0, 0.5, 0.75],
    ];
    let mut rng = Rng::new(seed);
    let balloons: Vec<Balloon> = (0..22)
        .map(|_| Balloon {
            x: rng.range(0.0, SPAN_X),
            y: rng.range(0.0, SPAN_Y),
            size: rng.range(28.0, 56.0),
            color: rng.pick(&palette),
            sway: rng.range(25.0, 70.0),
            offset: rng.unit(),
            cycles: 1.0,
        })
        .collect();
    Box::new(move |canvas, phase| {
        for balloon in &balloons {
            let y = travel(balloon.y, -balloon.cycles, phase, SPAN_Y) - MARGIN;
            let x = balloon.x - MARGIN + balloon.sway * (TAU * (phase + balloon.offset)).sin();
            let s = balloon.size;
            let mut string = vec![(x, y + 1.3 * s)];
            for step in 1..=6 {
                let drop = step as f32 * 22.0;
                string.push((
                    x + 8.0 * (TAU * (phase + balloon.offset) + step as f32 * 0.9).sin(),
                    y + 1.3 * s + drop,
                ));
            }
            for pair in string.windows(2) {
                canvas.segment(pair[0], pair[1], 0.8, [0.85, 0.85, 0.9], 0.45);
            }
            canvas.paint(x, y, s * 1.6, |dx, dy| {
                let (u, v) = (dx / s, dy / (1.2 * s));
                let d2 = u * u + v * v;
                let body = smoothstep(1.0, 0.9, d2);
                let shine = (-((u + 0.38).powi(2) + (v + 0.42).powi(2)) * 11.0).exp() * 0.7;
                let shade = 0.5 + 0.5 * (1.0 - d2).max(0.0);
                let knot_y = (dy - 1.2 * s) / (0.2 * s);
                let knot = if (0.0..1.0).contains(&knot_y) && dx.abs() < knot_y * 0.12 * s {
                    0.9
                } else {
                    0.0
                };
                let weight = body * (shade + shine) * 0.9 + knot * 0.8;
                scaled(balloon.color, weight)
            });
        }
    })
}

pub fn nebula(_seed: u64) -> Draw {
    Box::new(move |canvas, phase| {
        canvas.field(128, 72, |u, v| {
            let a = (TAU * (1.2 * u + 0.7 * v + phase)).sin();
            let b = (TAU * (2.3 * u - 1.1 * v - phase + 0.3)).sin();
            let c = (TAU * (0.7 * u + 1.9 * v + 2.0 * phase + 0.5)).sin();
            let d = (TAU * (3.7 * u + 0.4 * v - 2.0 * phase)).sin();
            let density = (0.5 + 0.5 * (a * 0.35 + b * 0.3 + c * 0.2 + d * 0.15)).powf(2.2);
            let hue = 0.5 + 0.5 * (TAU * (0.9 * u - 0.6 * v + phase + 0.2)).sin();
            let color = mix(
                mix([0.55, 0.15, 0.9], [0.15, 0.4, 1.0], hue),
                [1.0, 0.3, 0.65],
                (density * 1.4 - 0.5).clamp(0.0, 1.0),
            );
            scaled(color, density * 0.9)
        });
    })
}

pub fn synthwave(_seed: u64) -> Draw {
    Box::new(move |canvas, phase| {
        let horizon = HEIGHT as f32 * 0.6;
        let centre = WIDTH as f32 / 2.0;
        let sun_y = horizon - 110.0;
        let radius = 230.0;
        canvas.glow(centre, sun_y, 320.0, [1.0, 0.2, 0.6], 0.1);
        canvas.paint(centre, sun_y - radius / 2.0, radius + 4.0, |dx, dy| {
            let y = sun_y - radius / 2.0 + dy;
            let d = (dx).hypot(y - sun_y);
            if y > horizon - 4.0 || d > radius {
                return [0.0; 3];
            }
            let t = ((y - (sun_y - radius)) / (2.0 * radius)).clamp(0.0, 1.0);
            let slit_zone = ((y - sun_y) / radius).max(0.0);
            let stripe = ((y - phase * 28.0) / 28.0).rem_euclid(1.0);
            let visible = if slit_zone > 0.0 {
                smoothstep(0.12 + 0.5 * slit_zone, 0.16 + 0.5 * slit_zone, stripe)
            } else {
                1.0
            };
            scaled(
                mix([1.0, 0.9, 0.25], [1.0, 0.15, 0.6], t),
                smoothstep(radius, radius - 2.0, d) * visible * 0.95,
            )
        });
        let magenta = [1.0, 0.25, 0.85];
        canvas.rect(
            (0.0, horizon - 4.0),
            (WIDTH as f32, horizon + 4.0),
            |_, y| scaled(magenta, (1.0 - (y - horizon).abs() / 4.0).max(0.0) * 0.9),
        );
        let ground = HEIGHT as f32 - horizon;
        for index in 0..14 {
            let t = (index as f32 + phase) / 14.0;
            let y = horizon + ground * t * t;
            canvas.segment(
                (0.0, y),
                (WIDTH as f32, y),
                0.9 + 1.8 * t,
                magenta,
                0.25 + 0.75 * t,
            );
        }
        for index in -16..=16 {
            let bottom = centre + index as f32 * 190.0;
            canvas.line(
                (centre, horizon),
                (bottom, HEIGHT as f32 + 40.0),
                1.4,
                [0.35, 0.85, 1.0],
                0.9,
                0.9,
            );
        }
    })
}

pub fn light_streaks(seed: u64) -> Draw {
    struct Beam {
        x: f32,
        y: f32,
        half_length: f32,
        width: f32,
        color: Color,
        brightness: f32,
        direction: f32,
    }
    let palette = [
        [1.0, 0.75, 0.4],
        [0.5, 0.75, 1.0],
        [1.0, 0.5, 0.7],
        [0.9, 0.95, 1.0],
    ];
    let mut rng = Rng::new(seed);
    let beams: Vec<Beam> = (0..18)
        .map(|_| Beam {
            x: rng.range(0.0, SPAN_X),
            y: rng.range(0.05, 0.95) * HEIGHT as f32,
            half_length: rng.range(150.0, 450.0),
            width: rng.range(2.0, 7.0),
            color: rng.pick(&palette),
            brightness: rng.range(0.25, 0.65),
            direction: rng.pick(&[-1.0, 1.0]),
        })
        .collect();
    Box::new(move |canvas, phase| {
        for beam in &beams {
            let x = travel(beam.x, beam.direction, phase, SPAN_X) - MARGIN;
            let reach = beam.half_length * 1.8;
            canvas.rect(
                (x - reach, beam.y - beam.width * 3.0),
                (x + reach, beam.y + beam.width * 3.0),
                |px, py| {
                    let across = (-((py - beam.y) / beam.width).powi(2)).exp();
                    let along = (-((px - x).abs() / beam.half_length).powf(1.5)).exp();
                    scaled(beam.color, across * along * beam.brightness)
                },
            );
        }
    })
}
