use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use crate::{
    error::Result,
    ffmpeg::{self, DownloadProgress, Tools},
    karaoke::transcribe::{self, ModelSize},
};

#[derive(Debug, Clone)]
pub struct ToolInfo {
    pub name: String,
    pub path: PathBuf,
    pub version: Option<String>,
    pub size_bytes: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub size: ModelSize,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub installed: bool,
}

#[derive(Debug, Clone)]
pub struct DependencyReport {
    pub tools: Vec<ToolInfo>,
    pub models: Vec<ModelInfo>,
    pub backend: &'static str,
}

pub async fn inspect(tools: Tools, models_dir: PathBuf) -> DependencyReport {
    inspect_paths(tools.ffmpeg_path(), tools.ffprobe_path(), models_dir).await
}

/// Inspects the configured location even when one of its binaries is missing.
pub async fn inspect_directory(directory: PathBuf, models_dir: PathBuf) -> DependencyReport {
    inspect_paths(
        &directory.join(format!("ffmpeg{}", std::env::consts::EXE_SUFFIX)),
        &directory.join(format!("ffprobe{}", std::env::consts::EXE_SUFFIX)),
        models_dir,
    )
    .await
}

async fn inspect_paths(
    ffmpeg_path: &Path,
    ffprobe_path: &Path,
    models_dir: PathBuf,
) -> DependencyReport {
    let (ffmpeg, ffprobe) = tokio::join!(
        inspect_tool("FFmpeg", ffmpeg_path),
        inspect_tool("FFprobe", ffprobe_path),
    );
    DependencyReport {
        tools: vec![ffmpeg, ffprobe],
        models: model_inventory(&models_dir),
        backend: if cfg!(feature = "cuda") {
            "CUDA"
        } else if cfg!(feature = "vulkan") {
            "Vulkan"
        } else {
            "CPU"
        },
    }
}

fn resolved_tool_path(path: &Path) -> PathBuf {
    if path.is_absolute() || path.components().count() > 1 {
        return std::path::absolute(path).unwrap_or_else(|_| path.to_owned());
    }
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .map(|directory| directory.join(path))
        .find(|candidate| candidate.is_file())
        .map(|candidate| std::path::absolute(&candidate).unwrap_or(candidate))
        .unwrap_or_else(|| path.to_owned())
}

async fn inspect_tool(name: &str, selected_path: &Path) -> ToolInfo {
    let path = resolved_tool_path(selected_path);
    let size_bytes = std::fs::metadata(&path).map_or(0, |metadata| metadata.len());
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        ffmpeg::command(&path).arg("-version").output(),
    )
    .await;
    let (version, error) = match result {
        Ok(Ok(output)) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .unwrap_or_default()
                .trim()
                .to_owned();
            if version.is_empty() {
                (
                    None,
                    Some("Versi dependensi tidak dapat dibaca.".to_owned()),
                )
            } else {
                (Some(version), None)
            }
        }
        Ok(Ok(output)) => (
            None,
            Some(format!(
                "Pemeriksaan gagal ({}): {}",
                output.status,
                ffmpeg::tail(&String::from_utf8_lossy(&output.stderr), 200)
            )),
        ),
        Ok(Err(error)) => (None, Some(error.to_string())),
        Err(_) => (
            None,
            Some("Pemeriksaan melewati batas waktu 10 detik.".to_owned()),
        ),
    };
    ToolInfo {
        name: name.to_owned(),
        path,
        version,
        size_bytes,
        error,
    }
}

fn model_inventory(models_dir: &Path) -> Vec<ModelInfo> {
    [
        ModelSize::Tiny,
        ModelSize::Base,
        ModelSize::Small,
        ModelSize::Medium,
    ]
    .into_iter()
    .map(|size| {
        let path = models_dir.join(size.file_name());
        let size_bytes = std::fs::metadata(&path).map_or(0, |metadata| {
            if metadata.is_file() {
                metadata.len()
            } else {
                0
            }
        });
        ModelInfo {
            size,
            path,
            size_bytes,
            installed: size_bytes > 0,
        }
    })
    .collect()
}

