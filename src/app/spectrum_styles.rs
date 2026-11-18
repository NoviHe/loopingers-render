//! Native, audio-driven geometry for the supplied halo visual references.
use std::f32::consts::TAU;

use eframe::egui::{self, Color32, Painter, Pos2, Rect, Shape, Stroke};
use loopkaraoke::{
    config::{SpectrumKind, SpectrumOverlaySettings},
    spectrum::BAND_COUNT,
};

const RING_POINTS: usize = 384;
const HALO_SPIKES: usize = 96;
const GRADIENT_SECTIONS: usize = 12;

/// Keep the liquid motion available if the custom GPU shader cannot initialize.
pub(super) fn paint_liquid_fallback(
    painter: &Painter,
    area: Rect,
    profile: &[f32; 128],
    settings: &SpectrumOverlaySettings,
    time: f64,
) {
    let radius = area.width().min(area.height()) * 0.5;
    if radius <= 0.0 || !settings.enabled {
        return;
    }
    let rotation = if time.is_finite() {
        (time * settings.rotation_speed.min(100) as f64 / 100.0 * 0.4).rem_euclid(TAU as f64) as f32
    } else {
        0.0
    };
    let mut points: Vec<_> = (0..RING_POINTS)
        .map(|i| {
            let position = i as f32 / RING_POINTS as f32 * profile.len() as f32;
            let low = position.floor() as usize % profile.len();
            let high = (low + 1) % profile.len();
            let offset = profile[low] * (1.0 - position.fract()) + profile[high] * position.fract();
            circle_point(
                area.center(),
                radius * (0.72 + offset),
                i as f32 / RING_POINTS as f32 * TAU + rotation,
            )
        })
        .collect();
    points.push(points[0]);
    let color = Color32::from_rgb(settings.color[0], settings.color[1], settings.color[2]);
    paint_ring_glow(
        painter,
        area.center(),
        radius,
        &points,
        color,
        settings.thickness_px.clamp(1, 16) as f32,
        settings.glow_pct.min(100) as f32 / 100.0,
        settings.opacity_pct.min(100) as f32 / 100.0,
    );
}

pub(super) fn paint_spectrum(
    painter: &Painter,
    area: Rect,
    levels: &[f32; BAND_COUNT],
    settings: &SpectrumOverlaySettings,
    time: f64,
) {
    if !settings.enabled || area.width() <= 0.0 || area.height() <= 0.0 {
        return;
    }
    let sensitivity = settings.sensitivity_pct.clamp(25, 300) as f32 / 100.0;
    let levels = levels.map(|level| {
        if level.is_finite() {
            (level * sensitivity).clamp(0.0, 1.0)
        } else {
            0.0
        }
    });
    let color = Color32::from_rgb(settings.color[0], settings.color[1], settings.color[2]);
    let opacity = settings.opacity_pct.min(100) as f32 / 100.0;
    if opacity <= 0.0 {
        return;
    }
    if settings.kind == SpectrumKind::Bars {
        paint_bars(painter, area, &levels, color.gamma_multiply(opacity));
        return;
    }
    let rotation = if time.is_finite() {
        let rotations_per_second =
            settings.rotation_speed.min(100) as f64 / 100.0 * 0.4 / TAU as f64;
        (time * rotations_per_second).rem_euclid(1.0) as f32 * TAU
    } else {
        0.0
    };
    let thickness = (settings.thickness_px.clamp(1, 16) as f32)
        .min(area.width().min(area.height()) * 0.05)
        .max(0.25);
    let glow = settings.glow_pct.min(100) as f32 / 100.0;
    let radius = available_radius(area, thickness);
    if settings.kind == SpectrumKind::HaloSpectrum {
        for index in 0..HALO_SPIKES {
            let turn = index as f32 / HALO_SPIKES as f32;
            let angle = turn * TAU + rotation;
            let value = mirrored_level(&levels, turn, 1);
            let start = circle_point(area.center(), radius * 0.69, angle);
            let end = circle_point(area.center(), radius * (0.70 + value * 0.26), angle);
            paint_glowing_path(painter, &[start, end], color, thickness, glow, opacity);
        }
        return;
    }

    let points = ring_points(settings.kind, area.center(), radius, &levels, rotation);
    if settings.kind == SpectrumKind::HaloGeometry {
        let violet = Color32::from_rgb(
            settings.secondary_color[0],
            settings.secondary_color[1],
            settings.secondary_color[2],
        );
        let section_len = RING_POINTS / GRADIENT_SECTIONS;
        for section in 0..GRADIENT_SECTIONS {
            let start = section * section_len;
            let end = start + section_len;
            let blend = (section as f32 / GRADIENT_SECTIONS as f32 * 2.0 - 1.0).abs();
            let section_color = color.lerp_to_gamma(violet, blend * 0.8);
            paint_glowing_path(
                painter,
                &points[start..=end],
                section_color,
                thickness,
                glow,
                opacity,
            );
        }
    } else if settings.kind == SpectrumKind::RingGlow {
        paint_ring_glow(
            painter,
            area.center(),
            radius,
            &points,
            color,
            thickness,
            glow,
            opacity,
        );
    } else {
        paint_glowing_path(painter, &points, color, thickness, glow, opacity);
    }
}

