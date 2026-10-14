use std::path::{Path, PathBuf};

use super::graph::FilterGraph;

/// A visual effect offered to the user: an overlay video blended on top or a filter chain.
#[derive(Debug)]
pub struct EffectDef {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    overlay_file: Option<&'static str>,
    opacity: f64,
    vf: Option<&'static str>,
}

impl EffectDef {
    const fn none(id: &'static str, name: &'static str, desc: &'static str) -> Self {
        Self {
            id,
            name,
            desc,
            overlay_file: None,
            opacity: 1.0,
            vf: None,
        }
    }

    const fn overlay(
        id: &'static str,
        name: &'static str,
        desc: &'static str,
        file: &'static str,
        opacity: f64,
    ) -> Self {
        Self {
            overlay_file: Some(file),
            opacity,
            ..Self::none(id, name, desc)
        }
    }

    const fn filter(
        id: &'static str,
        name: &'static str,
        desc: &'static str,
        vf: &'static str,
    ) -> Self {
        Self {
            vf: Some(vf),
            ..Self::none(id, name, desc)
        }
    }
}

pub static EFFECTS: [EffectDef; 50] = [
    EffectDef::none("none", "Tanpa Efek (Asli)", "Tampilan video murni tanpa efek"),
    EffectDef::overlay("rain", "Hujan (Rain)", "Hujan deras miring dengan tiga lapis kedalaman dan kabut tipis", "rain.mp4", 0.9),
    EffectDef::overlay("snow", "Salju (Snow)", "Salju lembut turun dengan bokeh besar di depan", "snow.mp4", 0.9),
    EffectDef::overlay("fire", "Bara Api (Fire Embers)", "Percikan bara membara naik, dari putih panas ke merah", "fire.mp4", 0.95),
    EffectDef::overlay("bokeh", "Bokeh Sinematik", "Lingkaran cahaya hangat melayang dengan tepi terang", "bokeh.mp4", 0.85),
    EffectDef::overlay("light_leak", "Light Leak (Sunflare)", "Kilau cahaya sinematik hangat menyapu sudut layar", "light_leak.mp4", 0.8),
    EffectDef::overlay("sparkles", "Kilau Bintang (Sparkles)", "Bintang empat sudut berkedip lembut seperti glitter", "sparkles.mp4", 0.95),
    EffectDef::overlay("fireflies", "Kunang-kunang", "Cahaya kunang-kunang hijau melayang dan berdenyut", "fireflies.mp4", 0.95),
    EffectDef::overlay("dust", "Debu Emas (Golden Dust)", "Partikel debu emas halus melayang di udara", "dust.mp4", 0.9),
    EffectDef::overlay("stars", "Langit Bintang", "Bintang berkelip dan bintang jatuh sesekali", "stars.mp4", 0.95),
    EffectDef::overlay("confetti", "Konfeti", "Konfeti warna-warni berputar jatuh untuk suasana perayaan", "confetti.mp4", 0.9),
    EffectDef::overlay("hearts", "Hati Melayang (Hearts)", "Hati merah muda naik berayun dengan cahaya lembut", "hearts.mp4", 0.9),
    EffectDef::overlay("petals", "Kelopak Sakura", "Kelopak bunga jatuh berputar tertiup angin", "petals.mp4", 0.9),
    EffectDef::overlay("mist", "Kabut (Mist)", "Kabut tipis bergerak pelan, memberi kesan misterius", "mist.mp4", 0.8),
    EffectDef::overlay("aurora", "Aurora", "Tirai cahaya aurora hijau-ungu bergelombang", "aurora.mp4", 0.8),
    EffectDef::overlay("rays", "Sinar Cahaya (God Rays)", "Berkas sinar hangat dari sudut atas, bergoyang lembut", "rays.mp4", 0.8),
    EffectDef::overlay("lens_flare", "Lens Flare Sinematik", "Kilatan lensa anamorfik biru menyapu layar", "lens_flare.mp4", 0.9),
    EffectDef::overlay("drizzle", "Gerimis", "Hujan tipis lembut yang menenangkan", "drizzle.mp4", 0.9),
    EffectDef::overlay("rose_petals", "Kelopak Mawar", "Kelopak mawar merah jatuh berputar romantis", "rose_petals.mp4", 0.9),
    EffectDef::overlay("shooting_stars", "Bintang Jatuh", "Meteor berekor panjang melintas bergantian", "shooting_stars.mp4", 0.95),
    EffectDef::overlay("fireworks", "Kembang Api", "Roket meluncur dan meledak jadi percikan warna-warni", "fireworks.mp4", 0.95),
    EffectDef::overlay("lightning", "Petir", "Kilatan petir bercabang dengan cahaya langit", "lightning.mp4", 0.9),
    EffectDef::overlay("bubbles", "Gelembung", "Gelembung sabun transparan naik berkilau", "bubbles.mp4", 0.9),
    EffectDef::overlay("leaves", "Daun Musim Gugur", "Daun oranye dan merah berguguran tertiup angin", "leaves.mp4", 0.9),
    EffectDef::overlay("smoke", "Asap Tipis", "Asap lembut mengepul naik perlahan", "smoke.mp4", 0.8),
    EffectDef::overlay("music_notes", "Not Musik", "Not balok warna-warni melayang naik", "music_notes.mp4", 0.95),
    EffectDef::overlay("ripples", "Riak Air", "Lingkaran riak menyebar dari beberapa titik", "ripples.mp4", 0.9),
    EffectDef::overlay("film_dust", "Debu Film", "Bintik dan goresan film analog yang berkedip", "film_dust.mp4", 0.9),
    EffectDef::overlay("matrix", "Hujan Digital", "Aliran kode hijau jatuh bergaya Matrix", "matrix.mp4", 0.9),
    EffectDef::overlay("snowflakes", "Kristal Salju", "Kristal salju enam sisi berputar jatuh", "snowflakes.mp4", 0.95),
    EffectDef::overlay("gold_rain", "Hujan Emas", "Tetesan emas berkilau jatuh seperti glitter", "gold_rain.mp4", 0.95),
    EffectDef::overlay("stage_lights", "Lampu Panggung", "Sorot lampu panggung berwarna menyapu layar", "stage_lights.mp4", 0.85),
    EffectDef::overlay("equalizer", "Equalizer Musik", "Bar equalizer neon menari di bagian bawah", "equalizer.mp4", 0.9),
    EffectDef::overlay("rainbow", "Pelangi", "Pelangi lembut membentang di langit", "rainbow.mp4", 0.8),
    EffectDef::overlay("balloons", "Balon Udara", "Balon warna-warni terbang naik dengan tali", "balloons.mp4", 0.95),
    EffectDef::overlay("nebula", "Nebula Galaksi", "Awan galaksi ungu-biru yang bergerak pelan", "nebula.mp4", 0.8),
    EffectDef::overlay("synthwave", "Synthwave Retro", "Matahari neon dan grid perspektif ala 80-an", "synthwave.mp4", 0.9),
    EffectDef::overlay("light_streaks", "Garis Cahaya", "Garis cahaya anamorfik horizontal melintas", "light_streaks.mp4", 0.9),
    EffectDef::filter("color_grading", "Color Grading (Teal & Orange)", "Grading warna blockbuster Hollywood profesional", "colorbalance=rs=0.08:gs=-0.02:bs=-0.08:rh=-0.06:gh=0.02:bh=0.08:rm=0.04:gm=0:bm=-0.03,eq=saturation=1.18:contrast=1.10:brightness=-0.01"),
    EffectDef::filter("warm", "Hangat (Golden Hour)", "Nuansa emas hangat seperti sore hari", "colorbalance=rs=0.10:gs=0.02:bs=-0.10:rm=0.08:bm=-0.08:rh=0.05:bh=-0.06,eq=saturation=1.12:contrast=1.04"),
    EffectDef::filter("cool", "Dingin (Moody Blue)", "Nuansa biru dingin dan sendu", "colorbalance=rs=-0.08:bs=0.10:rm=-0.05:bm=0.08:bh=0.05,eq=saturation=0.92:contrast=1.06:brightness=-0.02"),
    EffectDef::filter("vintage", "Film Vintage", "Warna pudar ala film lama dengan butiran halus", "curves=preset=vintage,noise=alls=7:allf=t,vignette=PI/5"),
    EffectDef::filter("sepia", "Sepia", "Warna cokelat klasik foto jadul", "colorchannelmixer=.393:.769:.189:0:.349:.686:.168:0:.272:.534:.131"),
    EffectDef::filter("bw", "Hitam Putih Kontras", "Hitam putih dramatis dengan kontras tinggi", "hue=s=0,eq=contrast=1.18:brightness=-0.02,vignette=PI/5"),
    EffectDef::filter("dreamy", "Dreamy Glow", "Cahaya lembut berkabut ala mimpi", "[{in}]split[d1][d2];[d2]gblur=sigma=16,eq=brightness=0.02:saturation=1.15[d3];[d1][d3]blend=all_mode=screen:all_opacity=0.45"),
    EffectDef::filter("bloom", "Bloom Terang", "Area terang memendar bercahaya", "[{in}]split[d1][d2];[d2]eq=brightness=-0.18:contrast=1.7,gblur=sigma=22[d3];[d1][d3]blend=all_mode=screen:all_opacity=0.7"),
    EffectDef::filter("cinema", "Sinema Letterbox", "Bar hitam atas-bawah dan vignette ala bioskop", "vignette=PI/5,drawbox=x=0:y=0:w=iw:h=ih*0.1:color=black:t=fill,drawbox=x=0:y=ih*0.9:w=iw:h=ih*0.1:color=black:t=fill"),
    EffectDef::filter("pulse", "Denyut Cahaya", "Kecerahan berdenyut pelan mengikuti irama", "eq=brightness='0.045*sin(2*PI*t*1.6)':contrast='1+0.04*sin(2*PI*t*1.6)':eval=frame"),
    EffectDef::filter("vhs", "VHS Retro (90s Analog)", "Scanline televisi tabung, glitch RGB & noise pita analog", "rgbashift=rh=-3:bh=3:rv=0:bv=0,drawgrid=width=iw:height=4:thickness=1:color=black@0.22,noise=alls=13:allf=t+u,eq=contrast=1.08:saturation=0.88:brightness=0.02,vignette=PI/4"),
    EffectDef::filter("glitch", "Glitch Digital (Cyberpunk)", "Distorsi RGB split cybernetic & kedipan sinyal digital", "rgbashift=rh=12:bh=-12:rv=2:enable='lt(mod(n,48),4)',noise=alls=35:allf=t+u:enable='lt(mod(n,48),4)',noise=alls=4:allf=t+u:enable='gte(mod(n,48),4)'"),
];

