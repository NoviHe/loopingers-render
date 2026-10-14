use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Output, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use tokio::process::Command;

use crate::error::{Error, Result};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const ERROR_TAIL_CHARS: usize = 400;
const FFMPEG_DOWNLOAD_URL: &str =
    "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip";
const DOWNLOAD_CHUNK: usize = 64 * 1024;

/// Locations of the external FFmpeg binaries. A copy next to the executable wins, then one in the
/// app's own tools folder, then `PATH`.
#[derive(Debug, Clone)]
pub struct Tools {
    ffmpeg: PathBuf,
    ffprobe: PathBuf,
}

impl Tools {
    pub fn locate() -> Self {
        Self::locate_in(Path::new(""))
    }

    pub fn locate_in(tools_dir: &Path) -> Self {
        Self {
            ffmpeg: find_binary("ffmpeg", tools_dir),
            ffprobe: find_binary("ffprobe", tools_dir),
        }
    }

    pub fn ffmpeg(&self) -> Command {
        command(&self.ffmpeg)
    }

    pub fn ffprobe(&self) -> Command {
        command(&self.ffprobe)
    }

    pub async fn check(&self) -> Result<()> {
        run(self.ffmpeg().arg("-version"), "ffmpeg").await?;
        run(self.ffprobe().arg("-version"), "ffprobe").await?;
        Ok(())
    }
}

fn find_binary(name: &str, tools_dir: &Path) -> PathBuf {
    let file_name = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    exe_dir
        .into_iter()
        .chain([tools_dir.to_path_buf()])
        .map(|dir| dir.join(&file_name))
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| PathBuf::from(file_name))
}

/// Bytes downloaded so far, shared with the UI to draw a progress bar.
#[derive(Debug, Default)]
pub struct DownloadProgress {
    pub downloaded: AtomicU64,
    pub total: AtomicU64,
}

/// Returns working FFmpeg tools, downloading a Windows build into `tools_dir` when none are found.
pub async fn ensure_installed(
    tools_dir: PathBuf,
    progress: Arc<DownloadProgress>,
) -> Result<Tools> {
    let tools = Tools::locate_in(&tools_dir);
    if tools.check().await.is_ok() {
        return Ok(tools);
    }
    if !cfg!(windows) {
        return Err(Error::Tool(
            "FFmpeg tidak ditemukan. Pasang FFmpeg lewat package manager Anda (ffmpeg dan ffprobe harus ada di PATH)."
                .to_owned(),
        ));
    }

    let target = tools_dir.clone();
    tokio::task::spawn_blocking(move || download_ffmpeg(&target, &progress))
        .await
        .map_err(|error| Error::Tool(format!("Pemasangan FFmpeg terhenti: {error}")))??;

    let tools = Tools::locate_in(&tools_dir);
    tools.check().await?;
    Ok(tools)
}

fn download_ffmpeg(tools_dir: &Path, progress: &DownloadProgress) -> Result<()> {
    let failure = |error: &dyn std::fmt::Display| {
        Error::Tool(format!(
            "Gagal mengunduh FFmpeg dari {FFMPEG_DOWNLOAD_URL}: {error}"
        ))
    };
    fs::create_dir_all(tools_dir)?;
    let archive_path = tools_dir.join("ffmpeg-download.zip");

    let response = ureq::get(FFMPEG_DOWNLOAD_URL)
        .call()
        .map_err(|error| failure(&error))?;
    progress.downloaded.store(0, Ordering::Relaxed);
    progress.total.store(
        response.body().content_length().unwrap_or(0),
        Ordering::Relaxed,
    );

    let mut reader = response.into_body().into_reader();
    let mut file = File::create(&archive_path)?;
    let mut chunk = vec![0u8; DOWNLOAD_CHUNK];
    loop {
        let read = reader.read(&mut chunk).map_err(|error| failure(&error))?;
        if read == 0 {
            break;
        }
        file.write_all(&chunk[..read])?;
        progress
            .downloaded
            .fetch_add(read as u64, Ordering::Relaxed);
    }
    file.flush()?;
    drop(file);

    let extracted = extract_binaries(&archive_path, tools_dir);
    let _ = fs::remove_file(&archive_path);
    extracted
}

/// Copies `ffmpeg.exe` and `ffprobe.exe` out of the archive's `bin` folder.
fn extract_binaries(archive_path: &Path, tools_dir: &Path) -> Result<()> {
    let broken =
        |error: &dyn std::fmt::Display| Error::Tool(format!("Arsip FFmpeg tidak valid: {error}"));
    let mut archive = zip::ZipArchive::new(File::open(archive_path)?).map_err(|e| broken(&e))?;
    let wanted = ["ffmpeg.exe", "ffprobe.exe"];
    let mut found = 0;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|e| broken(&e))?;
        let name = entry.name().to_owned();
        let Some(file_name) = wanted
            .iter()
            .find(|candidate| name.ends_with(&format!("/bin/{candidate}")))
        else {
            continue;
        };
        io::copy(&mut entry, &mut File::create(tools_dir.join(file_name))?)?;
        found += 1;
    }
    if found == wanted.len() {
        Ok(())
    } else {
        Err(broken(&"ffmpeg.exe atau ffprobe.exe tidak ada di dalamnya"))
    }
}

pub fn command(program: &Path) -> Command {
    let mut cmd = Command::new(program);
    cmd.stdin(Stdio::null()).kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Runs `cmd` to completion and turns a non-zero exit into an error carrying the stderr tail.
pub async fn run(cmd: &mut Command, label: &str) -> Result<Output> {
    let output = cmd
        .output()
        .await
        .map_err(|error| Error::Tool(format!("{label} tidak dapat dijalankan: {error}")))?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(Error::Tool(format!(
            "{label} gagal: {}",
            tail(&String::from_utf8_lossy(&output.stderr), ERROR_TAIL_CHARS)
        )))
    }
}

/// Last `max_chars` characters of `text`, without splitting a character.
pub fn tail(text: &str, max_chars: usize) -> String {
    let text = text.trim();
    let skip = text.chars().count().saturating_sub(max_chars);
    text.chars().skip(skip).collect()
}

#[cfg(test)]
mod tests {
    use super::tail;

    #[test]
    fn tail_keeps_the_last_characters() {
        assert_eq!(tail("abcdef", 3), "def");
        assert_eq!(tail("ab", 5), "ab");
        assert_eq!(tail("日本語テキスト", 3), "キスト");
    }
}
