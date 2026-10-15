use std::sync::LazyLock;

use regex::Regex;

use super::{spread_words, LyricLine};
use crate::util::round2;

const MAX_LINE_SECONDS: f64 = 8.0;
const FALLBACK_GAP_SECONDS: f64 = 3.0;
const LAST_LINE_SECONDS: f64 = 4.0;

static TIMESTAMP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\[(\d+):(\d+(?:\.\d+)?)\](.*)").expect("LRC pattern is valid"));

/// Parses `[mm:ss.xx] text` lines; each line's words share its time span equally.
pub fn parse_lrc(content: &str) -> Vec<LyricLine> {
    let mut entries: Vec<(f64, &str)> = content
        .trim()
        .lines()
        .filter_map(|line| {
            let captures = TIMESTAMP.captures(line.trim())?;
            let minutes: f64 = captures[1].parse().ok()?;
            let seconds: f64 = captures[2].parse().ok()?;
            let lyric = captures.get(3)?.as_str().trim();
            (!lyric.is_empty()).then_some((minutes * 60.0 + seconds, lyric))
        })
        .collect();
    entries.sort_by(|a, b| a.0.total_cmp(&b.0));

    entries
        .iter()
        .enumerate()
        .filter_map(|(index, &(start, text))| {
            let end = match entries.get(index + 1) {
                Some(&(next_start, _)) => {
                    let end = (start + MAX_LINE_SECONDS).min(next_start);
                    if end <= start {
                        start + FALLBACK_GAP_SECONDS
                    } else {
                        end
                    }
                }
                None => start + LAST_LINE_SECONDS,
            };
            let words = spread_words(text, start, end - start);
            (!words.is_empty()).then(|| LyricLine {
                start: round2(start),
                end: round2(end),
                text: text.to_owned(),
                words,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sorts_and_times_lines() {
        let lines =
            parse_lrc("[00:10.00] second line\n[00:01.50]first\nnot a lyric\n[00:20.00]   ");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "first");
        assert_eq!((lines[0].start, lines[0].end), (1.5, 9.5));
        assert_eq!(lines[1].text, "second line");
        assert_eq!((lines[1].start, lines[1].end), (10.0, 14.0));
        assert_eq!(lines[1].words.len(), 2);
    }

    #[test]
    fn duplicate_timestamps_fall_back_to_a_short_span() {
        let lines = parse_lrc("[00:05.00]a\n[00:05.00]b");
        assert_eq!((lines[0].start, lines[0].end), (5.0, 8.0));
    }
}
