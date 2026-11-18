use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    error::{Error, Result},
    ffmpeg::{self, Tools},
    media::probe_audio,
};

const AAC_BITRATE: &str = "256k";

/// Options for joining, normalizing, crossfading, trimming/looping, and fading playlist audio.
#[derive(Debug, Clone, Default)]
pub struct AudioMergeOptions {
    pub bitrate: String,
    pub crossfade_secs: f64,
    pub fade_out_secs: f64,
    pub normalize_volume: bool,
    pub target_duration_secs: Option<f64>,
}

struct TempFileGuard(PathBuf);

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Joins every playlist track into one 48 kHz stereo AAC file, whatever the input formats are.
/// Returns the duration of the result in seconds.
pub async fn merge_tracks(tools: &Tools, inputs: &[PathBuf], output: &Path) -> Result<f64> {
    merge_tracks_with_bitrate(tools, inputs, output, AAC_BITRATE).await
}

/// Joins playlist tracks into 48 kHz stereo AAC using the requested bitrate (defaulting to 256k).
pub async fn merge_tracks_with_bitrate(
    tools: &Tools,
    inputs: &[PathBuf],
    output: &Path,
    bitrate: &str,
) -> Result<f64> {
    merge_tracks_with_options(
        tools,
        inputs,
        output,
        &AudioMergeOptions {
            bitrate: bitrate.to_owned(),
            ..AudioMergeOptions::default()
        },
    )
    .await
}

/// Merged audio duration along with each input track's probed duration (seconds).
#[derive(Debug, Clone)]
pub struct MergedAudio {
    pub duration: f64,
    pub track_durations: Vec<f64>,
}

/// Joins playlist tracks with optional loudness normalization, per-track crossfade,
/// manual target loop/trim duration, and end fade-out.
pub async fn merge_tracks_with_options(
    tools: &Tools,
    inputs: &[PathBuf],
    output: &Path,
    options: &AudioMergeOptions,
) -> Result<f64> {
    merge_tracks_detailed(tools, inputs, output, options, false)
        .await
        .map(|merged| merged.duration)
}

/// Joins playlist tracks and, when `probe_all_durations` is true, measures every input track
/// duration so chapter timestamps stay aligned with audio edits.
pub async fn merge_tracks_detailed(
    tools: &Tools,
    inputs: &[PathBuf],
    output: &Path,
    options: &AudioMergeOptions,
    probe_all_durations: bool,
) -> Result<MergedAudio> {
    if inputs.is_empty() {
        return Err(Error::invalid("Daftar file audio kosong."));
    }
    let bitrate = if options.bitrate.trim().is_empty() {
        AAC_BITRATE
    } else {
        options.bitrate.trim()
    };

    let need_durations = probe_all_durations
        || options.target_duration_secs.is_some()
        || options.fade_out_secs > 0.0
        || (options.crossfade_secs > 0.0 && inputs.len() > 1);
    let durations = if need_durations {
        probe_track_durations(tools, inputs).await?
    } else {
        vec![0.0; inputs.len()]
    };

    let (base_graph, last_label, joined_dur) = build_join_graph(&durations, options);

    if let Some(target_dur) = options
        .target_duration_secs
        .filter(|target| *target > joined_dur + 0.05 && joined_dur > 0.05)
    {
        let joined_tmp = output.with_extension("joined.flac");
        let _guard = TempFileGuard(joined_tmp.clone());
        let flac_graph = format!("{base_graph}[{last_label}]asetnsamples=n=4096:p=0[aout]");

        let mut step1 = tools.ffmpeg();
        step1.args(["-hide_banner", "-nostats", "-loglevel", "error", "-y"]);
        for input in inputs {
            step1.arg("-i").arg(input);
        }
        step1
            .arg("-filter_complex")
            .arg(&flac_graph)
            .args(["-map", "[aout]", "-map_metadata", "-1", "-c:a", "flac"])
            .arg(&joined_tmp);
        ffmpeg::run(&mut step1, "FFmpeg (gabung audio)").await?;

        let mut step2 = tools.ffmpeg();
        step2
            .args(["-hide_banner", "-nostats", "-loglevel", "error", "-y"])
            .args(["-stream_loop", "-1", "-i"])
            .arg(&joined_tmp)
            .arg("-t")
            .arg(format!("{target_dur:.3}"));
        if let Some(afade) = build_fade_out_filter(target_dur, options.fade_out_secs) {
            step2.args(["-af", &afade]);
        }
        step2
            .args([
                "-map",
                "0:a",
                "-map_metadata",
                "-1",
                "-c:a",
                "aac",
                "-b:a",
                bitrate,
            ])
            .arg(output);
        ffmpeg::run(&mut step2, "FFmpeg (loop audio manual)").await?;
    } else {
        let mut tail_stages = Vec::new();
        let effective_dur = if let Some(target_dur) = options.target_duration_secs {
            tail_stages.push(format!("atrim=0:{target_dur:.3}"));
            target_dur
        } else {
            joined_dur
        };
        if let Some(afade) = build_fade_out_filter(effective_dur, options.fade_out_secs) {
            tail_stages.push(afade);
        }
        let tail = if tail_stages.is_empty() {
            "anull".to_owned()
        } else {
            tail_stages.join(",")
        };
        let graph = format!("{base_graph}[{last_label}]{tail}[aout]");

        let mut cmd = tools.ffmpeg();
        cmd.args(["-hide_banner", "-nostats", "-loglevel", "error", "-y"]);
        for input in inputs {
            cmd.arg("-i").arg(input);
        }
        cmd.arg("-filter_complex")
            .arg(&graph)
            .args([
                "-map",
                "[aout]",
                "-map_metadata",
                "-1",
                "-c:a",
                "aac",
                "-b:a",
                bitrate,
            ])
            .arg(output);
        ffmpeg::run(&mut cmd, "FFmpeg (gabung audio)").await?;
    }

    let duration = probe_audio(tools, output).await?.duration;
    Ok(MergedAudio {
        duration,
        track_durations: durations,
    })
}

