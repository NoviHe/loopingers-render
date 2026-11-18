use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::render::encoder::VideoRenderSettings;

const HOME_ENV: &str = "LOOPKARAOKE_HOME";
const SETTINGS_FILE: &str = "settings.json";

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    /// Intermediate files (subtitles, merged audio, generated backgrounds, frames).
    pub temp: PathBuf,
    pub outputs: PathBuf,
    pub models: PathBuf,
    pub effects: PathBuf,
}

impl AppPaths {
    /// Release builds keep user data next to the executable, debug builds in the working
    /// directory. `LOOPKARAOKE_HOME` overrides both.
    pub fn detect() -> io::Result<Self> {
        let exe_dir = env::current_exe()?
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let data_dir = match env::var_os(HOME_ENV) {
            Some(dir) => PathBuf::from(dir),
            None if cfg!(debug_assertions) => env::current_dir()?,
            None => exe_dir.clone(),
        };
        let bundled = exe_dir.join("assets").join("effects");
        let effects = if bundled.is_dir() {
            bundled
        } else {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("assets")
                .join("effects")
        };
        Self::new(&data_dir, effects)
    }

    pub fn new(data_dir: &Path, effects: PathBuf) -> io::Result<Self> {
        let data_dir = std::path::absolute(data_dir)?;
        let paths = Self {
            temp: data_dir.join("temp"),
            outputs: data_dir.join("outputs"),
            models: data_dir.join("models"),
            effects,
            data_dir,
        };
        for dir in [&paths.temp, &paths.outputs, &paths.models] {
            fs::create_dir_all(dir)?;
        }
        Ok(paths)
    }

    pub fn settings_file(&self) -> PathBuf {
        self.data_dir.join(SETTINGS_FILE)
    }
}

fn default_true() -> bool {
    true
}

/// Persistent Generate advanced options saved locally in `settings.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GenerateAdvancedSettings {
    pub advanced_open: bool,
    pub fade_out_secs: String,
    pub crossfade_secs: String,
    pub manual_loop_duration: String,
    pub resolution_idx: usize,
    pub audio_bitrate_idx: usize,
    pub visual_variation_idx: usize,
    pub volume_norm_idx: usize,
    #[serde(default = "default_true")]
    pub anti_duplicate: bool,
}

impl Default for GenerateAdvancedSettings {
    fn default() -> Self {
        Self {
            advanced_open: false,
            fade_out_secs: "0".to_owned(),
            crossfade_secs: "0".to_owned(),
            manual_loop_duration: String::new(),
            resolution_idx: 0,
            audio_bitrate_idx: 1,
            visual_variation_idx: 0,
            volume_norm_idx: 1,
            anti_duplicate: true,
        }
    }
}

/// Persistent application settings saved locally in `settings.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DependencySettings {
    pub tools_dir: Option<PathBuf>,
    pub whisper_model: Option<String>,
}

impl DependencySettings {
    pub fn selected_whisper_model(&self) -> crate::karaoke::transcribe::ModelSize {
        use crate::karaoke::transcribe::ModelSize;
        ModelSize::ALL
            .into_iter()
            .find(|model| Some(model.name()) == self.whisper_model.as_deref())
            .unwrap_or(ModelSize::Base)
    }
}

/// Persistent application settings saved locally in `settings.json`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpectrumKind {
    #[default]
    Bars,
    HaloSpectrum,
    HaloGeometry,
    RingGlow,
}

