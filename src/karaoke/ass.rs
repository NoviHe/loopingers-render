use std::path::PathBuf;

use super::LyricLine;

const OUTLINE_WIDTH: &str = "3.0";
const SHADOW_DEPTH: &str = "2.0";
const INTRO_START_OFFSET: f64 = 0.5;
const INTRO_END_OFFSET: f64 = 5.5;
const INTRO_MIN_TRACK_SECONDS: f64 = 5.0;
const GAP_THRESHOLD_SECONDS: f64 = 0.05;
const MIN_WORD_SECONDS: f64 = 0.1;
const MAX_VISIBLE_TRACKLIST_ITEMS: usize = 12;

/// Mode tampilan daftar lagu yang dipilih pada video.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum PlaylistDisplayMode {
    /// Menampilkan lagu sebagai Visual (Daftar Lagu di Layar / Visualizer Spektrum) tanpa lirik karaoke.
    Visual,
    /// Menampilkan Lirik Lagu Karaoke saja.
    Lyrics,
    /// Menampilkan Keduanya: Visual (Daftar Lagu / Visualizer) sekaligus Lirik Lagu Karaoke.
    #[default]
    Both,
}

impl PlaylistDisplayMode {
    pub const ALL: [Self; 3] = [Self::Visual, Self::Lyrics, Self::Both];

    pub fn label(self) -> &'static str {
        match self {
            Self::Visual => "📊 Visual (Daftar Lagu / Visualizer)",
            Self::Lyrics => "🎤 Lirik Lagu",
            Self::Both => "✨ Keduanya (Visual + Lirik)",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::Visual => "Visual Saja",
            Self::Lyrics => "Lirik Lagu Saja",
            Self::Both => "Keduanya (Visual + Lirik)",
        }
    }

    pub fn includes_visual(self) -> bool {
        matches!(self, Self::Visual | Self::Both)
    }

    pub fn includes_lyrics(self) -> bool {
        matches!(self, Self::Lyrics | Self::Both)
    }
}

/// Posisi penempatan daftar lagu (tracklist visual) di layar video.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum TracklistPosition {
    #[default]
    TopLeft,
    TopRight,
    MiddleLeft,
    MiddleRight,
    Center,
}

