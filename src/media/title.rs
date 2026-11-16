use std::sync::LazyLock;

use regex::Regex;

struct Patterns {
    extension: Regex,
    hashed_prefix: Regex,
    bare_prefix: Regex,
    track_number: Regex,
    copy_index: Regex,
    media_tag: Regex,
    trailing_tag: Regex,
    by_artist: Regex,
    dangling_by: Regex,
    dash: Regex,
    spaces: Regex,
}

static PATTERNS: LazyLock<Patterns> = LazyLock::new(|| {
    let compile = |pattern: &str| Regex::new(pattern).expect("title pattern is valid");
    Patterns {
        extension: compile(r"(?i)\.(?:mp3|wav|m4a|flac|ogg|aac|opus|mp4|webm|mkv)$"),
        hashed_prefix: compile(r"(?i)^(?:audio|video)_[0-9a-z]+_"),
        bare_prefix: compile(r"(?i)^(?:audio|video)_"),
        track_number: compile(r"^\d+[.\-\s]+"),
        copy_index: compile(r"[(\[{]\s*\d+\s*[)\]}]"),
        media_tag: compile(
            r"(?i)[(\[{]\s*(?:official\s*(?:video|audio|music|lyrics?)?|lyrics?|audio|video|remaster(?:ed)?)\s*[)\]}]",
        ),
        trailing_tag: compile(r"(?i)[\s\-(\[]+(?:copy|official|lyrics?|audio)[\s\-)\]]*$"),
        by_artist: compile(r"(?i)\s+by\s+.*$"),
        dangling_by: compile(r"(?i)\s+by$"),
        dash: compile(r"\s*-\s*"),
        spaces: compile(r"\s+"),
    }
});

/// Turns file names and raw tags into a readable Title Case song title.
pub fn clean_proper_title(raw: &str) -> String {
    let patterns = &*PATTERNS;
    let mut text = raw.trim().to_owned();
    if text.is_empty() {
        return "Untitled".to_owned();
    }

    for pattern in [
        &patterns.extension,
        &patterns.hashed_prefix,
        &patterns.bare_prefix,
        &patterns.track_number,
    ] {
        text = pattern.replace(&text, "").into_owned();
    }
    text = text.replace('_', " ");
    for pattern in [
        &patterns.copy_index,
        &patterns.media_tag,
        &patterns.trailing_tag,
        &patterns.by_artist,
        &patterns.dangling_by,
    ] {
        text = pattern.replace_all(&text, "").into_owned();
    }
    text = patterns.dash.replace_all(&text, " ").into_owned();
    text = patterns.spaces.replace_all(&text, " ").trim().to_owned();

    let title = text
        .split_whitespace()
        .map(capitalize_word)
        .collect::<Vec<_>>()
        .join(" ");
    if title.is_empty() {
        "Untitled".to_owned()
    } else {
        title
    }
}

/// Cleaned artist name, or an empty string when the artist is missing or "unknown".
pub fn clean_artist(raw: &str) -> String {
    if raw.trim().is_empty() || raw.to_lowercase().contains("unknown") {
        String::new()
    } else {
        clean_proper_title(raw)
    }
}

fn capitalize_word(word: &str) -> String {
    let is_acronym = word.chars().count() <= 4
        && word.chars().any(char::is_uppercase)
        && !word.chars().any(char::is_lowercase)
        && word.chars().count() > 1;
    if is_acronym {
        return word.to_owned();
    }
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first
            .to_uppercase()
            .chain(chars.flat_map(char::to_lowercase))
            .collect(),
        None => String::new(),
    }
}

/// `MM:SS`, or `HH:MM:SS` once the time reaches an hour.
pub fn format_clock(total_seconds: u64) -> String {
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

/// YouTube timestamp format: `mm:ss`, switching to `h:mm:ss` once elapsed reaches one hour.
pub fn format_youtube_timestamp(total_seconds: u64) -> String {
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

/// Millisecond-bounded chapter entry used for MP4 chapters and `Timestamp.txt`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimedChapter {
    pub start_ms: u64,
    pub end_ms: u64,
    pub title: String,
}

fn clean_album_titles(titles: &[String]) -> Vec<String> {
    titles
        .iter()
        .enumerate()
        .map(|(idx, raw)| {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                format!("Track {}", idx + 1)
            } else {
                clean_proper_title(trimmed)
            }
        })
        .collect()
}

