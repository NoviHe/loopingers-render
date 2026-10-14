pub mod ass;
pub mod lrc;
pub mod styles;
pub mod transcribe;

use crate::util::round2;

#[derive(Debug, Clone, PartialEq)]
pub struct LyricWord {
    pub word: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LyricLine {
    pub start: f64,
    pub end: f64,
    pub text: String,
    pub words: Vec<LyricWord>,
}

/// Splits `text` into words and shares `duration` between them equally.
pub fn spread_words(text: &str, start: f64, duration: f64) -> Vec<LyricWord> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let step = duration / words.len().max(1) as f64;
    words
        .into_iter()
        .enumerate()
        .map(|(index, word)| {
            let word_start = start + index as f64 * step;
            LyricWord {
                word: word.to_owned(),
                start: round2(word_start),
                end: round2(word_start + step),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spread_words_divides_the_duration_evenly() {
        let words = spread_words("one two", 1.0, 2.0);
        assert_eq!(words.len(), 2);
        assert_eq!((words[0].start, words[0].end), (1.0, 2.0));
        assert_eq!((words[1].start, words[1].end), (2.0, 3.0));
    }

    #[test]
    fn spread_words_of_empty_text_is_empty() {
        assert!(spread_words("   ", 0.0, 3.0).is_empty());
    }
}