fn paint_bars(painter: &Painter, area: Rect, levels: &[f32; BAND_COUNT], color: Color32) {
    let pitch = area.width() / BAND_COUNT as f32;
    let gap = (pitch * 0.25).clamp(0.5, 3.0).min(pitch * 0.5);
    for (index, level) in levels.iter().enumerate() {
        let height = (area.height() * level).max(1.0).min(area.height());
        painter.rect_filled(
            Rect::from_min_max(
                egui::pos2(
                    area.left() + index as f32 * pitch + gap * 0.5,
                    area.bottom() - height,
                ),
                egui::pos2(
                    area.left() + (index + 1) as f32 * pitch - gap * 0.5,
                    area.bottom(),
                ),
            ),
            1,
            color,
        );
    }
}

fn available_radius(area: Rect, thickness: f32) -> f32 {
    // Leave room for glow/core thickness rather than cropping the outermost peaks.
    let half = area.width().min(area.height()) * 0.5;
    (half - (thickness * 3.0).min(half * 0.13)).max(0.0)
}

fn circle_point(center: Pos2, radius: f32, angle: f32) -> Pos2 {
    let (sin, cos) = angle.sin_cos();
    center + egui::vec2(cos * radius, sin * radius)
}

fn sample_band(levels: &[f32; BAND_COUNT], position: f32) -> f32 {
    let position = position.clamp(0.0, 1.0) * (BAND_COUNT - 1) as f32;
    let first = position.floor() as usize;
    let second = (first + 1).min(BAND_COUNT - 1);
    let blend = position.fract();
    levels[first] * (1.0 - blend) + levels[second] * blend
}

fn mirrored_level(levels: &[f32; BAND_COUNT], turn: f32, spread: usize) -> f32 {
    let band_position = 1.0 - ((turn * 2.0).rem_euclid(1.0) * 2.0 - 1.0).abs();
    let mut sum = 0.0;
    let mut weights = 0.0;
    for offset in -(spread as i32)..=spread as i32 {
        let weight = (spread + 1 - offset.unsigned_abs() as usize) as f32;
        sum += sample_band(levels, band_position + offset as f32 / BAND_COUNT as f32) * weight;
        weights += weight;
    }
    sum / weights
}

fn ring_points(
    kind: SpectrumKind,
    center: Pos2,
    radius: f32,
    levels: &[f32; BAND_COUNT],
    rotation: f32,
) -> Vec<Pos2> {
    let bass = levels[..8].iter().sum::<f32>() / 8.0;
    let mut points = Vec::with_capacity(RING_POINTS + 1);
    for index in 0..RING_POINTS {
        let turn = index as f32 / RING_POINTS as f32;
        let value = mirrored_level(
            levels,
            turn,
            if kind == SpectrumKind::RingGlow { 4 } else { 1 },
        );
        let factor = if kind == SpectrumKind::RingGlow {
            // A round inner rim with short outward lobes, rather than a wavy polygon.
            // The carrier only distributes audio energy; silence remains a circle.
            let angle = turn * TAU;
            let lobes = (0.5 + 0.5 * (angle * 12.0).cos()).powf(3.0);
            let lower_and_sides = 0.25 + 0.75 * ((angle.sin() + 1.0) * 0.5);
            0.75 + bass * 0.025 + (value * 0.75 + bass * 0.25) * 0.22 * lobes * lower_and_sides
        } else {
            0.73 + value * 0.23
        };
        points.push(circle_point(center, radius * factor, turn * TAU + rotation));
    }
    if let Some(first) = points.first().copied() {
        points.push(first);
    }
    points
}

