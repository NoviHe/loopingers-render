use std::path::Path;

use tokio::process::Command;

use crate::{
    error::Result,
    ffmpeg::{self, Tools},
    karaoke::{LyricLine, LyricWord},
    media::{probe_audio, probe_video, AudioInfo, VideoInfo},
};

type DemoWord = (&'static str, f64, f64);

struct DemoSong {
    file: &'static str,
    title: &'static str,
    artist: &'static str,
    tone_hz: &'static str,
    seconds: &'static str,
    lines: &'static [&'static [DemoWord]],
}

const SONGS: [DemoSong; 2] = [
    DemoSong {
        file: "demo_lagu_1.mp3",
        title: "Kemesraan (Sampel Demo 1)",
        artist: "Iwan Fals",
        tone_hz: "523.25",
        seconds: "10",
        lines: &[
            &[
                ("Kemesraan", 1.0, 2.0),
                ("ini", 2.0, 2.5),
                ("janganlah", 2.5, 3.4),
                ("cepat", 3.4, 4.1),
                ("berlalu", 4.1, 5.0),
            ],
            &[
                ("Hatiku", 5.5, 6.3),
                ("damai", 6.3, 7.1),
                ("jiwaku", 7.1, 7.9),
                ("tentram", 7.9, 8.7),
                ("bersamamu", 8.7, 9.5),
            ],
        ],
    },
    DemoSong {
        file: "demo_lagu_2.mp3",
        title: "Pelangi (Sampel Demo 2)",
        artist: "AT Mahmud",
        tone_hz: "659.25",
        seconds: "8",
        lines: &[
            &[
                ("Pelangi", 1.0, 1.8),
                ("pelangi", 1.8, 2.6),
                ("alangkah", 2.6, 3.5),
                ("indahmu", 3.5, 4.5),
            ],
            &[
                ("Merah", 5.0, 5.6),
                ("kuning", 5.6, 6.1),
                ("hijau", 6.1, 6.7),
                ("di langit", 6.7, 7.2),
                ("yang biru", 7.2, 7.8),
            ],
        ],
    },
];

pub struct DemoTrack {
    pub info: AudioInfo,
    pub lyrics: Vec<LyricLine>,
}

pub struct DemoSet {
    pub video: VideoInfo,
    pub tracks: Vec<DemoTrack>,
}

fn lyric_line(words: &[DemoWord]) -> LyricLine {
    LyricLine {
        start: words[0].1,
        end: words[words.len() - 1].2,
        text: words
            .iter()
            .map(|word| word.0)
            .collect::<Vec<_>>()
            .join(" "),
        words: words
            .iter()
            .map(|&(word, start, end)| LyricWord {
                word: word.to_owned(),
                start,
                end,
            })
            .collect(),
    }
}

async fn generate_if_missing(mut cmd: Command, output: &Path) -> Result<()> {
    if !output.exists() {
        cmd.arg(output);
        ffmpeg::run(&mut cmd, "FFmpeg (contoh demo)").await?;
    }
    Ok(())
}

/// Builds a synthetic background loop and two tone tracks with timed lyrics, for trying the
/// workflow without any media of your own.
pub async fn generate(tools: &Tools, dir: &Path) -> Result<DemoSet> {
    tokio::fs::create_dir_all(dir).await?;
    let video_path = dir.join("demo_background_loop.mp4");

    let mut video = tools.ffmpeg();
    video.args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "mandelbrot=size=1280x720:rate=30",
        "-t",
        "5",
        "-pix_fmt",
        "yuv420p",
        "-c:v",
        "libx264",
    ]);
    generate_if_missing(video, &video_path).await?;

    let mut tracks = Vec::new();
    for song in &SONGS {
        let path = dir.join(song.file);
        let mut tone = tools.ffmpeg();
        tone.args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
        ])
        .arg(format!(
            "sine=frequency={}:duration={}",
            song.tone_hz, song.seconds
        ))
        .args(["-c:a", "libmp3lame"]);
        generate_if_missing(tone, &path).await?;

        let mut info = probe_audio(tools, &path).await?;
        info.title = song.title.to_owned();
        info.artist = song.artist.to_owned();
        tracks.push(DemoTrack {
            info,
            lyrics: song.lines.iter().map(|words| lyric_line(words)).collect(),
        });
    }

    Ok(DemoSet {
        video: probe_video(tools, &video_path).await?,
        tracks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lyric_line_spans_its_words() {
        let line = lyric_line(SONGS[1].lines[1]);
        assert_eq!(line.text, "Merah kuning hijau di langit yang biru");
        assert_eq!((line.start, line.end), (5.0, 7.8));
        assert_eq!(line.words.len(), 5);
    }
}
