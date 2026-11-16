use serde::{Deserialize, Serialize};

use crate::hardware::HardwareInfo;

const CPU_PRESETS: [&str; 7] = [
    "ultrafast",
    "superfast",
    "veryfast",
    "faster",
    "fast",
    "medium",
    "slow",
];
const NVENC_PRESETS: [&str; 7] = ["p1", "p2", "p3", "p4", "p5", "p6", "p7"];
const AMF_PRESETS: [&str; 3] = ["speed", "balanced", "quality"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoder {
    NvencH264,
    NvencHevc,
    AmfH264,
    AmfHevc,
    CpuX264,
    CpuX265,
}

impl Encoder {
    pub const ALL: [Self; 6] = [
        Self::NvencH264,
        Self::NvencHevc,
        Self::AmfH264,
        Self::AmfHevc,
        Self::CpuX264,
        Self::CpuX265,
    ];

    pub fn is_hevc(self) -> bool {
        matches!(self, Self::NvencHevc | Self::AmfHevc | Self::CpuX265)
    }

    pub fn is_available(self, hardware: &HardwareInfo) -> bool {
        match self {
            Self::NvencH264 => hardware.nvenc_h264,
            Self::NvencHevc => hardware.nvenc_hevc,
            Self::AmfH264 => hardware.amf_h264,
            Self::AmfHevc => hardware.amf_hevc,
            Self::CpuX264 => true,
            Self::CpuX265 => hardware.cpu_x265,
        }
    }

    /// The requested encoder, or software x264 when this machine cannot provide it.
    pub fn resolve(requested: Self, hardware: &HardwareInfo) -> Self {
        if requested.is_available(hardware) {
            requested
        } else {
            Self::CpuX264
        }
    }

    /// Hardware H.264 (NVIDIA first, then AMD) when available, otherwise software x264.
    pub fn best_available(hardware: &HardwareInfo) -> Self {
        [Self::NvencH264, Self::AmfH264]
            .into_iter()
            .find(|encoder| encoder.is_available(hardware))
            .unwrap_or(Self::CpuX264)
    }

    /// Best available HEVC encoder (NVIDIA, AMD, then CPU x265), falling back to H.264 if unavailable.
    pub fn best_hevc_or_h264(hardware: &HardwareInfo) -> Self {
        [Self::NvencHevc, Self::AmfHevc, Self::CpuX265]
            .into_iter()
            .find(|encoder| encoder.is_available(hardware))
            .unwrap_or_else(|| Self::best_available(hardware))
    }

    /// Short name for the encoder picker.
    pub fn name(self) -> &'static str {
        match self {
            Self::NvencH264 => "NVIDIA NVENC H.264",
            Self::NvencHevc => "NVIDIA NVENC HEVC",
            Self::AmfH264 => "AMD AMF H.264",
            Self::AmfHevc => "AMD AMF HEVC",
            Self::CpuX264 => "CPU x264 (semua PC)",
            Self::CpuX265 => "CPU x265 HEVC (semua PC)",
        }
    }

    pub fn label(self, gpu_name: &str) -> String {
        match self {
            Self::CpuX264 => "CPU Software (x264)".to_owned(),
            Self::CpuX265 => "CPU Software (x265 HEVC)".to_owned(),
            _ => format!("{} ({gpu_name})", self.name()),
        }
    }

    pub fn presets(self) -> &'static [&'static str] {
        match self {
            Self::NvencH264 | Self::NvencHevc => &NVENC_PRESETS,
            Self::AmfH264 | Self::AmfHevc => &AMF_PRESETS,
            Self::CpuX264 | Self::CpuX265 => &CPU_PRESETS,
        }
    }

    pub fn default_preset(self) -> &'static str {
        match self {
            Self::NvencH264 | Self::NvencHevc => "p5",
            Self::AmfH264 | Self::AmfHevc => "quality",
            Self::CpuX264 => "veryfast",
            Self::CpuX265 => "medium",
        }
    }

    /// Video codec arguments. `preset` is checked against what the chosen encoder supports.
    pub fn args(self, preset: &str, quality: u32) -> Vec<String> {
        let quality = quality.to_string();
        let preset = if self.presets().contains(&preset) {
            preset
        } else {
            self.default_preset()
        };
        let mut args: Vec<&str> = match self {
            Self::NvencH264 | Self::NvencHevc => {
                let codec = if self == Self::NvencH264 {
                    "h264_nvenc"
                } else {
                    "hevc_nvenc"
                };
                vec![
                    "-c:v", codec, "-preset", preset, "-tune", "hq", "-rc", "vbr", "-cq", &quality,
                    "-b:v", "0", "-pix_fmt", "yuv420p",
                ]
            }
            Self::AmfH264 | Self::AmfHevc => {
                let codec = if self == Self::AmfH264 {
                    "h264_amf"
                } else {
                    "hevc_amf"
                };
                vec![
                    "-c:v", codec, "-quality", preset, "-rc", "cqp", "-qp_i", &quality, "-qp_p",
                    &quality, "-pix_fmt", "yuv420p",
                ]
            }
            Self::CpuX264 => vec![
                "-c:v", "libx264", "-preset", preset, "-crf", &quality, "-pix_fmt", "yuv420p",
                "-threads", "0",
            ],
            Self::CpuX265 => vec![
                "-c:v",
                "libx265",
                "-preset",
                preset,
                "-crf",
                &quality,
                "-pix_fmt",
                "yuv420p",
                "-x265-params",
                "log-level=error",
                "-threads",
                "0",
            ],
        };
        if self.is_hevc() {
            args.extend(["-tag:v", "hvc1"]);
        }
        args.into_iter().map(str::to_owned).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum VideoTarget {
    #[default]
    Custom,
    YouTubeStandard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum VideoCodec {
    #[default]
    Hevc,
    H264,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum VideoCompression {
    #[default]
    OriginalQuality,
    CompactSize,
}

/// Global video render preset from Settings (`Custom | Standar YouTube`, codec, compression).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoRenderSettings {
    pub target: VideoTarget,
    pub codec: VideoCodec,
    pub compression: VideoCompression,
}

impl VideoRenderSettings {
    pub fn effective_codec(self) -> VideoCodec {
        if self.target == VideoTarget::YouTubeStandard {
            VideoCodec::H264
        } else {
            self.codec
        }
    }

    pub fn resolve_encoder(self, hardware: &HardwareInfo) -> Encoder {
        match self.effective_codec() {
            VideoCodec::Hevc => Encoder::best_hevc_or_h264(hardware),
            VideoCodec::H264 => Encoder::best_available(hardware),
        }
    }

    pub fn resolve_preset(self, encoder: Encoder) -> &'static str {
        if self.compression == VideoCompression::CompactSize
            && matches!(encoder, Encoder::CpuX264 | Encoder::CpuX265)
        {
            return "medium";
        }
        encoder.default_preset()
    }

    pub fn resolve_quality(self, encoder: Encoder) -> u32 {
        match self.compression {
            VideoCompression::OriginalQuality => {
                if encoder.is_hevc() {
                    20
                } else {
                    18
                }
            }
            VideoCompression::CompactSize => {
                if encoder.is_hevc() {
                    28
                } else {
                    27
                }
            }
        }
    }

    pub fn resolve_audio_bitrate(self, preferred: &str) -> String {
        if self.target == VideoTarget::YouTubeStandard {
            return "192k".to_owned();
        }
        let trimmed = preferred.trim();
        if !trimmed.is_empty() {
            return trimmed.to_owned();
        }
        if self.compression == VideoCompression::CompactSize {
            "128k".to_owned()
        } else {
            "256k".to_owned()
        }
    }

    pub fn resolve_target_size(self, requested: (u32, u32)) -> (u32, u32) {
        if self.target != VideoTarget::YouTubeStandard {
            return requested;
        }
        if requested == (1280, 720) {
            (1280, 720)
        } else {
            (1920, 1080)
        }
    }

    /// True when Generate can mux the looping source video directly with `-c:v copy`.
    pub fn can_stream_copy(
        self,
        source_codec: &str,
        source_pix_fmt: &str,
        source_size: (u32, u32),
        target_size: (u32, u32),
    ) -> bool {
        if self.compression == VideoCompression::CompactSize
            || source_size != target_size
            || source_size.0 == 0
            || source_size.1 == 0
        {
            return false;
        }
        let is_h264 = source_codec.eq_ignore_ascii_case("h264");
        let is_hevc =
            source_codec.eq_ignore_ascii_case("hevc") || source_codec.eq_ignore_ascii_case("h265");
        if self.target == VideoTarget::YouTubeStandard {
            let is_8bit_420 = source_pix_fmt.eq_ignore_ascii_case("yuv420p");
            return is_h264
                && is_8bit_420
                && (source_size == (1920, 1080) || source_size == (1280, 720));
        }
        match self.effective_codec() {
            VideoCodec::H264 => is_h264,
            VideoCodec::Hevc => is_h264 || is_hevc,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hardware(nvenc: bool, amf: bool) -> HardwareInfo {
        HardwareInfo {
            gpu_name: "Test GPU".to_owned(),
            has_nvidia: nvenc,
            has_amd: amf,
            nvenc_h264: nvenc,
            nvenc_hevc: false,
            amf_h264: amf,
            amf_hevc: amf,
            cpu_x265: false,
            gpu_whisper: false,
        }
    }

    #[test]
    fn falls_back_to_cpu_without_gpu_support() {
        let none = hardware(false, false);
        assert_eq!(
            Encoder::resolve(Encoder::NvencH264, &none),
            Encoder::CpuX264
        );
        assert_eq!(Encoder::resolve(Encoder::AmfH264, &none), Encoder::CpuX264);
        let nvidia = hardware(true, false);
        assert_eq!(
            Encoder::resolve(Encoder::NvencH264, &nvidia),
            Encoder::NvencH264
        );
        assert_eq!(
            Encoder::resolve(Encoder::NvencHevc, &nvidia),
            Encoder::CpuX264
        );
    }

    #[test]
    fn best_available_prefers_nvidia_then_amd() {
        assert_eq!(
            Encoder::best_available(&hardware(true, true)),
            Encoder::NvencH264
        );
        assert_eq!(
            Encoder::best_available(&hardware(false, true)),
            Encoder::AmfH264
        );
        assert_eq!(
            Encoder::best_available(&hardware(false, false)),
            Encoder::CpuX264
        );
    }

    #[test]
    fn presets_are_normalized_per_encoder() {
        assert!(Encoder::CpuX264
            .args("p4", 22)
            .contains(&"veryfast".to_owned()));
        assert!(Encoder::CpuX264
            .args("slow", 22)
            .contains(&"slow".to_owned()));
        assert!(Encoder::NvencH264
            .args("medium", 20)
            .contains(&"p5".to_owned()));
        assert!(Encoder::NvencH264.args("p6", 20).contains(&"p6".to_owned()));
        assert!(Encoder::AmfH264
            .args("p4", 20)
            .contains(&"quality".to_owned()));
    }

    #[test]
    fn amf_uses_constant_quantizer() {
        let args = Encoder::AmfHevc.args("quality", 24);
        assert!(args.contains(&"hevc_amf".to_owned()));
        assert!(args.contains(&"hvc1".to_owned()));
        assert!(args.windows(2).any(|pair| pair == ["-qp_i", "24"]));
    }

    #[test]
    fn cpu_x265_args_enable_mp4_tag() {
        let args = Encoder::CpuX265.args("medium", 28);
        assert!(args.contains(&"libx265".to_owned()));
        assert!(args.contains(&"hvc1".to_owned()));
        assert!(args.windows(2).any(|pair| pair == ["-crf", "28"]));
    }

    #[test]
    fn youtube_standard_forces_h264_192k_and_16_9() {
        let hw = HardwareInfo {
            cpu_x265: true,
            ..hardware(true, false)
        };
        let settings = VideoRenderSettings {
            target: VideoTarget::YouTubeStandard,
            codec: VideoCodec::Hevc,
            compression: VideoCompression::OriginalQuality,
        };
        assert_eq!(settings.effective_codec(), VideoCodec::H264);
        assert_eq!(settings.resolve_encoder(&hw), Encoder::NvencH264);
        assert_eq!(settings.resolve_audio_bitrate("320k"), "192k");
        assert_eq!(settings.resolve_target_size((1280, 720)), (1280, 720));
        assert_eq!(settings.resolve_target_size((3840, 2160)), (1920, 1080));
        assert!(settings.can_stream_copy("h264", "yuv420p", (1920, 1080), (1920, 1080)));
        assert!(!settings.can_stream_copy("h264", "yuv420p10le", (1920, 1080), (1920, 1080)));
        assert!(!settings.can_stream_copy("hevc", "yuv420p", (1920, 1080), (1920, 1080)));
    }

    #[test]
    fn compact_size_disables_stream_copy_and_compresses() {
        let hw = HardwareInfo {
            cpu_x265: true,
            ..hardware(false, false)
        };
        let settings = VideoRenderSettings {
            target: VideoTarget::Custom,
            codec: VideoCodec::Hevc,
            compression: VideoCompression::CompactSize,
        };
        let encoder = settings.resolve_encoder(&hw);
        assert_eq!(encoder, Encoder::CpuX265);
        assert_eq!(settings.resolve_preset(encoder), "medium");
        assert_eq!(settings.resolve_quality(encoder), 28);
        assert!(!settings.can_stream_copy("hevc", "yuv420p", (1920, 1080), (1920, 1080)));
        assert_eq!(settings.resolve_audio_bitrate(""), "128k");
    }
}
