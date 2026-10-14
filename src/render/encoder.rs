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
}

impl Encoder {
    pub const ALL: [Self; 5] = [
        Self::NvencH264,
        Self::NvencHevc,
        Self::AmfH264,
        Self::AmfHevc,
        Self::CpuX264,
    ];

    pub fn is_available(self, hardware: &HardwareInfo) -> bool {
        match self {
            Self::NvencH264 => hardware.nvenc_h264,
            Self::NvencHevc => hardware.nvenc_hevc,
            Self::AmfH264 => hardware.amf_h264,
            Self::AmfHevc => hardware.amf_hevc,
            Self::CpuX264 => true,
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

    /// Short name for the encoder picker.
    pub fn name(self) -> &'static str {
        match self {
            Self::NvencH264 => "NVIDIA NVENC H.264",
            Self::NvencHevc => "NVIDIA NVENC HEVC",
            Self::AmfH264 => "AMD AMF H.264",
            Self::AmfHevc => "AMD AMF HEVC",
            Self::CpuX264 => "CPU x264 (semua PC)",
        }
    }

    pub fn label(self, gpu_name: &str) -> String {
        match self {
            Self::CpuX264 => "CPU Software (x264)".to_owned(),
            _ => format!("{} ({gpu_name})", self.name()),
        }
    }

    pub fn presets(self) -> &'static [&'static str] {
        match self {
            Self::NvencH264 | Self::NvencHevc => &NVENC_PRESETS,
            Self::AmfH264 | Self::AmfHevc => &AMF_PRESETS,
            Self::CpuX264 => &CPU_PRESETS,
        }
    }

    pub fn default_preset(self) -> &'static str {
        match self {
            Self::NvencH264 | Self::NvencHevc => "p5",
            Self::AmfH264 | Self::AmfHevc => "quality",
            Self::CpuX264 => "veryfast",
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
        let args: Vec<&str> = match self {
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
        };
        args.into_iter().map(str::to_owned).collect()
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
        assert!(args.windows(2).any(|pair| pair == ["-qp_i", "24"]));
    }
}
