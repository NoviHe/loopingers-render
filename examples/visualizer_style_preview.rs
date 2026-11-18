//! Export the native effect geometry as PNGs without opening a desktop window.
#[path = "../src/app/liquid_ring_motion.rs"]
mod liquid_ring_motion;
#[path = "../src/app/spectrum_styles.rs"]
mod spectrum_styles;

use eframe::egui::{self, Pos2, Rect};
use loopkaraoke::{
    config::{SpectrumKind, SpectrumOverlaySettings},
    spectrum::BAND_COUNT,
};

fn edge(a: Pos2, b: Pos2, p: Pos2) -> f32 {
    (p.x - a.x) * (b.y - a.y) - (p.y - a.y) * (b.x - a.x)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!(
                "loopkaraoke-style-previews-{}",
                uuid::Uuid::new_v4()
            ))
        });
    std::fs::create_dir_all(&output)?;
    let levels: [f32; BAND_COUNT] = std::array::from_fn(|index| {
        let a = (index as f32 - 8.0) / 5.0;
        let b = (index as f32 - 25.0) / 7.0;
        (0.1 + 0.75 * (-a * a).exp() + 0.5 * (-b * b).exp()).min(1.0)
    });
    let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(640.0, 360.0));
    for kind in SpectrumKind::ALL {
        let ctx = egui::Context::default();
        let mut settings = SpectrumOverlaySettings::default();
        settings.select_kind(kind);
        let result = ctx.run(
            egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ctx| {
                let painter = ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    egui::Id::new("effect"),
                ));
                let area = Rect::from_center_size(screen.center(), egui::vec2(480.0, 280.0));
                if kind == SpectrumKind::RingGlow {
                    let mut liquid = liquid_ring_motion::LiquidRingMotion::default();
                    liquid.advance(
                        &levels,
                        settings.sensitivity_pct,
                        settings.smoothing_pct,
                        0.016,
                        true,
                    );
                    spectrum_styles::paint_liquid_fallback(
                        &painter,
                        area,
                        &liquid.profile,
                        &settings,
                        1.0,
                    );
                } else {
                    spectrum_styles::paint_spectrum(&painter, area, &levels, &settings, 1.0);
                }
            },
        );
        let mut pixels = image::RgbaImage::from_pixel(640, 360, image::Rgba([8, 12, 20, 255]));
        for primitive in ctx.tessellate(result.shapes, 1.0) {
            let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive else {
                continue;
            };
            let clip = primitive.clip_rect.intersect(screen);
            for triangle in mesh.indices.chunks_exact(3) {
                let vertices = [triangle[0], triangle[1], triangle[2]]
                    .map(|index| mesh.vertices[index as usize]);
                let [a, b, c] = vertices.map(|vertex| vertex.pos);
                let denominator = edge(a, b, c);
                if denominator.abs() < 0.0001 {
                    continue;
                }
                let min_x = a.x.min(b.x).min(c.x).max(clip.left()).max(0.0).floor() as u32;
                let max_x = a.x.max(b.x).max(c.x).min(clip.right()).min(640.0).ceil() as u32;
                let min_y = a.y.min(b.y).min(c.y).max(clip.top()).max(0.0).floor() as u32;
                let max_y = a.y.max(b.y).max(c.y).min(clip.bottom()).min(360.0).ceil() as u32;
                for y in min_y..max_y {
                    for x in min_x..max_x {
                        let p = egui::pos2(x as f32 + 0.5, y as f32 + 0.5);
                        let weights = [
                            edge(b, c, p) / denominator,
                            edge(c, a, p) / denominator,
                            edge(a, b, p) / denominator,
                        ];
                        if weights.iter().any(|weight| *weight < -0.00001) {
                            continue;
                        }
                        let channels: [f32; 4] = std::array::from_fn(|channel| {
                            vertices
                                .iter()
                                .zip(weights)
                                .map(|(vertex, weight)| {
                                    vertex.color.to_array()[channel] as f32 * weight
                                })
                                .sum()
                        });
                        let alpha = (channels[3] / 255.0).clamp(0.0, 1.0);
                        let pixel = pixels.get_pixel_mut(x, y);
                        for channel in 0..3 {
                            pixel[channel] =
                                (channels[channel] + pixel[channel] as f32 * (1.0 - alpha))
                                    .clamp(0.0, 255.0) as u8;
                        }
                    }
                }
            }
        }
        let path = output.join(format!("{kind:?}.png"));
        pixels.save(&path)?;
        println!("{}", path.display());
    }
    Ok(())
}
