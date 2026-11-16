use std::path::PathBuf;

use crate::render::graph::FilterGraph;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualizerPreset {
    BarsRound,
    BarsDense,
    RadialNcs,
    RadialPulse,
    LedClassic,
    LedMirror,
    WaveGraph,
    CyberGrid,
    SpiralGalaxy,
    Starburst,
}

impl VisualizerPreset {
    pub const ALL: [Self; 10] = [
        Self::BarsRound,
        Self::BarsDense,
        Self::RadialNcs,
        Self::RadialPulse,
        Self::LedClassic,
        Self::LedMirror,
        Self::WaveGraph,
        Self::CyberGrid,
        Self::SpiralGalaxy,
        Self::Starburst,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::BarsRound => "Bars Round (Balok Bulat)",
            Self::BarsDense => "Bars Dense (Spektrum Rapat)",
            Self::RadialNcs => "Radial NCS (Lingkaran Melingkar)",
            Self::RadialPulse => "Radial Pulse (Denyut Cincin)",
            Self::LedClassic => "LED Classic (Lampu Spektrum)",
            Self::LedMirror => "LED Mirror (Simetris Kiri-Kanan)",
            Self::WaveGraph => "Wave Graph (Gelombang Halus)",
            Self::CyberGrid => "Cyber Grid (Grid Futuristik)",
            Self::SpiralGalaxy => "Spiral Galaxy (Pusaran Galaksi)",
            Self::Starburst => "Starburst (Kilauan Bintang)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualizerPosition {
    Center,
    Bottom,
    Full,
}

impl VisualizerPosition {
    pub const ALL: [Self; 3] = [Self::Center, Self::Bottom, Self::Full];

    pub fn label(self) -> &'static str {
        match self {
            Self::Center => "Tengah (Center)",
            Self::Bottom => "Bawah (Bottom)",
            Self::Full => "Layar Penuh (Full)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticleType {
    None,
    Dust,
    Sparkles,
    Fire,
    Snow,
    NeonRain,
    NcsRise,
}

impl ParticleType {
    pub const ALL: [Self; 7] = [
        Self::None,
        Self::Dust,
        Self::Sparkles,
        Self::Fire,
        Self::Snow,
        Self::NeonRain,
        Self::NcsRise,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "Tanpa Partikel",
            Self::Dust => "Bintik (Dust)",
            Self::Sparkles => "Bintang (Sparkles)",
            Self::Fire => "Percikan Api (Fire)",
            Self::Snow => "Salju Halus (Snow)",
            Self::NeonRain => "Hujan Neon",
            Self::NcsRise => "NCS Melayang (Rise)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSourceMode {
    AutoPlaylist,
    Custom,
}

impl TextSourceMode {
    pub const ALL: [Self; 2] = [Self::AutoPlaylist, Self::Custom];

    pub fn label(self) -> &'static str {
        match self {
            Self::AutoPlaylist => "Otomatis dari Lagu Playlist",
            Self::Custom => "Teks Kustom",
        }
    }
}

#[derive(Debug, Clone)]
pub struct VisualizerSpec {
    pub enabled: bool,
    pub preset: VisualizerPreset,
    pub position: VisualizerPosition,
    pub size: f32,           // 20.0 ..= 100.0 (persen)
    pub sensitivity: f32,    // 10.0 ..= 100.0 (gain)
    pub rotation_deg: f32,   // 0.0 ..= 360.0
    pub color_a: [u8; 3],    // Warna Bawah / Primer (Hex)
    pub color_b: [u8; 3],    // Warna Atas / Sekunder (Hex)
    pub bg_dim: f32,         // 0.0 ..= 0.8
    pub bg_blur: f32,        // 0.0 ..= 20.0
    pub bg_tint_amount: f32, // 0.0 ..= 0.7
    pub bg_tint_color: [u8; 3],
    pub bg_neon_frame: bool,
    pub bg_neon_width: f32, // 4.0 ..= 28.0
    pub bg_pulse: bool,
    pub particle: ParticleType,
    pub particle_density: u32,
    pub show_text: bool,
    pub text_mode: TextSourceMode,
    pub text_custom: String,
    pub text_color: [u8; 3],
    pub text_size: f32,
    pub text_glow: bool,
    pub text_position: u8, // 0 = Atas, 1 = Tengah, 2 = Bawah
    pub show_overlay: bool,
    pub overlay_path: Option<PathBuf>,
    pub overlay_size: f32,
    pub overlay_opacity: f32,
    pub overlay_depth_behind: bool,
    pub overlay_knockout_black: bool,
    pub overlay_pulse_beat: bool,
}

impl Default for VisualizerSpec {
    fn default() -> Self {
        Self {
            enabled: false,
            preset: VisualizerPreset::RadialNcs,
            position: VisualizerPosition::Center,
            size: 55.0,
            sensitivity: 65.0,
            rotation_deg: 0.0,
            color_a: [0, 229, 255],  // Neon Cyan #00e5ff
            color_b: [255, 43, 214], // Neon Magenta #ff2bd6
            bg_dim: 0.25,
            bg_blur: 0.0,
            bg_tint_amount: 0.0,
            bg_tint_color: [47, 214, 196],
            bg_neon_frame: true,
            bg_neon_width: 8.0,
            bg_pulse: true,
            particle: ParticleType::Sparkles,
            particle_density: 50,
            show_text: true,
            text_mode: TextSourceMode::AutoPlaylist,
            text_custom: String::new(),
            text_color: [255, 255, 255],
            text_size: 44.0,
            text_glow: true,
            text_position: 0, // Atas
            show_overlay: false,
            overlay_path: None,
            overlay_size: 35.0,
            overlay_opacity: 100.0,
            overlay_depth_behind: false,
            overlay_knockout_black: true,
            overlay_pulse_beat: true,
        }
    }
}

impl VisualizerSpec {
    pub fn hex_color(rgb: [u8; 3]) -> String {
        format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
    }