impl SpectrumKind {
    pub const ALL: [Self; 4] = [
        Self::Bars,
        Self::HaloSpectrum,
        Self::HaloGeometry,
        Self::RingGlow,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Bars => "Bar Horizontal",
            Self::HaloSpectrum => "Halo Spectrum",
            Self::HaloGeometry => "Halo Geometry",
            Self::RingGlow => "Ring Glow",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SpectrumOverlaySettings {
    pub kind: SpectrumKind,
    pub enabled: bool,
    pub color: [u8; 3],
    pub secondary_color: [u8; 3],
    pub width_pct: u32,
    pub height_pct: u32,
    pub x_pct: u32,
    pub y_pct: u32,
    pub sensitivity_pct: u32,
    pub smoothing_pct: u32,
    pub glow_pct: u32,
    pub opacity_pct: u32,
    pub thickness_px: u32,
    pub rotation_speed: u32,
}

impl Default for SpectrumOverlaySettings {
    fn default() -> Self {
        Self {
            kind: SpectrumKind::Bars,
            enabled: true,
            color: [40, 190, 255],
            secondary_color: [112, 80, 255],
            width_pct: 70,
            height_pct: 22,
            x_pct: 50,
            y_pct: 82,
            sensitivity_pct: 100,
            smoothing_pct: 65,
            glow_pct: 65,
            opacity_pct: 100,
            thickness_px: 2,
            rotation_speed: 0,
        }
    }
}

impl SpectrumOverlaySettings {
    pub fn select_kind(&mut self, kind: SpectrumKind) {
        let (enabled, smoothing_pct) = (self.enabled, self.smoothing_pct);
        *self = Self {
            kind,
            enabled,
            smoothing_pct,
            ..Self::default()
        };
        if kind != SpectrumKind::Bars {
            self.width_pct = 55;
            self.height_pct = 55;
            self.x_pct = 50;
            self.y_pct = 50;
            self.color = match kind {
                SpectrumKind::HaloSpectrum => [255, 255, 255],
                SpectrumKind::HaloGeometry => [255, 30, 230],
                SpectrumKind::RingGlow => [255, 225, 90],
                SpectrumKind::Bars => self.color,
            };
            self.glow_pct = if kind == SpectrumKind::HaloSpectrum {
                15
            } else {
                85
            };
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualizerTextFont {
    Sans,
    #[default]
    Semibold,
    Monospace,
}

impl VisualizerTextFont {
    pub const ALL: [Self; 3] = [Self::Sans, Self::Semibold, Self::Monospace];

    pub fn label(self) -> &'static str {
        match self {
            Self::Sans => "Sans Serif",
            Self::Semibold => "Semibold",
            Self::Monospace => "Monospace",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VisualizerTextSettings {
    pub enabled: bool,
    pub use_song_title: bool,
    pub content: String,
    pub font: VisualizerTextFont,
    pub size_px: u32,
    pub color: [u8; 3],
    pub opacity_pct: u32,
    pub x_pct: u32,
    pub y_pct: u32,
}

impl Default for VisualizerTextSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            use_song_title: true,
            content: String::new(),
            font: VisualizerTextFont::Semibold,
            size_px: 48,
            color: [255, 255, 255],
            opacity_pct: 100,
            x_pct: 50,
            y_pct: 12,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VisualizerBackgroundSettings {
    pub enabled: bool,
    pub blur_px: u32,
    pub vignette_pct: u32,
    pub dim_pct: u32,
    pub tint_pct: u32,
    pub tint_color: [u8; 3],
    pub beat_pulse: bool,
    pub beat_strength_pct: u32,
}

impl Default for VisualizerBackgroundSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            blur_px: 0,
            vignette_pct: 0,
            dim_pct: 0,
            tint_pct: 0,
            tint_color: [47, 214, 196],
            beat_pulse: false,
            beat_strength_pct: 8,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VisualizerLogoSettings {
    pub enabled: bool,
    pub path: Option<PathBuf>,
    pub size_pct: u32,
    pub opacity_pct: u32,
    pub x_pct: u32,
    pub y_pct: u32,
    pub beat_pulse: bool,
}

impl Default for VisualizerLogoSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            path: None,
            size_pct: 20,
            opacity_pct: 100,
            x_pct: 50,
            y_pct: 50,
            beat_pulse: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VisualizerPreviewSettings {
    pub text: VisualizerTextSettings,
    pub background: VisualizerBackgroundSettings,
    pub logo: VisualizerLogoSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KaraokeBannerSettings {
    pub enabled: bool,
    pub font_preset: usize,
    pub size: u32,
    pub color: [u8; 3],
    pub x: f32,
    pub y: f32,
}

impl Default for KaraokeBannerSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            font_preset: 0,
            size: 48,
            color: [255; 3],
            x: 50.0,
            y: 15.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PagePinSettings {
    pub karaoke: bool,
    pub visualizer: bool,
    pub thumbnail: bool,
    pub lyrics: bool,
    pub playlist: bool,
}

impl Default for PagePinSettings {
    fn default() -> Self {
        Self {
            karaoke: true,
            visualizer: true,
            thumbnail: true,
            lyrics: true,
            playlist: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KaraokeLyricStyleSettings {
    pub theme: usize,
    pub normal: [u8; 3],
    pub singing: [u8; 3],
    pub outline: [u8; 3],
    pub shadow: [u8; 3],
    pub font_size: u32,
    pub font_preset: usize,
    pub custom_font: Option<PathBuf>,
    pub alignment: u8,
    pub margin: i32,
    pub x: f32,
    pub y: f32,
    pub smooth_wipe: bool,
}

impl Default for KaraokeLyricStyleSettings {
    fn default() -> Self {
        Self {
            theme: 0,
            normal: [224; 3],
            singing: [255, 215, 0],
            outline: [0; 3],
            shadow: [32; 3],
            font_size: 48,
            font_preset: 0,
            custom_font: None,
            alignment: 2,
            margin: 70,
            x: 50.0,
            y: 85.0,
            smooth_wipe: true,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KaraokeSettings {
    pub lyric_style: KaraokeLyricStyleSettings,
    pub banner: KaraokeBannerSettings,
    pub preview_text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VisualizerSettings {
    pub spectrum: SpectrumOverlaySettings,
    pub preview: VisualizerPreviewSettings,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageSettings {
    pub output_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SettingsPage {
    pub video: VideoRenderSettings,
    pub storage: StorageSettings,
    pub dependencies: DependencySettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub dark_mode: bool,
    pub settings: SettingsPage,
    pub resolution: String,
    pub fps: u32,
    pub auto_generate_thumbnail: bool,
    pub generate: GenerateAdvancedSettings,
    pub visualizer: VisualizerSettings,
    pub karaoke: KaraokeSettings,
    #[serde(rename = "pinup", alias = "page_pins")]
    pub page_pins: PagePinSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            dark_mode: true,
            settings: SettingsPage::default(),
            resolution: "1080p".to_owned(),
            fps: 30,
            auto_generate_thumbnail: true,
            generate: GenerateAdvancedSettings::default(),
            visualizer: VisualizerSettings::default(),
            karaoke: KaraokeSettings::default(),
            page_pins: PagePinSettings::default(),
        }
    }
}

impl AppSettings {
    pub fn load(paths: &AppPaths) -> Self {
        let path = paths.settings_file();
        fs::read_to_string(path)
            .ok()
            .and_then(|raw| Self::from_settings_json(&raw))
            .unwrap_or_default()
    }

    fn from_settings_json(raw: &str) -> Option<Self> {
        let mut config: serde_json::Value = serde_json::from_str(raw).ok()?;
        if let Some(fields) = config.as_object_mut() {
            let video = fields.remove("video");
            let dependencies = fields.remove("dependencies");
            let output_dir = fields.remove("output_dir");
            if video.is_some() || dependencies.is_some() || output_dir.is_some() {
                let settings = fields
                    .entry("settings")
                    .or_insert_with(|| serde_json::json!({}));
                if let Some(settings) = settings.as_object_mut() {
                    if let Some(video) = video {
                        settings.entry("video").or_insert(video);
                    }
                    if let Some(dependencies) = dependencies {
                        settings.entry("dependencies").or_insert(dependencies);
                    }
                    if let Some(output_dir) = output_dir {
                        let storage = settings
                            .entry("storage")
                            .or_insert_with(|| serde_json::json!({}));
                        if let Some(storage) = storage.as_object_mut() {
                            storage.entry("output_dir").or_insert(output_dir);
                        }
                    }
                }
            }
            for (old, new) in [
                ("spectrum_overlay", "spectrum"),
                ("visualizer_preview", "preview"),
            ] {
                if let Some(value) = fields.remove(old) {
                    let visualizer = fields
                        .entry("visualizer")
                        .or_insert_with(|| serde_json::json!({}));
                    if let Some(visualizer) = visualizer.as_object_mut() {
                        visualizer.entry(new).or_insert(value);
                    }
                }
            }
            if let Some(banner) = fields.remove("karaoke_banner") {
                let karaoke = fields
                    .entry("karaoke")
                    .or_insert_with(|| serde_json::json!({}));
                if let Some(karaoke) = karaoke.as_object_mut() {
                    karaoke.entry("banner").or_insert(banner);
                }
            }
        }
        serde_json::from_value::<Self>(config).ok()
    }

    pub fn save(&self, paths: &AppPaths) -> io::Result<()> {
        let path = paths.settings_file();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        fs::write(path, json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::encoder::{VideoCodec, VideoCompression, VideoTarget};

    #[test]
    fn visualizer_layers_default_for_old_settings_and_persist_independently() {
        let mut settings: AppSettings =
            AppSettings::from_settings_json(r#"{"spectrum_overlay":{"kind":"ring_glow"}}"#)
                .unwrap();
        assert_eq!(
            settings.visualizer.preview,
            VisualizerPreviewSettings::default()
        );
        let old_background: VisualizerBackgroundSettings =
            serde_json::from_str(r#"{"blur_px":12}"#).unwrap();
        assert!(old_background.enabled);
        assert_eq!(old_background.blur_px, 12);
        settings.visualizer.preview.text = VisualizerTextSettings {
            enabled: true,
            use_song_title: false,
            content: "Judul khusus".into(),
            font: VisualizerTextFont::Monospace,
            ..Default::default()
        };
        settings.visualizer.preview.background = VisualizerBackgroundSettings {
            blur_px: 12,
            vignette_pct: 45,
            dim_pct: 30,
            tint_pct: 15,
            beat_pulse: true,
            ..Default::default()
        };
        settings.visualizer.preview.logo = VisualizerLogoSettings {
            enabled: true,
            path: Some(PathBuf::from("custom-logo.png")),
            size_pct: 25,
            ..Default::default()
        };
        let restored: AppSettings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(restored, settings);
        assert_eq!(restored.visualizer.spectrum.kind, SpectrumKind::RingGlow);
        settings
            .visualizer
            .spectrum
            .select_kind(SpectrumKind::HaloSpectrum);
        assert_eq!(settings.visualizer.preview, restored.visualizer.preview);
    }

    #[test]
    fn dependencies_default_for_older_settings_and_custom_tools_round_trip() {
        let old: AppSettings = serde_json::from_str(r#"{"dark_mode":false,"fps":60}"#).unwrap();
        assert_eq!(old.settings.dependencies, DependencySettings::default());
        assert!(!old.dark_mode);
        assert_eq!(old.fps, 60);
        let configured = AppSettings {
            settings: SettingsPage {
                dependencies: DependencySettings {
                    tools_dir: Some(PathBuf::from("custom-tools")),
                    whisper_model: Some("small".to_owned()),
                },
                ..SettingsPage::default()
            },
            ..old
        };
        let saved = serde_json::to_string(&configured).unwrap();
        assert_eq!(
            serde_json::from_str::<AppSettings>(&saved).unwrap(),
            configured
        );
    }

    #[test]
    fn spectrum_presets_restore_defaults_and_round_trip_without_breaking_old_settings() {
        let old: SpectrumOverlaySettings =
            serde_json::from_str(r#"{"color":[255,255,255],"width_pct":90}"#).unwrap();
        assert_eq!(old.kind, SpectrumKind::Bars);
        assert_eq!(old.smoothing_pct, 65);
        for kind in SpectrumKind::ALL {
            let mut settings = old.clone();
            settings.select_kind(kind);
            assert_eq!(settings.kind, kind);
            let saved = serde_json::to_string(&settings).unwrap();
            assert_eq!(
                serde_json::from_str::<SpectrumOverlaySettings>(&saved).unwrap(),
                settings
            );
            assert!(settings.height_pct <= 60);
            if kind != SpectrumKind::Bars {
                assert_eq!((settings.x_pct, settings.y_pct), (50, 50));
            }
        }
    }

    #[test]
    fn whisper_model_choice_survives_disk_reload_and_invalid_choice_falls_back() {
        use crate::karaoke::transcribe::ModelSize;
        let root = std::env::temp_dir().join(format!("whisper-settings-{}", uuid::Uuid::new_v4()));
        let paths = AppPaths::new(&root, root.join("effects")).unwrap();
        for model in ModelSize::ALL {
            let mut settings = AppSettings::default();
            settings.settings.dependencies.whisper_model = Some(model.name().to_owned());
            settings.save(&paths).unwrap();
            assert_eq!(
                AppSettings::load(&paths)
                    .settings
                    .dependencies
                    .selected_whisper_model(),
                model
            );
        }
        let mut settings = AppSettings::default();
        settings.settings.dependencies.whisper_model = Some("unknown-model".to_owned());
        assert_eq!(
            settings.settings.dependencies.selected_whisper_model(),
            ModelSize::Base
        );
        assert_eq!(
            DependencySettings::default().selected_whisper_model(),
            ModelSize::Base
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn legacy_menu_visibility_settings_are_ignored_without_losing_preferences() {
        let saved = r#"{"dark_mode":false,"fps":60,"show_tab_playlist":false,"show_tab_lyrics":false,"show_tab_thumbnail":false}"#;
        let settings: AppSettings = serde_json::from_str(saved).unwrap();
        assert!(!settings.dark_mode);
        assert_eq!(settings.fps, 60);
        let serialized = serde_json::to_value(&settings).unwrap();
        assert!(serialized
            .as_object()
            .unwrap()
            .keys()
            .all(|key| !key.starts_with("show_tab_")));
    }

    #[test]
    fn settings_round_trip_and_fallback() {
        let root = std::env::temp_dir().join(format!("loopkaraoke-cfg-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let paths = AppPaths::new(&root, root.join("effects")).unwrap();

        assert_eq!(AppSettings::load(&paths), AppSettings::default());

        let custom = AppSettings {
            dark_mode: false,
            settings: SettingsPage {
                storage: StorageSettings {
                    output_dir: Some(root.join("custom-outputs")),
                },
                video: VideoRenderSettings {
                    target: VideoTarget::YouTubeStandard,
                    codec: VideoCodec::H264,
                    compression: VideoCompression::CompactSize,
                    speed: crate::render::encoder::RenderSpeed::Balanced,
                },
                ..SettingsPage::default()
            },
            resolution: "720p".to_owned(),
            fps: 60,
            generate: GenerateAdvancedSettings {
                advanced_open: true,
                fade_out_secs: "5".to_owned(),
                crossfade_secs: "3".to_owned(),
                manual_loop_duration: "01:02:00".to_owned(),
                resolution_idx: 3,
                audio_bitrate_idx: 2,
                visual_variation_idx: 1,
                volume_norm_idx: 0,
                anti_duplicate: false,
            },
            visualizer: VisualizerSettings {
                spectrum: SpectrumOverlaySettings {
                    enabled: false,
                    color: [255, 80, 120],
                    width_pct: 85,
                    height_pct: 30,
                    sensitivity_pct: 175,
                    ..SpectrumOverlaySettings::default()
                },
                ..VisualizerSettings::default()
            },
            ..AppSettings::default()
        };
        custom.save(&paths).unwrap();
        assert_eq!(AppSettings::load(&paths), custom);

        fs::write(paths.settings_file(), "{ broken json").unwrap();
        assert_eq!(AppSettings::load(&paths), AppSettings::default());

        let _ = fs::remove_dir_all(&root);
    }
}
