pub type Resolution = (u32, u32);

const DEFAULT_ASPECT: &str = "16:9";
const DEFAULT_RESOLUTION: Resolution = (1920, 1080);

pub const ASPECT_RATIOS: [&str; 3] = ["16:9", "9:16", "1:1"];
pub const RESOLUTION_LABELS: [&str; 3] = ["1080p", "720p", "4k"];

static RESOLUTIONS: [(&str, [(&str, Resolution); 3]); 3] = [
    (
        "16:9",
        [
            ("1080p", (1920, 1080)),
            ("720p", (1280, 720)),
            ("4k", (3840, 2160)),
        ],
    ),
    (
        "9:16",
        [
            ("1080p", (1080, 1920)),
            ("720p", (720, 1280)),
            ("4k", (2160, 3840)),
        ],
    ),
    (
        "1:1",
        [
            ("1080p", (1080, 1080)),
            ("720p", (720, 720)),
            ("4k", (2160, 2160)),
        ],
    ),
];

/// Output size for an aspect ratio and quality label; unknown labels fall back to 16:9 / 1080p.
pub fn dimensions(aspect_ratio: &str, resolution: &str) -> Resolution {
    let sizes = RESOLUTIONS
        .iter()
        .find(|(aspect, _)| *aspect == aspect_ratio)
        .or_else(|| {
            RESOLUTIONS
                .iter()
                .find(|(aspect, _)| *aspect == DEFAULT_ASPECT)
        })
        .map(|(_, sizes)| sizes);
    sizes
        .and_then(|sizes| sizes.iter().find(|(label, _)| *label == resolution))
        .map_or(DEFAULT_RESOLUTION, |(_, size)| *size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_and_unknown_presets_resolve() {
        assert_eq!(dimensions("9:16", "720p"), (720, 1280));
        assert_eq!(dimensions("1:1", "4k"), (2160, 2160));
        assert_eq!(dimensions("21:9", "720p"), (1280, 720));
        assert_eq!(dimensions("16:9", "8k"), (1920, 1080));
    }
}