/// An effect that is ready to be applied to a render.
#[derive(Debug, Clone)]
pub enum AppliedEffect {
    None,
    Filter(&'static str),
    Overlay { path: PathBuf, opacity: f64 },
}

impl AppliedEffect {
    /// Unknown ids mean "no effect". An overlay whose video file is missing is skipped too.
    pub fn resolve(id: &str, effects_dir: &Path) -> Self {
        let Some(def) = EFFECTS.iter().find(|def| def.id == id) else {
            return Self::None;
        };
        match (def.vf, def.overlay_file) {
            (Some(vf), _) => Self::Filter(vf),
            (None, Some(file)) => {
                let path = effects_dir.join(file);
                if path.is_file() {
                    Self::Overlay {
                        path,
                        opacity: def.opacity,
                    }
                } else {
                    eprintln!(
                        "Efek overlay '{id}' dilewati: {} tidak ditemukan",
                        path.display()
                    );
                    Self::None
                }
            }
            (None, None) => Self::None,
        }
    }

    /// FFmpeg arguments that add the overlay video as an extra looping input.
    pub fn input_args(&self) -> Vec<std::ffi::OsString> {
        match self {
            Self::Overlay { path, .. } => vec![
                "-stream_loop".into(),
                "-1".into(),
                "-i".into(),
                path.as_os_str().to_owned(),
            ],
            _ => Vec::new(),
        }
    }