impl TracklistPosition {
    pub const ALL: [Self; 5] = [
        Self::TopLeft,
        Self::TopRight,
        Self::MiddleLeft,
        Self::MiddleRight,
        Self::Center,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::TopLeft => "Kiri Atas",
            Self::TopRight => "Kanan Atas",
            Self::MiddleLeft => "Kiri Tengah",
            Self::MiddleRight => "Kanan Tengah",
            Self::Center => "Tengah Layar",
        }
    }

    pub fn ass_alignment(self) -> u8 {
        match self {
            Self::TopLeft => 7,
            Self::TopRight => 9,
            Self::MiddleLeft => 4,
            Self::MiddleRight => 6,
            Self::Center => 5,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntroBanner {
    pub font: String,
    pub size: u32,
    pub color: [u8; 3],
    pub x: f32,
    pub y: f32,
}

/// Subtitle options as sent by the UI; every field falls back to a default when absent.
#[derive(Debug, Default, Clone)]
pub struct StyleConfig {
    pub font_name: Option<String>,
    pub font_path: Option<PathBuf>,
    pub font_size: Option<u32>,
    pub normal_color: Option<String>,
    pub singing_color: Option<String>,
    pub outline_color: Option<String>,
    pub shadow_color: Option<String>,
    pub alignment: Option<u8>,
    pub margin_v: Option<i32>,
    pub pos_x: Option<f32>,
    pub pos_y: Option<f32>,
    pub show_intro: Option<bool>,
    pub intro_banner: Option<IntroBanner>,
    pub wipe_mode: Option<String>,
    pub show_lyrics: Option<bool>,
    pub show_tracklist: Option<bool>,
    pub tracklist_position: Option<TracklistPosition>,
    pub tracklist_font_size: Option<u32>,
    pub tracklist_title: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AssStyle {
    font_name: String,
    font_path: Option<PathBuf>,
    font_size: u32,
    normal_color: String,
    singing_color: String,
    outline_color: String,
    shadow_color: String,
    alignment: u8,
    margin_v: i32,
    pos_x: Option<f32>,
    pos_y: Option<f32>,
    show_intro: bool,
    intro_banner: Option<IntroBanner>,
    smooth_wipe: bool,
    show_lyrics: bool,
    show_tracklist: bool,
    tracklist_position: TracklistPosition,
    tracklist_font_size: u32,
    tracklist_title: String,
}

impl AssStyle {
    pub fn from_config(config: &StyleConfig, video_height: u32) -> Self {
        let text = |value: &Option<String>, default: &str| {
            value.clone().unwrap_or_else(|| default.to_owned())
        };
        Self {
            font_name: text(&config.font_name, "Impact"),
            font_path: config.font_path.clone(),
            font_size: config
                .font_size
                .unwrap_or(if video_height >= 1080 { 48 } else { 32 }),
            normal_color: text(&config.normal_color, "#FFFFFF"),
            singing_color: text(&config.singing_color, "#FFD700"),
            outline_color: text(&config.outline_color, "#000000"),
            shadow_color: text(&config.shadow_color, "#000000"),
            alignment: config.alignment.unwrap_or(2),
            margin_v: config.margin_v.unwrap_or(70),
            pos_x: config.pos_x,
            pos_y: config.pos_y,
            show_intro: config.show_intro.unwrap_or(true),
            intro_banner: config.intro_banner.clone(),
            smooth_wipe: config.wipe_mode.as_deref().unwrap_or("smooth") == "smooth",
            show_lyrics: config.show_lyrics.unwrap_or(true),
            show_tracklist: config.show_tracklist.unwrap_or(false),
            tracklist_position: config.tracklist_position.unwrap_or_default(),
            tracklist_font_size: config
                .tracklist_font_size
                .unwrap_or(if video_height >= 1080 { 26 } else { 18 }),
            tracklist_title: text(&config.tracklist_title, "DAFTAR LAGU"),
        }
    }
}

/// A playlist entry as the subtitle file needs it, with title and artist already cleaned.
#[derive(Debug)]
pub struct SubtitleTrack {
    pub title: String,
    pub artist: String,
    pub duration: f64,
    pub lyrics: Vec<LyricLine>,
}

/// `#RRGGBB` (or `#RGB`) to the ASS `&HAABBGGRR&` notation; invalid input becomes white.
pub fn ass_color(hex: &str, alpha: u8) -> String {
    let digits = hex.trim_start_matches('#');
    let expanded: String = if digits.len() == 3 {
        digits.chars().flat_map(|c| [c, c]).collect()
    } else {
        digits.to_owned()
    };
    let valid = expanded.len() == 6 && expanded.chars().all(|c| c.is_ascii_hexdigit());
    let rgb = if valid { expanded } else { "FFFFFF".to_owned() }.to_uppercase();
    format!("&H{alpha:02X}{}{}{}&", &rgb[4..6], &rgb[2..4], &rgb[0..2])
}

/// `H:MM:SS.cc`
pub fn ass_time(seconds: f64) -> String {
    let centiseconds = (seconds.max(0.0) * 100.0).round() as u64;
    let hours = centiseconds / 360_000;
    let minutes = centiseconds / 6000 % 60;
    let secs = centiseconds / 100 % 60;
    format!("{hours}:{minutes:02}:{secs:02}.{:02}", centiseconds % 100)
}

fn header(width: u32, height: u32, style: &AssStyle) -> String {
    let primary = ass_color(&style.normal_color, 0);
    let secondary = ass_color(&style.singing_color, 0);
    let outline = ass_color(&style.outline_color, 0);
    let back = ass_color(&style.shadow_color, 100);
    let title_primary = ass_color("#FFFFFF", 0);
    let font = &style.font_name;
    let size = style.font_size;
    let active_size = (f64::from(size) * 1.05) as u32;
    let title_size = (f64::from(size) * 0.9) as u32;
    let (alignment, margin_v) = (style.alignment, style.margin_v);
    let tl_size = style.tracklist_font_size;
    let tl_align = style.tracklist_position.ass_alignment();

    format!(
        "[Script Info]\n\
         ; Script generated by LoopKaraoke Studio\n\
         Title: LoopKaraoke Video\n\
         ScriptType: v4.00+\n\
         WrapStyle: 0\n\
         ScaledBorderAndShadow: yes\n\
         YCbCr Matrix: TV.709\n\
         PlayResX: {width}\n\
         PlayResY: {height}\n\
         \n\
         [V4+ Styles]\n\
         Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n\
         Style: Karaoke,{font},{size},{primary},{secondary},{outline},{back},-1,0,0,0,100,100,1,0,1,{OUTLINE_WIDTH},{SHADOW_DEPTH},{alignment},30,30,{margin_v},1\n\
         Style: KaraokeActive,{font},{active_size},{secondary},{secondary},{outline},{back},-1,0,0,0,100,100,1,0,1,{OUTLINE_WIDTH},{SHADOW_DEPTH},{alignment},30,30,{margin_v},1\n\
         Style: SongTitle,{font},{title_size},{title_primary},{secondary},{outline},{back},-1,0,0,0,100,100,1,0,1,2,2,7,50,50,50,1\n\
         Style: TrackList,{font},{tl_size},{title_primary},{secondary},{outline},{back},-1,0,0,0,100,100,1,0,1,2.2,1.8,{tl_align},45,45,45,1\n\
         \n\
         [Events]\n\
         Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n"
    )
}

/// Override-block braces would be read as ASS tags, so lyrics never carry them.
fn escape_text(text: &str) -> String {
    text.replace('{', "(").replace('}', ")").replace('\n', " ")
}

fn karaoke_event(
    line: &LyricLine,
    offset: f64,
    smooth_wipe: bool,
    pos: Option<(i32, i32)>,
) -> String {
    let start = line.start + offset;
    let prefix = format!(
        "Dialogue: 0,{},{},Karaoke,,0,0,0,,",
        ass_time(start),
        ass_time(line.end + offset)
    );
    let pos_tag = if let Some((x, y)) = pos {
        format!("{{\\an5\\pos({x},{y})}}")
    } else {
        String::new()
    };
    if line.words.is_empty() {
        return format!("{prefix}{pos_tag}{}", escape_text(&line.text));
    }

    let wipe = if smooth_wipe { "kf" } else { "k" };
    let mut text = String::new();
    let mut cursor = start;
    for word in &line.words {
        let word_start = word.start + offset;
        let word_end = word.end + offset;
        let gap = word_start - cursor;
        if gap > GAP_THRESHOLD_SECONDS {
            text.push_str(&format!("{{\\k{}}}", (gap * 100.0).round() as i64));
        }
        let centiseconds = (((word_end - word_start).max(MIN_WORD_SECONDS)) * 100.0).round() as i64;
        text.push_str(&format!(
            "{{\\{wipe}{}}}{} ",
            centiseconds.max(10),
            escape_text(&word.word)
        ));
        cursor = word_end;
    }
    format!("{prefix}{pos_tag}{}", text.trim_end())
}

fn intro_event(track: &SubtitleTrack, offset: f64, font_size: u32) -> String {
    let artist_size = (f64::from(font_size) * 0.65) as u32;
    let title = escape_text(&track.title);
    let text =
        if !track.artist.is_empty() && track.artist.to_lowercase() != track.title.to_lowercase() {
            format!(
                "{{\\fad(600,600)}}🎵 {title} \\N {{\\fs{artist_size}}}{}",
                escape_text(&track.artist)
            )
        } else {
            format!("{{\\fad(600,600)}}🎵 {title}")
        };
    format!(
        "Dialogue: 1,{},{},SongTitle,,0,0,0,,{text}",
        ass_time(offset + INTRO_START_OFFSET),
        ass_time(offset + INTRO_END_OFFSET)
    )
}

fn tracklist_event(
    tracks: &[SubtitleTrack],
    active_idx: usize,
    offset: f64,
    style: &AssStyle,
) -> String {
    let total = tracks.len();
    let (start_idx, end_idx) = if total <= MAX_VISIBLE_TRACKLIST_ITEMS {
        (0, total)
    } else {
        let half = MAX_VISIBLE_TRACKLIST_ITEMS / 2;
        let mut s = active_idx.saturating_sub(half);
        if s + MAX_VISIBLE_TRACKLIST_ITEMS > total {
            s = total - MAX_VISIBLE_TRACKLIST_ITEMS;
        }
        (s, s + MAX_VISIBLE_TRACKLIST_ITEMS)
    };

    let secondary = ass_color(&style.singing_color, 0);
    let active_size = (f64::from(style.tracklist_font_size) * 1.08) as u32;
    let header_size = (f64::from(style.tracklist_font_size) * 1.12) as u32;

    let mut lines = Vec::with_capacity(end_idx - start_idx + 1);
    let header_title = escape_text(style.tracklist_title.trim());
    if !header_title.is_empty() {
        lines.push(format!(
            "{{\\fs{header_size}\\c{secondary}}}🎵 {header_title}{{\\r}}"
        ));
    }

    for (i, item) in tracks.iter().enumerate().take(end_idx).skip(start_idx) {
        let raw_title = escape_text(&item.title);
        let short_title = if raw_title.chars().count() > 36 {
            format!("{}...", raw_title.chars().take(33).collect::<String>())
        } else {
            raw_title
        };
        if i == active_idx {
            lines.push(format!(
                "{{\\c{secondary}\\fs{active_size}}}▶ {:02}. {short_title}{{\\r}}",
                i + 1
            ));
        } else {
            lines.push(format!(
                "{{\\c&H00E0E0E0&\\alpha&H40&}}   {:02}. {short_title}{{\\r}}",
                i + 1
            ));
        }
    }

    format!(
        "Dialogue: 2,{},{},TrackList,,0,0,0,,{}",
        ass_time(offset),
        ass_time(offset + tracks[active_idx].duration),
        lines.join("\\N")
    )
}

/// Builds the complete `.ass` script for the playlist. Each track's lyrics are shifted by the
/// summed duration of the tracks before it.
pub fn build_ass(tracks: &[SubtitleTrack], width: u32, height: u32, style: &AssStyle) -> String {
    let mut script = header(width, height, style);
    let mut offset = 0.0;
    for (idx, track) in tracks.iter().enumerate() {
        if style.show_tracklist && track.duration > 0.0 {
            script.push_str(&tracklist_event(tracks, idx, offset, style));
            script.push('\n');
        }
        if let Some(banner) = style
            .intro_banner
            .as_ref()
            .filter(|_| style.show_intro && track.duration > 0.0)
        {
            let x = (width as f32 * banner.x.clamp(0.0, 100.0) / 100.0).round();
            let y = (height as f32 * banner.y.clamp(0.0, 100.0) / 100.0).round();
            let [r, g, b] = banner.color;
            let font = escape_text(&banner.font);
            let size = banner.size;
            let mut text = escape_text(&track.title);
            if !track.artist.trim().is_empty() && !track.artist.eq_ignore_ascii_case(&track.title) {
                text.push_str(&format!(
                    "\\N{{\\fs{}}}{}",
                    size * 65 / 100,
                    escape_text(&track.artist)
                ));
            }
            let start = ass_time(offset);
            let end = ass_time(offset + track.duration.min(5.0));
            script.push_str(&format!(
                "Dialogue: 1,{start},{end},SongTitle,,0,0,0,,{{\\an5\\pos({x},{y})\\fn{font}\\fs{size}\\c&H{b:02X}{g:02X}{r:02X}&}}{text}\n"
            ));
        } else if !style.show_tracklist
            && style.show_intro
            && track.duration > INTRO_MIN_TRACK_SECONDS
        {
            script.push_str(&intro_event(track, offset, style.font_size));
            script.push('\n');
        }
        if style.show_lyrics {
            let pos = match (style.pos_x, style.pos_y) {
                (Some(x), Some(y)) => Some((
                    (width as f32 * x / 100.0).round() as i32,
                    (height as f32 * y / 100.0).round() as i32,
                )),
                _ => None,
            };
            for line in &track.lyrics {
                script.push_str(&karaoke_event(line, offset, style.smooth_wipe, pos));
                script.push('\n');
            }
        }
        offset += track.duration;
    }
    script
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::karaoke::LyricWord;

    fn word(text: &str, start: f64, end: f64) -> LyricWord {
        LyricWord {
            word: text.to_owned(),
            start,
            end,
        }
    }

    #[test]
    fn colors_are_converted_to_bgr() {
        assert_eq!(ass_color("#FFD700", 0), "&H0000D7FF&");
        assert_eq!(ass_color("abc", 100), "&H64CCBBAA&");
        assert_eq!(ass_color("nonsense", 0), "&H00FFFFFF&");
    }

    #[test]
    fn times_use_centiseconds() {
        assert_eq!(ass_time(0.0), "0:00:00.00");
        assert_eq!(ass_time(3725.456), "1:02:05.46");
        assert_eq!(ass_time(-1.0), "0:00:00.00");
    }

    #[test]
    fn karaoke_line_has_gap_and_wipe_tags() {
        let line = LyricLine {
            start: 1.0,
            end: 3.0,
            text: "hi there".to_owned(),
            words: vec![word("hi", 1.5, 2.0), word("there", 2.0, 3.0)],
        };
        assert_eq!(
            karaoke_event(&line, 10.0, true, None),
            "Dialogue: 0,0:00:11.00,0:00:13.00,Karaoke,,0,0,0,,{\\k50}{\\kf50}hi {\\kf100}there"
        );
        assert!(karaoke_event(&line, 0.0, false, None).contains("{\\k100}there"));

        let positioned = karaoke_event(&line, 10.0, true, Some((960, 918)));
        assert!(positioned.contains("{\\an5\\pos(960,918)}"));
    }

    #[test]
    fn script_offsets_lyrics_and_adds_intro_for_long_tracks() {
        let lyric = LyricLine {
            start: 0.0,
            end: 1.0,
            text: "x".to_owned(),
            words: vec![word("x", 0.0, 1.0)],
        };
        let tracks = vec![
            SubtitleTrack {
                title: "One".into(),
                artist: "Band".into(),
                duration: 60.0,
                lyrics: vec![],
            },
            SubtitleTrack {
                title: "Two".into(),
                artist: String::new(),
                duration: 3.0,
                lyrics: vec![lyric],
            },
        ];
        let style = AssStyle::from_config(&StyleConfig::default(), 1080);
        let script = build_ass(&tracks, 1920, 1080, &style);
        assert!(script.contains("PlayResX: 1920"));
        assert!(script.contains("Dialogue: 1,0:00:00.50,0:00:05.50,SongTitle"));
        assert!(script.contains("Dialogue: 0,0:01:00.00,0:01:01.00,Karaoke"));
        assert_eq!(script.matches("SongTitle,,").count(), 1);
    }

    #[test]
    fn tracklist_and_lyrics_modes_are_respected() {
        let lyric = LyricLine {
            start: 0.0,
            end: 1.0,
            text: "hello".to_owned(),
            words: vec![word("hello", 0.0, 1.0)],
        };
        let tracks = vec![SubtitleTrack {
            title: "First Song".into(),
            artist: "Artist".into(),
            duration: 10.0,
            lyrics: vec![lyric],
        }];

        // Visual only: show_tracklist = true, show_lyrics = false
        let visual_cfg = StyleConfig {
            show_lyrics: Some(false),
            show_tracklist: Some(true),
            tracklist_position: Some(TracklistPosition::TopLeft),
            ..Default::default()
        };
        let visual_style = AssStyle::from_config(&visual_cfg, 1080);
        let visual_script = build_ass(&tracks, 1920, 1080, &visual_style);
        assert!(visual_script.contains("Dialogue: 2,0:00:00.00,0:00:10.00,TrackList"));
        assert!(visual_script.contains("▶ 01. First Song"));
        assert!(!visual_script.contains("Dialogue: 0,"));

        // Both: show_tracklist = true, show_lyrics = true
        let both_cfg = StyleConfig {
            show_lyrics: Some(true),
            show_tracklist: Some(true),
            ..Default::default()
        };
        let both_style = AssStyle::from_config(&both_cfg, 1080);
        let both_script = build_ass(&tracks, 1920, 1080, &both_style);
        assert!(both_script.contains("TrackList"));
        assert!(both_script.contains("Dialogue: 0,0:00:00.00,0:00:01.00,Karaoke"));
    }
}
