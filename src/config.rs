use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

const HOME_ENV: &str = "LOOPKARAOKE_HOME";

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
}
