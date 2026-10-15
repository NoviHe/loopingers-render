use std::time::Duration;

use tokio::{process::Command, time::timeout};

use crate::ffmpeg::{self, Tools};

const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const ENCODER_TEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Default)]
pub struct HardwareInfo {
    pub gpu_name: String,
    pub has_nvidia: bool,
    pub has_amd: bool,
    pub nvenc_h264: bool,
    pub nvenc_hevc: bool,
    pub amf_h264: bool,
    pub amf_hevc: bool,
    /// Whisper can run on the GPU (build with the `cuda` or `vulkan` feature).
    pub gpu_whisper: bool,
}

impl HardwareInfo {
    pub async fn detect(tools: &Tools) -> Self {
        let listed = list_encoders(tools).await;
        let (nvenc_h264, nvenc_hevc, amf_h264, amf_hevc) = tokio::join!(
            encoder_works(tools, &listed, "h264_nvenc"),
            encoder_works(tools, &listed, "hevc_nvenc"),
            encoder_works(tools, &listed, "h264_amf"),
            encoder_works(tools, &listed, "hevc_amf"),
        );
        let adapters = adapter_names().await;
        let has_nvidia = nvenc_h264 || nvenc_hevc || mentions(&adapters, &["nvidia"]);
        let has_amd = amf_h264 || amf_hevc || mentions(&adapters, &["amd", "radeon"]);

        Self {
            gpu_name: pick_gpu_name(&adapters, has_nvidia, has_amd),
            has_nvidia,
            has_amd,
            nvenc_h264,
            nvenc_hevc,
            amf_h264,
            amf_hevc,
            gpu_whisper: (has_nvidia || has_amd) && cfg!(any(feature = "cuda", feature = "vulkan")),
        }
    }

    pub fn has_gpu(&self) -> bool {
        self.has_nvidia || self.has_amd
    }

    pub fn vendor(&self) -> &'static str {
        match (self.has_nvidia, self.has_amd) {
            (true, _) => "NVIDIA",
            (false, true) => "AMD",
            (false, false) => "CPU",
        }
    }
}

fn mentions(adapters: &[String], needles: &[&str]) -> bool {
    adapters.iter().any(|name| {
        let name = name.to_lowercase();
        needles.iter().any(|needle| name.contains(needle))
    })
}

fn pick_gpu_name(adapters: &[String], has_nvidia: bool, has_amd: bool) -> String {
    let wanted: &[&str] = if has_nvidia {
        &["nvidia"]
    } else if has_amd {
        &["amd", "radeon"]
    } else {
        &[]
    };
    adapters
        .iter()
        .find(|name| {
            let name = name.to_lowercase();
            wanted.iter().any(|needle| name.contains(needle))
        })
        .cloned()
        .unwrap_or_else(|| "CPU Only".to_owned())
}

/// Graphics adapter names: `nvidia-smi` for NVIDIA, the Windows video controller list for the rest.
async fn adapter_names() -> Vec<String> {
    let mut names = Vec::new();

    let mut smi = ffmpeg::command("nvidia-smi".as_ref());
    smi.args(["--query-gpu=name", "--format=csv,noheader"]);
    names.extend(output_lines(smi).await);

    if cfg!(windows) {
        let mut query = ffmpeg::command("powershell".as_ref());
        query.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "(Get-CimInstance Win32_VideoController).Name",
        ]);
        names.extend(output_lines(query).await);
    }
    names
}

async fn output_lines(mut cmd: Command) -> Vec<String> {
    match timeout(PROBE_TIMEOUT, cmd.output()).await {
        Ok(Ok(output)) if output.status.success() => String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

async fn list_encoders(tools: &Tools) -> String {
    let mut cmd = tools.ffmpeg();
    cmd.args(["-hide_banner", "-encoders"]);
    match timeout(PROBE_TIMEOUT, cmd.output()).await {
        Ok(Ok(output)) => String::from_utf8_lossy(&output.stdout).into_owned(),
        _ => String::new(),
    }
}

/// An encoder counts as available only when FFmpeg lists it and a one-frame test encode works,
/// which fails on machines without the matching GPU or driver.
async fn encoder_works(tools: &Tools, listed: &str, encoder: &str) -> bool {
    if !listed.contains(encoder) {
        return false;
    }
    let mut cmd = tools.ffmpeg();
    cmd.args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        "color=size=256x256:rate=1",
        "-frames:v",
        "1",
        "-pix_fmt",
        "yuv420p",
        "-c:v",
        encoder,
        "-f",
        "null",
        "-",
    ]);
    matches!(timeout(ENCODER_TEST_TIMEOUT, cmd.output()).await, Ok(Ok(output)) if output.status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_name_prefers_the_detected_vendor() {
        let adapters = vec!["Intel UHD".to_owned(), "AMD Radeon RX 6600".to_owned()];
        assert_eq!(pick_gpu_name(&adapters, false, true), "AMD Radeon RX 6600");
        assert_eq!(pick_gpu_name(&adapters, false, false), "CPU Only");
        assert!(mentions(&adapters, &["radeon"]));
    }
}