    /// Adds the effect to `graph`; `overlay_input` is the index of the overlay's FFmpeg input.
    ///
    /// Overlays are blended in RGB: blending the planes of a YUV picture would also push the
    /// chroma planes towards a purple cast.
    pub fn apply(&self, graph: &mut FilterGraph, overlay_input: usize, width: u32, height: u32) {
        match self {
            Self::None => {}
            // A filter that needs several branches names its input with the `{in}` placeholder.
            Self::Filter(vf) if vf.contains("{in}") => {
                graph.stage(|input| vf.replace("{in}", input));
            }
            Self::Filter(vf) => graph.chain(vf),
            Self::Overlay { opacity, .. } => graph.stage(|input| {
                format!(
                    "[{overlay_input}:v]scale={width}:{height}:force_original_aspect_ratio=increase,\
                     crop={width}:{height},format=gbrp[ov];\
                     [{input}]format=gbrp[bg];\
                     [bg][ov]blend=all_mode=screen:all_opacity={opacity},format=yuv420p"
                )
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_effects_resolve_to_none() {
        assert!(matches!(
            AppliedEffect::resolve("sparkle", Path::new(".")),
            AppliedEffect::None
        ));
    }

    #[test]
    fn filters_resolve_without_assets() {
        assert!(matches!(
            AppliedEffect::resolve("vhs", Path::new("missing")),
            AppliedEffect::Filter(_)
        ));
    }

    #[test]
    fn overlay_blends_on_the_requested_input() {
        let effect = AppliedEffect::Overlay {
            path: PathBuf::from("rain.mp4"),
            opacity: 0.85,
        };
        let mut graph = FilterGraph::new("0:v");
        effect.apply(&mut graph, 2, 1280, 720);
        assert_eq!(
            graph.finish("null"),
            "[2:v]scale=1280:720:force_original_aspect_ratio=increase,crop=1280:720,format=gbrp[ov];[0:v]format=gbrp[bg];[bg][ov]blend=all_mode=screen:all_opacity=0.85,format=yuv420p[s1];[s1]null[outv]"
        );
    }
}