/// Computes MP4/YouTube chapter entries across the rendered duration, accounting for
/// audio crossfades and optional manual duration looping.
pub fn build_timed_chapters(
    raw_titles: &[String],
    durations: &[f64],
    crossfade_secs: f64,
    total_duration_secs: f64,
) -> Vec<TimedChapter> {
    if raw_titles.is_empty() {
        return Vec::new();
    }
    let clean_titles = clean_album_titles(raw_titles);
    let mut offsets = Vec::with_capacity(clean_titles.len());
    let mut acc_dur = 0.0_f64;

    for (idx, title) in clean_titles.iter().enumerate() {
        let dur = durations.get(idx).copied().unwrap_or(0.0).max(0.0);
        if idx == 0 {
            offsets.push((0.0_f64, title.as_str()));
            acc_dur = dur;
        } else {
            let safe_cf = if crossfade_secs > 0.0 {
                crossfade_secs.min(acc_dur * 0.45).min(dur * 0.45)
            } else {
                0.0
            };
            if safe_cf >= 0.05 {
                offsets.push(((acc_dur - safe_cf).max(0.0), title.as_str()));
                acc_dur = (acc_dur + dur - safe_cf).max(0.05);
            } else {
                offsets.push((acc_dur, title.as_str()));
                acc_dur += dur;
            }
        }
    }

    let target_secs = if total_duration_secs > 0.05 {
        total_duration_secs
    } else {
        acc_dur
    };
    let total_ms = (target_secs * 1000.0).round() as u64;
    if total_ms == 0 {
        return Vec::new();
    }

    let mut chapters: Vec<TimedChapter> = Vec::new();
    let mut cycle = 0usize;
    loop {
        let cycle_start = cycle as f64 * acc_dur;
        if cycle > 0 && (cycle_start >= target_secs - 0.05 || acc_dur <= 0.05) {
            break;
        }
        for &(offset, title) in &offsets {
            let abs_ms = ((cycle_start + offset) * 1000.0).round() as u64;
            if !chapters.is_empty() && abs_ms.saturating_add(1000) > total_ms {
                break;
            }
            if chapters.is_empty() && abs_ms >= total_ms {
                break;
            }
            let start_ms = chapters
                .last()
                .map_or(abs_ms, |prev| abs_ms.max(prev.start_ms + 1));
            if start_ms >= total_ms && !chapters.is_empty() {
                break;
            }
            chapters.push(TimedChapter {
                start_ms,
                end_ms: total_ms,
                title: title.to_owned(),
            });
        }
        if target_secs <= acc_dur + 0.05 {
            break;
        }
        cycle += 1;
    }

    for idx in 0..chapters.len().saturating_sub(1) {
        chapters[idx].end_ms = chapters[idx + 1].start_ms;
    }
    if let Some(last) = chapters.last_mut() {
        last.end_ms = total_ms.max(last.start_ms + 1);
    }
    chapters
}

/// Formats `playlist.txt` with one clean track title per line.
pub fn format_playlist_txt(raw_titles: &[String]) -> String {
    clean_album_titles(raw_titles).join("\n")
}

/// Formats `Timestamp.txt` ready for a YouTube video description (`DAFTAR LAGU` + lines).
pub fn format_timestamp_txt(chapters: &[TimedChapter]) -> String {
    let mut lines = Vec::with_capacity(chapters.len() + 1);
    lines.push("DAFTAR LAGU".to_owned());
    for chapter in chapters {
        let secs = (chapter.start_ms + 500) / 1000;
        lines.push(format!(
            "{} - {}",
            format_youtube_timestamp(secs),
            chapter.title
        ));
    }
    lines.join("\n")
}

fn escape_ffmetadata(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '=' | ';' | '#' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            '\n' => out.push_str("\\n"),
            '\r' => {}
            _ => out.push(ch),
        }
    }
    out
}

/// Encodes `chapters` as FFmpeg `ffmetadata` (`;FFMETADATA1`) with 1 ms timebase.
pub fn format_ffmetadata(chapters: &[TimedChapter]) -> String {
    let mut out = String::from(";FFMETADATA1\n");
    for chapter in chapters {
        out.push_str("\n[CHAPTER]\nTIMEBASE=1/1000\nSTART=");
        out.push_str(&chapter.start_ms.to_string());
        out.push_str("\nEND=");
        out.push_str(&chapter.end_ms.to_string());
        out.push_str("\ntitle=");
        out.push_str(&escape_ffmetadata(&chapter.title));
        out.push('\n');
    }
    out
}

/// The part of a track the chapter list needs.
#[derive(Debug)]
pub struct ChapterSource {
    pub title: Option<String>,
    pub filename: Option<String>,
    pub duration: f64,
}

#[derive(Debug, PartialEq)]
pub struct Chapter {
    pub index: usize,
    pub seconds: f64,
    pub time_str: String,
    pub title: String,
    pub duration: f64,
}

