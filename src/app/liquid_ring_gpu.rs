//! GPU rendering for the liquid Ring Glow preview, integrated into egui's GL pass.
use std::sync::{Arc, Mutex};

use eframe::{
    egui::{self, Painter, Rect},
    egui_glow,
    glow::{self, HasContext},
};
use loopkaraoke::config::SpectrumOverlaySettings;

const VERTEX_SHADER: &str = r#"
out vec2 v_uv;
void main() {
    vec2 position = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
    v_uv = vec2(position.x, 1.0 - position.y);
    gl_Position = vec4(position * 2.0 - 1.0, 0.0, 1.0);
}
"#;
const FRAGMENT_SHADER: &str = include_str!("shaders/liquid_ring.frag");

struct Resources {
    program: glow::Program,
    vao: glow::VertexArray,
    canvas_size: Option<glow::UniformLocation>,
    canvas_offset: Option<glow::UniformLocation>,
    center: Option<glow::UniformLocation>,
    half_size: Option<glow::UniformLocation>,
    profile: Option<glow::UniformLocation>,
    color: Option<glow::UniformLocation>,
    opacity: Option<glow::UniformLocation>,
    glow: Option<glow::UniformLocation>,
    thickness: Option<glow::UniformLocation>,
    rotation: Option<glow::UniformLocation>,
}

/// Shared handles are destroyed explicitly while eframe's GL context is current.
/// A callback already queued for a destroyed renderer safely becomes a no-op.
pub struct LiquidRingRenderer {
    resources: Arc<Mutex<Option<Resources>>>,
}

