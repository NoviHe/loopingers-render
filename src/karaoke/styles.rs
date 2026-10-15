#[derive(Debug)]
pub struct Theme {
    pub name: &'static str,
    pub normal_color: &'static str,
    pub singing_color: &'static str,
    pub outline_color: &'static str,
    pub shadow_color: &'static str,
}

pub static THEMES: [(&str, Theme); 5] = [
    (
        "golden",
        Theme {
            name: "Golden Glow (Kuning Emas)",
            normal_color: "#E0E0E0",
            singing_color: "#FFD700",
            outline_color: "#000000",
            shadow_color: "#202020",
        },
    ),
    (
        "neon_cyan",
        Theme {
            name: "Neon Cyan (Biru Neon)",
            normal_color: "#FFFFFF",
            singing_color: "#00F0FF",
            outline_color: "#0A192F",
            shadow_color: "#020C1B",
        },
    ),
    (
        "sunset_pink",
        Theme {
            name: "Sunset Pink (Merah Muda Menyala)",
            normal_color: "#FFF0F5",
            singing_color: "#FF2A85",
            outline_color: "#1A0010",
            shadow_color: "#0D0008",
        },
    ),
    (
        "emerald",
        Theme {
            name: "Emerald Spark (Hijau Neon)",
            normal_color: "#F0FFF0",
            singing_color: "#39FF14",
            outline_color: "#051A05",
            shadow_color: "#000000",
        },
    ),
    (
        "classic",
        Theme {
            name: "Classic Karaoke (Putih & Biru)",
            normal_color: "#FFFFFF",
            singing_color: "#1E90FF",
            outline_color: "#000000",
            shadow_color: "#101010",
        },
    ),
];