#[allow(clippy::too_many_arguments)]
fn paint_ring_glow(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    points: &[Pos2],
    color: Color32,
    thickness: f32,
    glow: f32,
    opacity: f32,
) {
    // Radius-scaled layers keep the soft halo visible at small preview sizes.
    if glow > 0.0 {
        for (spread, alpha) in [
            (0.56, 0.012),
            (0.46, 0.018),
            (0.37, 0.025),
            (0.29, 0.035),
            (0.22, 0.05),
            (0.16, 0.065),
            (0.11, 0.09),
            (0.07, 0.14),
            (0.035, 0.2),
        ] {
            painter.add(Shape::line(
                points.to_vec(),
                Stroke::new(
                    thickness + radius * spread * glow,
                    color.gamma_multiply(alpha * glow * opacity),
                ),
            ));
        }
    }
    // Fill the reactive protrusions back to a circular rim for the reference's
    // solid luminous scallops, leaving the center transparent.
    let inner_radius = points
        .iter()
        .map(|p| p.distance(center))
        .fold(f32::INFINITY, f32::min);
    let luminous = color
        .lerp_to_gamma(Color32::WHITE, 0.82)
        .gamma_multiply(opacity);
    let mut mesh = egui::epaint::Mesh::default();
    for point in points {
        let inner = center + (*point - center).normalized() * inner_radius;
        mesh.colored_vertex(inner, luminous);
        mesh.colored_vertex(*point, luminous);
    }
    for index in 0..points.len() - 1 {
        let base = index as u32 * 2;
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base + 1, base + 3, base + 2);
    }
    painter.add(Shape::mesh(mesh));
    paint_glowing_path(
        painter,
        points,
        color.lerp_to_gamma(Color32::WHITE, 0.55),
        thickness,
        glow,
        opacity,
    );
    painter.circle_stroke(
        center,
        inner_radius,
        Stroke::new(thickness.max(radius * 0.018), luminous),
    );
}

fn paint_glowing_path(
    painter: &Painter,
    points: &[Pos2],
    color: Color32,
    thickness: f32,
    glow: f32,
    opacity: f32,
) {
    if glow > 0.0 {
        for (spread, alpha) in [(6.0, 0.045), (3.5, 0.09), (1.8, 0.18)] {
            painter.add(Shape::line(
                points.to_vec(),
                Stroke::new(
                    thickness * (1.0 + spread * glow),
                    color.gamma_multiply(alpha * glow * opacity),
                ),
            ));
        }
    }
    painter.add(Shape::line(
        points.to_vec(),
        Stroke::new(thickness, color.gamma_multiply(opacity)),
    ));
    if glow > 0.0 {
        painter.add(Shape::line(
            points.to_vec(),
            Stroke::new(
                (thickness * 0.38).max(0.35),
                color
                    .lerp_to_gamma(Color32::WHITE, 0.72)
                    .gamma_multiply(opacity),
            ),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reactive_rings_close_and_stay_inside_their_available_area() {
        let area = Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(440.0, 280.0));
        for kind in [SpectrumKind::HaloGeometry, SpectrumKind::RingGlow] {
            let points = ring_points(
                kind,
                area.center(),
                available_radius(area, 3.0),
                &[1.0; BAND_COUNT],
                0.7,
            );
            assert_eq!(points.len(), RING_POINTS + 1);
            assert_eq!(points.first(), points.last());
            assert!(points
                .iter()
                .all(|point| point.x.is_finite() && point.y.is_finite() && area.contains(*point)));
        }
    }

    #[test]
    fn silence_produces_a_circle_while_real_audio_changes_its_radius() {
        let center = egui::pos2(200.0, 200.0);
        for kind in [SpectrumKind::HaloGeometry, SpectrumKind::RingGlow] {
            let quiet = ring_points(kind, center, 100.0, &[0.0; BAND_COUNT], 0.0);
            let rotated = ring_points(kind, center, 100.0, &[0.0; BAND_COUNT], 1.0);
            let loud = ring_points(kind, center, 100.0, &[1.0; BAND_COUNT], 0.0);
            for ((quiet, rotated), loud) in quiet.iter().zip(&rotated).zip(&loud) {
                assert!((quiet.distance(center) - rotated.distance(center)).abs() < 0.001);
                assert!(loud.distance(center) > quiet.distance(center));
            }
        }
    }

    #[test]
    fn ring_glow_has_a_round_rim_and_audio_driven_outward_lobes() {
        let center = Pos2::ZERO;
        let points = ring_points(
            SpectrumKind::RingGlow,
            center,
            100.0,
            &[1.0; BAND_COUNT],
            0.0,
        );
        let rim = points
            .iter()
            .map(|p| p.distance(center))
            .fold(f32::INFINITY, f32::min);
        let peak = points
            .iter()
            .map(|p| p.distance(center))
            .fold(0.0_f32, f32::max);
        assert!((rim - 77.5).abs() < 0.01);
        assert!(peak - rim > 20.0);
        assert!(
            points[RING_POINTS / 4].distance(center)
                > points[RING_POINTS * 3 / 4].distance(center) + 10.0
        );
    }

    #[test]
    fn mirrored_frequency_values_match_on_opposite_sides() {
        let levels = std::array::from_fn(|index| index as f32 / (BAND_COUNT - 1) as f32);
        for turn in [0.0, 0.07, 0.2, 0.34] {
            assert!(
                (mirrored_level(&levels, turn, 1) - mirrored_level(&levels, turn + 0.5, 1)).abs()
                    < 0.0001
            );
        }
    }
}
