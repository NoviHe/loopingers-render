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
}
