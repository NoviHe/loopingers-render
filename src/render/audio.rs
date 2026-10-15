use std::path::{Path, PathBuf};

use crate::{
    error::{Error, Result},
    ffmpeg::{self, Tools},
    media::probe_audio,
};

const AAC_BITRATE: &str = "256k";

/// Joins every playlist track into one 48 kHz stereo AAC file, whatever the input formats are.
/// Returns the duration of the result in seconds.
pub async fn merge_tracks(tools: &Tools, inputs: &[PathBuf], output: &Path) -> Result<f64> {
    if inputs.is_empty() {
        return Err(Error::invalid("Daftar file audio kosong."));
    }

    let mut cmd = tools.ffmpeg();
    cmd.args(["-hide_banner", "-nostats", "-loglevel", "error", "-y"]);
    for input in inputs {
        cmd.arg("-i").arg(input);
    }

    let normalized: String = (0..inputs.len())
        .map(|index| {
            format!("[{index}:a]aformat=sample_rates=48000:channel_layouts=stereo[a{index}];")
        })
        .collect();
    let labels: String = (0..inputs.len())
        .map(|index| format!("[a{index}]"))
        .collect();
    let graph = format!(
        "{normalized}{labels}concat=n={}:v=0:a=1[aout]",
        inputs.len()
    );

    cmd.arg("-filter_complex")
        .arg(graph)
        .args(["-map", "[aout]", "-c:a", "aac", "-b:a", AAC_BITRATE])
        .arg(output);
    ffmpeg::run(&mut cmd, "FFmpeg (gabung audio)").await?;

    Ok(probe_audio(tools, output).await?.duration)
}
