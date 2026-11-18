//! Compile the real GL shader and export offscreen previews without showing a window.
#[path = "../src/app/liquid_ring_gpu.rs"]
mod liquid_ring_gpu;
#[path = "../src/app/liquid_ring_motion.rs"]
mod liquid_ring_motion;

use std::{path::Path, sync::Arc};

use eframe::{
    egui::{self, Pos2, Rect},
    egui_glow,
    glow::{self, HasContext},
};
use liquid_ring_gpu::LiquidRingRenderer;
use loopkaraoke::{
    config::{SpectrumKind, SpectrumOverlaySettings},
    spectrum::BAND_COUNT,
};

const WIDTH: u32 = 640;
const HEIGHT: u32 = 360;

fn check_scroll_clipping(
    gl: &glow::Context,
    renderer: &LiquidRingRenderer,
    painter: &mut egui_glow::Painter,
) {
    let mut settings = SpectrumOverlaySettings::default();
    settings.select_kind(SpectrumKind::RingGlow);
    for scale in [1.0, 1.5, 2.0] {
        let ctx = egui::Context::default();
        let screen =
            Rect::from_min_size(Pos2::ZERO, egui::vec2(WIDTH as f32, HEIGHT as f32) / scale);
        let mut reference = None;
        for (dx, dy) in [(0, 0), (0, -120), (0, 120), (-280, 0), (280, 0)] {
            let canvas = screen.translate(egui::vec2(dx as f32, dy as f32) / scale);
            let area = Rect::from_center_size(canvas.center(), egui::vec2(300.0, 300.0) / scale);
            let frame = ctx.run(
                egui::RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                },
                |ctx| {
                    renderer.paint(
                        &ctx.layer_painter(egui::LayerId::background()),
                        canvas,
                        area,
                        &[0.0; 128],
                        &settings,
                        0.0,
                    );
                },
            );
            painter.clear([WIDTH, HEIGHT], [0.03, 0.045, 0.065, 1.0]);
            painter.paint_and_update_textures(
                [WIDTH, HEIGHT],
                scale,
                &ctx.tessellate(frame.shapes, scale),
                &frame.textures_delta,
            );
            let mut pixels = vec![0; (WIDTH * HEIGHT * 4) as usize];
            unsafe {
                gl.read_pixels(
                    0,
                    0,
                    WIDTH as i32,
                    HEIGHT as i32,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelPackData::Slice(Some(&mut pixels)),
                );
                assert_eq!(gl.get_error(), glow::NO_ERROR);
            }
            let mut image = image::RgbaImage::from_raw(WIDTH, HEIGHT, pixels).unwrap();
            image::imageops::flip_vertical_in_place(&mut image);
            let Some(original): Option<&image::RgbaImage> = reference.as_ref() else {
                reference = Some(image);
                continue;
            };
            let mut mismatches = 0;
            let mut compared = 0;
            for y in 1..HEIGHT - 1 {
                for x in 1..WIDTH - 1 {
                    let source_x = x as i32 - dx;
                    let source_y = y as i32 - dy;
                    if source_x < 1
                        || source_x >= WIDTH as i32 - 1
                        || source_y < 1
                        || source_y >= HEIGHT as i32 - 1
                    {
                        continue;
                    }
                    let expected = original.get_pixel(source_x as u32, source_y as u32);
                    let actual = image.get_pixel(x, y);
                    mismatches += usize::from(
                        (0..3).any(|channel| expected[channel].abs_diff(actual[channel]) > 4),
                    );
                    compared += 1;
                }
            }
            assert!(mismatches < compared / 200, "scroll must clip, not squash the ring: dpi={scale}, shift=({dx},{dy}), {mismatches}/{compared} mismatches");
        }
        println!("Scroll clipping preserves ring geometry at {scale}x DPI");
    }
}