fn build_fade_out_filter(total_duration: f64, fade_out_secs: f64) -> Option<String> {
    if fade_out_secs <= 0.0 || total_duration <= 0.0 {
        return None;
    }
    let d = fade_out_secs.min(total_duration);
    let st = (total_duration - d).max(0.0);
    Some(format!("afade=t=out:st={st:.3}:d={d:.3}"))
}

/// Probe a bounded number of files concurrently, preserving playlist order for chapters.
async fn probe_track_durations(tools: &Tools, inputs: &[PathBuf]) -> Result<Vec<f64>> {
    let mut tasks = tokio::task::JoinSet::new();
    let mut remaining = inputs.iter().enumerate();
    let mut durations = vec![0.0; inputs.len()];
    for (index, path) in remaining.by_ref().take(4) {
        let (tools, path) = (tools.clone(), path.clone());
        tasks.spawn(async move { (index, probe_audio(&tools, &path).await) });
    }
    while let Some(result) = tasks.join_next().await {
        let (index, info) = result
            .map_err(|error| Error::internal(format!("Pemeriksaan audio terhenti: {error}")))?;
        durations[index] = info?.duration.max(0.0);
        if let Some((index, path)) = remaining.next() {
            let (tools, path) = (tools.clone(), path.clone());
            tasks.spawn(async move { (index, probe_audio(&tools, &path).await) });
        }
    }
    Ok(durations)
}

fn build_join_graph(durations: &[f64], options: &AudioMergeOptions) -> (String, String, f64) {
    let norm_prefix = if options.normalize_volume {
        "loudnorm=I=-14:TP=-1.5:LRA=11,"
    } else {
        ""
    };
    let mut graph: String = (0..durations.len())
        .map(|index| {
            format!(
                "[{index}:a]{norm_prefix}aresample=48000,aformat=sample_rates=48000:channel_layouts=stereo[a{index}];"
            )
        })
        .collect();

    if durations.len() == 1 {
        return (graph, "a0".to_owned(), durations[0]);
    }

    if options.crossfade_secs > 0.0 {
        let mut prev = "a0".to_owned();
        let mut acc_dur = durations[0];
        for (index, &track_dur) in durations.iter().enumerate().skip(1) {
            let next_label = format!("cf{index}");
            let safe_cf = options
                .crossfade_secs
                .min(acc_dur * 0.45)
                .min(track_dur * 0.45);
            if safe_cf >= 0.05 {
                graph.push_str(&format!(
                    "[{prev}][a{index}]acrossfade=d={safe_cf:.3}:c1=tri:c2=tri[{next_label}];"
                ));
                acc_dur = (acc_dur + track_dur - safe_cf).max(0.05);
            } else {
                graph.push_str(&format!(
                    "[{prev}][a{index}]concat=n=2:v=0:a=1[{next_label}];"
                ));
                acc_dur += track_dur;
            }
            prev = next_label;
        }
        return (graph, prev, acc_dur);
    }

    let labels: String = (0..durations.len())
        .map(|index| format!("[a{index}]"))
        .collect();
    graph.push_str(&format!(
        "{labels}concat=n={}:v=0:a=1[joined];",
        durations.len()
    ));
    (graph, "joined".to_owned(), durations.iter().sum())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_graph_builds_normalize_and_safe_crossfade() {
        let options = AudioMergeOptions {
            bitrate: "192k".to_owned(),
            crossfade_secs: 4.0,
            fade_out_secs: 5.0,
            normalize_volume: true,
            target_duration_secs: None,
        };
        let (graph, last, total) = build_join_graph(&[10.0, 6.0], &options);
        assert!(graph.contains("loudnorm=I=-14:TP=-1.5:LRA=11"));
        // crossfade clamps to 45% of shortest track (6.0 * 0.45 = 2.7s)
        assert!(graph.contains("acrossfade=d=2.700:c1=tri:c2=tri"));
        assert_eq!(last, "cf1");
        assert!((total - 13.3).abs() < 0.001);
        assert_eq!(
            build_fade_out_filter(total, options.fade_out_secs).as_deref(),
            Some("afade=t=out:st=8.300:d=5.000")
        );
    }

    #[test]
    fn fade_out_clamps_to_total_duration() {
        assert_eq!(
            build_fade_out_filter(3.0, 10.0).as_deref(),
            Some("afade=t=out:st=0.000:d=3.000")
        );
        assert_eq!(build_fade_out_filter(10.0, 0.0), None);
    }
}