    /// Builds the FFmpeg filtergraph stage that generates the visualizer from audio stream `1:a:0`
    /// and overlays it onto the background video stream.
    pub fn apply_ffmpeg(&self, graph: &mut FilterGraph, width: u32, height: u32) {
        let col_a = format!(
            "0x{:02x}{:02x}{:02x}",
            self.color_a[0], self.color_a[1], self.color_a[2]
        );
        let col_b = format!(
            "0x{:02x}{:02x}{:02x}",
            self.color_b[0], self.color_b[1], self.color_b[2]
        );

        let scale_factor = (self.size / 100.0).clamp(0.2, 1.0);
        let viz_w = ((width as f32 * scale_factor) as u32 / 2) * 2;
        let viz_h = match self.position {
            VisualizerPosition::Bottom => ((height as f32 * 0.35 * scale_factor) as u32 / 2) * 2,
            VisualizerPosition::Center => ((height as f32 * 0.60 * scale_factor) as u32 / 2) * 2,
            VisualizerPosition::Full => height,
        };

        let y_expr = match self.position {
            VisualizerPosition::Bottom => format!("H-h-{}", (height as f32 * 0.05) as u32),
            VisualizerPosition::Center => "(H-h)/2".to_string(),
            VisualizerPosition::Full => "0".to_string(),
        };

        // Determine FFmpeg filter mode based on preset
        let (filter_cmd, is_wave) = match self.preset {
            VisualizerPreset::WaveGraph => (
                format!("showwaves=s={viz_w}x{viz_h}:mode=line:colors={col_a}|{col_b}:rate=30"),
                true,
            ),
            VisualizerPreset::CyberGrid => (
                format!("showcqt=s={viz_w}x{viz_h}:bar_g=1:axis=0:basefreq=30:endfreq=12000"),
                false,
            ),
            VisualizerPreset::LedClassic | VisualizerPreset::LedMirror => (
                format!("showfreqs=s={viz_w}x{viz_h}:mode=dot:ascale=log:fscale=log:colors={col_a}|{col_b}"),
                false,
            ),
            _ => (
                format!("showfreqs=s={viz_w}x{viz_h}:mode=bar:ascale=log:fscale=log:colors={col_a}|{col_b}"),
                false,
            ),
        };

        graph.stage(|bg_input| {
            if is_wave {
                format!(
                    "[1:a:0]{filter_cmd},format=rgba,colorkey=0x000000:0.1:0.1[viz_raw];\
                     [{bg_input}][viz_raw]overlay=(W-w)/2:{y_expr}:shortest=1"
                )
            } else {
                format!(
                    "[1:a:0]{filter_cmd},format=rgba,colorkey=0x000000:0.15:0.1[viz_raw];\
                     [{bg_input}][viz_raw]overlay=(W-w)/2:{y_expr}:shortest=1"
                )
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_visualizer_spec_has_sensible_values() {
        let spec = VisualizerSpec::default();
        assert!(!spec.enabled);
        assert_eq!(spec.preset, VisualizerPreset::RadialNcs);
        assert_eq!(spec.position, VisualizerPosition::Center);
        assert!(spec.size > 20.0);
        assert_eq!(VisualizerSpec::hex_color(spec.color_a), "#00e5ff");
        assert_eq!(VisualizerSpec::hex_color(spec.color_b), "#ff2bd6");
    }

    #[test]
    fn presets_and_positions_have_labels() {
        for preset in VisualizerPreset::ALL {
            assert!(!preset.label().is_empty());
        }
        for pos in VisualizerPosition::ALL {
            assert!(!pos.label().is_empty());
        }
        for p in ParticleType::ALL {
            assert!(!p.label().is_empty());
        }
    }

    #[test]
    fn ffmpeg_filter_stages_are_generated() {
        let spec = VisualizerSpec::default();
        let mut graph = FilterGraph::new("0:v");
        spec.apply_ffmpeg(&mut graph, 1920, 1080);
        let filter = graph.finish("null");
        assert!(filter.contains("showfreqs"));
        assert!(filter.contains("colorkey"));
        assert!(filter.contains("overlay"));
    }
}