impl LiquidRingRenderer {
    pub fn new(gl: Arc<glow::Context>) -> Result<Self, String> {
        // eframe's native GL renderer provides GL 3.2+ (or GLES 3). Both support
        // gl_VertexID and dynamically indexed fragment-uniform arrays.
        let version = if gl.version().is_embedded {
            "#version 300 es\nprecision highp float;\nprecision highp int;\n"
        } else {
            "#version 140\n"
        };
        unsafe {
            let program = gl.create_program()?;
            let mut shaders = Vec::with_capacity(2);
            for (kind, source) in [
                (glow::VERTEX_SHADER, VERTEX_SHADER),
                (glow::FRAGMENT_SHADER, FRAGMENT_SHADER),
            ] {
                let shader = match gl.create_shader(kind) {
                    Ok(shader) => shader,
                    Err(error) => {
                        for shader in shaders {
                            gl.delete_shader(shader);
                        }
                        gl.delete_program(program);
                        return Err(error);
                    }
                };
                gl.shader_source(shader, &format!("{version}{source}"));
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    let error = gl.get_shader_info_log(shader);
                    gl.delete_shader(shader);
                    for shader in shaders {
                        gl.delete_shader(shader);
                    }
                    gl.delete_program(program);
                    return Err(format!("Shader Ring Glow: {error}"));
                }
                gl.attach_shader(program, shader);
                shaders.push(shader);
            }
            gl.link_program(program);
            for shader in shaders {
                gl.detach_shader(program, shader);
                gl.delete_shader(shader);
            }
            if !gl.get_program_link_status(program) {
                let error = gl.get_program_info_log(program);
                gl.delete_program(program);
                return Err(format!("Program Ring Glow: {error}"));
            }
            let vao = match gl.create_vertex_array() {
                Ok(vao) => vao,
                Err(error) => {
                    gl.delete_program(program);
                    return Err(error);
                }
            };
            let resources = Resources {
                program,
                vao,
                canvas_size: gl.get_uniform_location(program, "u_canvas_size"),
                canvas_offset: gl.get_uniform_location(program, "u_canvas_offset"),
                center: gl.get_uniform_location(program, "u_center"),
                half_size: gl.get_uniform_location(program, "u_half_size"),
                profile: gl.get_uniform_location(program, "u_profile[0]"),
                color: gl.get_uniform_location(program, "u_color"),
                opacity: gl.get_uniform_location(program, "u_opacity"),
                glow: gl.get_uniform_location(program, "u_glow"),
                thickness: gl.get_uniform_location(program, "u_thickness"),
                rotation: gl.get_uniform_location(program, "u_rotation"),
            };
            Ok(Self {
                resources: Arc::new(Mutex::new(Some(resources))),
            })
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn paint(
        &self,
        painter: &Painter,
        canvas: Rect,
        area: Rect,
        profile: &[f32; 128],
        settings: &SpectrumOverlaySettings,
        time: f64,
    ) {
        if !settings.enabled || !canvas.is_positive() || !area.is_positive() {
            return;
        }
        let resources = Arc::clone(&self.resources);
        let profile = profile.map(|value| {
            if value.is_finite() {
                value.clamp(-0.025, 0.19)
            } else {
                0.0
            }
        });
        let center = area.center() - canvas.min;
        let half_size = area.width().min(area.height()) * 0.5;
        let color = settings.color.map(|channel| channel as f32 / 255.0);
        let opacity = settings.opacity_pct.min(100) as f32 / 100.0;
        let glow = settings.glow_pct.min(100) as f32 / 100.0;
        let thickness = settings.thickness_px.clamp(1, 16) as f32;
        let rotation = if time.is_finite() {
            (time * settings.rotation_speed.min(100) as f64 / 100.0 * 0.4)
                .rem_euclid(std::f64::consts::TAU) as f32
        } else {
            0.0
        };
        painter.add(egui::PaintCallback {
            rect: canvas,
            callback: Arc::new(egui_glow::CallbackFn::new(move |info, painter| {
                let Ok(guard) = resources.lock() else {
                    return;
                };
                let Some(resources) = guard.as_ref() else {
                    return;
                };
                let gl = painter.gl();
                // egui clamps callback viewports to the physical screen. UVs
                // span that smaller viewport, not the original scrolled canvas.
                let viewport = info.viewport_in_pixels();
                let scale = info.pixels_per_point;
                let size = egui::vec2(viewport.width_px as f32, viewport.height_px as f32) / scale;
                let offset = egui::pos2(viewport.left_px as f32, viewport.top_px as f32) / scale
                    - canvas.min;
                if size.x <= 0.0 || size.y <= 0.0 {
                    return;
                }
                unsafe {
                    gl.use_program(Some(resources.program));
                    gl.bind_vertex_array(Some(resources.vao));
                    gl.enable(glow::BLEND);
                    gl.blend_equation(glow::FUNC_ADD);
                    gl.blend_func(glow::ONE, glow::ONE_MINUS_SRC_ALPHA);
                    gl.disable(glow::DEPTH_TEST);
                    gl.disable(glow::CULL_FACE);
                    // Keep geometry anchored to the original canvas; scissoring
                    // only hides pixels and never rescales the visible portion.
                    gl.uniform_2_f32(resources.canvas_size.as_ref(), size.x, size.y);
                    gl.uniform_2_f32(resources.canvas_offset.as_ref(), offset.x, offset.y);
                    gl.uniform_2_f32(resources.center.as_ref(), center.x, center.y);
                    gl.uniform_1_f32(resources.half_size.as_ref(), half_size);
                    gl.uniform_1_f32_slice(resources.profile.as_ref(), &profile);
                    gl.uniform_3_f32(resources.color.as_ref(), color[0], color[1], color[2]);
                    gl.uniform_1_f32(resources.opacity.as_ref(), opacity);
                    gl.uniform_1_f32(resources.glow.as_ref(), glow);
                    gl.uniform_1_f32(resources.thickness.as_ref(), thickness);
                    gl.uniform_1_f32(resources.rotation.as_ref(), rotation);
                    gl.draw_arrays(glow::TRIANGLES, 0, 3);
                    gl.bind_vertex_array(None);
                    gl.use_program(None);
                }
            })),
        });
    }

    pub fn destroy(&self, gl: &glow::Context) {
        if let Ok(mut resources) = self.resources.lock() {
            if let Some(resources) = resources.take() {
                unsafe {
                    gl.delete_vertex_array(resources.vao);
                    gl.delete_program(resources.program);
                }
            }
        }
    }
}
