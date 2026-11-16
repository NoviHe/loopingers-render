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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub dark_mode: bool,
    pub video: VideoRenderSettings,
    pub show_tab_background: bool,
    pub show_tab_playlist: bool,
    pub show_tab_lyrics: bool,
    pub show_tab_visualizer: bool,
    pub show_tab_render: bool,
    pub show_tab_thumbnail: bool,
    pub resolution: String,
    pub fps: u32,
    pub auto_generate_thumbnail: bool,
    pub generate: GenerateAdvancedSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            dark_mode: true,
            video: VideoRenderSettings::default(),
            show_tab_background: false,
            show_tab_playlist: false,
            show_tab_lyrics: true,
            show_tab_visualizer: true,
            show_tab_render: false,
            show_tab_thumbnail: true,
            resolution: "1080p".to_owned(),
            fps: 30,
            auto_generate_thumbnail: true,
            generate: GenerateAdvancedSettings::default(),
        }
    }
}

impl AppSettings {
    pub fn load(paths: &AppPaths) -> Self {
        let path = paths.settings_file();
        fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Self>(&raw).ok())
            .unwrap_or_default()
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
    fn settings_round_trip_and_fallback() {
        let root = std::env::temp_dir().join(format!("loopkaraoke-cfg-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let paths = AppPaths::new(&root, root.join("effects")).unwrap();

        assert_eq!(AppSettings::load(&paths), AppSettings::default());

        let custom = AppSettings {
            dark_mode: false,
            video: VideoRenderSettings {
                target: VideoTarget::YouTubeStandard,
                codec: VideoCodec::H264,
                compression: VideoCompression::CompactSize,
            },
            show_tab_render: true,
            resolution: "720p".to_owned(),
            fps: 60,
            generate: GenerateAdvancedSettings {
                fade_out_secs: "5".to_owned(),
                crossfade_secs: "3".to_owned(),
                manual_loop_duration: "01:02:00".to_owned(),
                resolution_idx: 3,
                audio_bitrate_idx: 2,
                visual_variation_idx: 1,
                volume_norm_idx: 0,
                anti_duplicate: false,
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