pub async fn download_model(
    models_dir: PathBuf,
    size: ModelSize,
    progress: Arc<DownloadProgress>,
) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        let path = models_dir.join(size.file_name());
        transcribe::ensure_model_with_progress(&path, size, &progress)
    })
    .await
    .map_err(|error| crate::error::Error::internal(format!("Unduhan model terhenti: {error}")))?
}

/// Removes only one recognized model file inside the configured model directory.
pub fn remove_model(models_dir: &Path, size: ModelSize) -> Result<()> {
    let path = models_dir.join(size.file_name());
    let metadata = std::fs::symlink_metadata(&path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(crate::error::Error::internal(
            "Model bukan berkas biasa atau berupa symbolic link.",
        ));
    }
    let directory = std::fs::canonicalize(models_dir)?;
    let resolved = std::fs::canonicalize(&path)?;
    if resolved.parent() != Some(directory.as_path()) {
        return Err(crate::error::Error::internal(
            "Berkas model berada di luar folder model aplikasi.",
        ));
    }
    std::fs::remove_file(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_directory_requires_both_tools_and_does_not_select_bundled_tools() {
        let root = std::env::temp_dir().join(format!("explicit-tools-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ffmpeg = root.join(format!("ffmpeg{}", std::env::consts::EXE_SUFFIX));
        let ffprobe = root.join(format!("ffprobe{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&ffmpeg, b"selected ffmpeg").unwrap();
        assert!(Tools::from_directory(&root).is_err());
        std::fs::write(&ffprobe, b"selected ffprobe").unwrap();
        let tools = Tools::from_directory(&root).unwrap();
        assert_eq!(tools.ffmpeg_path(), ffmpeg);
        assert_eq!(tools.ffprobe_path(), ffprobe);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn removal_only_touches_selected_known_model_and_rejects_directory() {
        let root = std::env::temp_dir().join(format!("remove-model-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("ggml-tiny.bin"), b"tiny").unwrap();
        std::fs::write(root.join("ggml-base.bin"), b"base").unwrap();
        remove_model(&root, ModelSize::Tiny).unwrap();
        assert!(!root.join("ggml-tiny.bin").exists());
        assert!(root.join("ggml-base.bin").is_file());
        std::fs::create_dir(root.join("ggml-small.bin")).unwrap();
        assert!(remove_model(&root, ModelSize::Small).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn inspection_reports_missing_explicit_tools_without_using_fallback() {
        let root = std::env::temp_dir().join(format!("missing-tools-{}", uuid::Uuid::new_v4()));
        assert!(Tools::from_directory(&root).is_err());
        let result = inspect_tool("FFmpeg", &root.join("ffmpeg.exe")).await;
        assert!(result.error.is_some());
        assert!(result.version.is_none());
        assert_eq!(result.path, root.join("ffmpeg.exe"));
        assert_eq!(result.size_bytes, 0);
        let report = inspect_directory(root.clone(), root.join("models")).await;
        for (tool, name) in report.tools.iter().zip(["ffmpeg", "ffprobe"]) {
            assert_eq!(
                tool.path,
                root.join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
            );
            assert!(tool.error.is_some());
            assert!(tool.version.is_none());
        }
    }

    #[test]
    fn model_inventory_does_not_treat_empty_or_partial_downloads_as_installed() {
        let root = std::env::temp_dir().join(format!("model-inventory-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("ggml-tiny.bin"), b"").unwrap();
        std::fs::write(root.join("ggml-base.bin.part"), b"partial download").unwrap();
        std::fs::write(root.join("ggml-small.bin"), b"model data").unwrap();
        let models = model_inventory(&root);
        assert!(!models[0].installed);
        assert!(!models[1].installed);
        assert!(models[2].installed);
        assert_eq!(models[2].size_bytes, 10);
        assert!(!models[3].installed);
        std::fs::remove_dir_all(root).unwrap();
    }
}