pub fn build_chapters(tracks: &[ChapterSource]) -> Vec<Chapter> {
    let mut start = 0.0;
    tracks
        .iter()
        .enumerate()
        .map(|(position, track)| {
            let raw = [&track.title, &track.filename]
                .into_iter()
                .flatten()
                .find(|value| !value.is_empty())
                .cloned()
                .unwrap_or_else(|| format!("Track {}", position + 1));
            let chapter = Chapter {
                index: position + 1,
                seconds: start,
                time_str: format_clock(start as u64),
                title: clean_proper_title(&raw),
                duration: track.duration,
            };
            start += track.duration;
            chapter
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_upload_prefix_numbering_and_tags() {
        assert_eq!(
            clean_proper_title("audio_0cbae9d7_01. king_of_kings (Official Video).mp3"),
            "King Of Kings"
        );
        assert_eq!(
            clean_proper_title("Song-Name [Lyrics] (1).wav"),
            "Song Name"
        );
    }

    #[test]
    fn keeps_short_acronyms_and_drops_artist_suffix() {
        assert_eq!(clean_proper_title("GOD is good by Someone"), "GOD Is Good");
        assert_eq!(clean_proper_title("   "), "Untitled");
    }

    #[test]
    fn artist_is_empty_when_unknown() {
        assert_eq!(clean_artist("Unknown Artist"), "");
        assert_eq!(clean_artist("hillsong worship"), "Hillsong Worship");
    }

    #[test]
    fn chapters_accumulate_start_times() {
        let tracks = vec![
            ChapterSource {
                title: Some("one".into()),
                filename: None,
                duration: 125.0,
            },
            ChapterSource {
                title: None,
                filename: Some("two.mp3".into()),
                duration: 3600.0,
            },
            ChapterSource {
                title: None,
                filename: None,
                duration: 10.0,
            },
        ];
        let chapters = build_chapters(&tracks);
        assert_eq!(chapters[0].time_str, "00:00");
        assert_eq!(chapters[1].time_str, "02:05");
        assert_eq!(chapters[1].title, "Two");
        assert_eq!(chapters[2].time_str, "01:02:05");
        assert_eq!(chapters[2].title, "Track 3");
    }

    #[test]
    fn formats_youtube_timestamps_switching_to_single_hour_digit() {
        assert_eq!(format_youtube_timestamp(0), "00:00");
        assert_eq!(format_youtube_timestamp(245), "04:05");
        assert_eq!(format_youtube_timestamp(3599), "59:59");
        assert_eq!(format_youtube_timestamp(3600), "1:00:00");
        assert_eq!(format_youtube_timestamp(3725), "1:02:05");
    }

    #[test]
    fn builds_timestamp_playlist_and_ffmetadata_for_ten_tracks() {
        let titles: Vec<String> = (1..=10)
            .map(|i| format!("{i:02}. lagu_test_{i}.mp3"))
            .collect();
        let durations = vec![245.0; 10];
        let chapters = build_timed_chapters(&titles, &durations, 0.0, 2450.0);
        assert_eq!(chapters.len(), 10);
        assert_eq!(chapters[0].start_ms, 0);
        assert_eq!(chapters[0].end_ms, 245_000);
        assert_eq!(chapters[9].end_ms, 2_450_000);

        let playlist = format_playlist_txt(&titles);
        assert_eq!(playlist.lines().count(), 10);

        let ts = format_timestamp_txt(&chapters);
        let ts_lines: Vec<&str> = ts.lines().collect();
        assert_eq!(ts_lines.len(), 11);
        assert_eq!(ts_lines[0], "DAFTAR LAGU");
        assert_eq!(ts_lines[1], "00:00 - Lagu Test 1");
        assert_eq!(ts_lines[2], "04:05 - Lagu Test 2");
        assert_eq!(ts_lines[10], "36:45 - Lagu Test 10");

        let meta = format_ffmetadata(&chapters);
        assert!(meta.starts_with(";FFMETADATA1\n"));
        assert!(meta.contains("START=0\nEND=245000\ntitle=Lagu Test 1\n"));
    }

    #[test]
    fn timed_chapters_handle_crossfade_and_manual_loop_past_one_hour() {
        let titles = vec!["satu.mp3".into(), "dua#test=ok.mp3".into()];
        let durations = vec![1800.0, 1850.0];
        let chapters = build_timed_chapters(&titles, &durations, 5.0, 4000.0);
        // Track 2 starts at 1800 - 5 = 1795s; cycle 2 starts at 3645s (> 1h -> h:mm:ss).
        assert_eq!(chapters.len(), 3);
        assert_eq!(chapters[1].start_ms, 1_795_000);
        assert_eq!(chapters[2].start_ms, 3_645_000);
        let ts = format_timestamp_txt(&chapters);
        assert!(ts.contains("1:00:45 - Satu"));

        let meta = format_ffmetadata(&chapters);
        assert!(meta.contains("title=Dua\\#test\\=ok"));
    }
}