fn render_previews(gl: Arc<glow::Context>, output: &Path) -> Result<(), String> {
    let renderer = LiquidRingRenderer::new(Arc::clone(&gl))?;
    let mut painter = egui_glow::Painter::new(Arc::clone(&gl), "", None, false)
        .map_err(|error| error.to_string())?;
    let (framebuffer, texture) = unsafe {
        let texture = gl.create_texture()?;
        gl.bind_texture(glow::TEXTURE_2D, Some(texture));
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA8 as i32,
            WIDTH as i32,
            HEIGHT as i32,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(None),
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::LINEAR as i32,
        );
        let framebuffer = gl.create_framebuffer()?;
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(texture),
            0,
        );
        assert_eq!(
            gl.check_framebuffer_status(glow::FRAMEBUFFER),
            glow::FRAMEBUFFER_COMPLETE
        );
        (framebuffer, texture)
    };
    let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(WIDTH as f32, HEIGHT as f32));
    let area = Rect::from_center_size(screen.center(), egui::vec2(300.0, 300.0));
    let ctx = egui::Context::default();
    let mut settings = SpectrumOverlaySettings::default();
    settings.select_kind(SpectrumKind::RingGlow);
    for (name, active, opacity) in [
        ("liquid-ring", true, 100),
        ("silent-ring", false, 100),
        ("transparent-ring", true, 0),
    ] {
        settings.opacity_pct = opacity;
        // The same motion model as the app, driven by a deterministic FFT fixture.
        let levels: [f32; BAND_COUNT] = std::array::from_fn(|index| {
            if !active {
                return 0.0;
            }
            let a = (index as f32 - 8.0) / 5.0;
            let b = (index as f32 - 25.0) / 7.0;
            (0.1 + 0.75 * (-a * a).exp() + 0.5 * (-b * b).exp()).min(1.0)
        });
        let mut motion = liquid_ring_motion::LiquidRingMotion::default();
        motion.advance(
            &levels,
            settings.sensitivity_pct,
            settings.smoothing_pct,
            1.0 / 60.0,
            true,
        );
        for _ in 0..60 {
            motion.advance(
                &levels,
                settings.sensitivity_pct,
                settings.smoothing_pct,
                1.0 / 60.0,
                false,
            );
        }
        let profile = motion.profile;
        let frame = ctx.run(
            egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ctx| {
                renderer.paint(
                    &ctx.layer_painter(egui::LayerId::background()),
                    screen,
                    area,
                    &profile,
                    &settings,
                    0.0,
                );
            },
        );
        painter.clear([WIDTH, HEIGHT], [0.03, 0.045, 0.065, 1.0]);
        painter.paint_and_update_textures(
            [WIDTH, HEIGHT],
            1.0,
            &ctx.tessellate(frame.shapes, 1.0),
            &frame.textures_delta,
        );
        let mut pixels = vec![0; (WIDTH * HEIGHT * 4) as usize];
        unsafe {
            gl.read_pixels(
                0,
                0,
                WIDTH as i32,
                HEIGHT as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut pixels)),
            );
            assert_eq!(
                gl.get_error(),
                glow::NO_ERROR,
                "GL preview must have no error"
            );
        }
        let bright_pixels = pixels
            .chunks_exact(4)
            .filter(|pixel| pixel[0] > 150)
            .count();
        if opacity == 0 {
            assert_eq!(
                bright_pixels, 0,
                "zero opacity must leave the background untouched"
            );
        } else {
            assert!(
                bright_pixels > 200,
                "actual shader must draw a luminous ring"
            );
        }
        let center_index = ((HEIGHT / 2 * WIDTH + WIDTH / 2) * 4) as usize;
        assert!(
            pixels[center_index] < 30,
            "ring center must remain transparent"
        );
        let mut image = image::RgbaImage::from_raw(WIDTH, HEIGHT, pixels).unwrap();
        image::imageops::flip_vertical_in_place(&mut image);
        let path = output.join(format!("{name}.png"));
        image.save(&path).map_err(|error| error.to_string())?;
        println!("{} ({bright_pixels} bright pixels)", path.display());
    }
    check_scroll_clipping(&gl, &renderer, &mut painter);
    renderer.destroy(&gl);
    renderer.destroy(&gl); // Cleanup must be idempotent.
    painter.destroy();
    unsafe {
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        gl.delete_framebuffer(framebuffer);
        gl.delete_texture(texture);
    }
    Ok(())
}

struct Finished;
impl eframe::App for Finished {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

fn main() -> eframe::Result {
    let output = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!("loopkaraoke-liquid-gpu-{}", uuid::Uuid::new_v4()))
        });
    std::fs::create_dir_all(&output).expect("preview directory can be created");
    eframe::run_native(
        "Liquid Ring GPU validation",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Glow,
            viewport: egui::ViewportBuilder::default()
                .with_visible(false)
                .with_inner_size([640.0, 360.0]),
            ..Default::default()
        },
        Box::new(move |cc| {
            render_previews(cc.gl.clone().expect("GL backend selected"), &output)
                .map_err(std::io::Error::other)?;
            Ok(Box::new(Finished))
        }),
    )
}
