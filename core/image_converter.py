import os
import subprocess
import time
import uuid
from typing import Optional, Dict, Any
import numpy as np
import imageio_ffmpeg

from core.media_info import get_video_info
from core.hardware import get_hardware_info

FFMPEG_EXE = imageio_ffmpeg.get_ffmpeg_exe()

RESOLUTION_PRESETS = {
    "16:9": {
        "1080p": (1920, 1080),
        "720p": (1280, 720),
        "4k": (3840, 2160)
    },
    "9:16": {
        "1080p": (1080, 1920),
        "720p": (720, 1280),
        "4k": (2160, 3840)
    },
    "1:1": {
        "1080p": (1080, 1080),
        "720p": (720, 720),
        "4k": (2160, 2160)
    }
}

SUPPORTED_IMAGE_EXTENSIONS = {".jpg", ".jpeg", ".png", ".webp", ".jfif"}


def is_image_file(filename_or_path: str) -> bool:
    """Checks whether the filename has a supported image extension."""
    ext = os.path.splitext(filename_or_path)[1].lower()
    return ext in SUPPORTED_IMAGE_EXTENSIONS


def estimate_tempo_bpm(audio_path: str) -> float:
    """
    Estimates dominant musical tempo (BPM) from an audio track
    using fast audio envelope onset autocorrelation.
    """
    if not audio_path or not os.path.exists(audio_path):
        return 105.0

    try:
        pipe = subprocess.Popen([
            FFMPEG_EXE, '-ss', '5', '-t', '25', '-i', os.path.abspath(audio_path),
            '-f', 'f32le', '-ac', '1', '-ar', '22050', '-'
        ], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        raw = pipe.stdout.read()
        if not raw or len(raw) < 22050 * 5 * 4:
            return 105.0

        samples = np.frombuffer(raw, dtype=np.float32)
        hop = 220
        n_hops = len(samples) // hop
        if n_hops < 50:
            return 105.0

        frames = samples[:n_hops * hop].reshape(n_hops, hop)
        envelope = np.sqrt(np.mean(frames ** 2, axis=1))

        onset = np.diff(envelope, prepend=0)
        onset = np.maximum(0, onset)
        onset = onset - np.mean(onset)

        autocorr = np.correlate(onset, onset, mode='full')[len(onset) - 1:]
        min_lag = int(22050 / hop * 60 / 160)
        max_lag = int(22050 / hop * 60 / 65)
        if max_lag >= len(autocorr):
            return 105.0

        peak_lag = min_lag + np.argmax(autocorr[min_lag:max_lag])
        bpm = (22050 / hop * 60) / peak_lag
        return float(np.clip(bpm, 65.0, 155.0))
    except Exception:
        return 105.0


def convert_image_to_video(
    image_path: str,
    output_path: str,
    duration: float = 10.0,
    fps: int = 30,
    aspect_ratio: str = "16:9",
    resolution: str = "1080p",
    effect: str = "zoom_in",
    beat_pulse: bool = True,
    bpm: float = 105.0,
    visual_effect: str = "none"
) -> Dict[str, Any]:
    """
    Converts a still image (JPG, PNG, WEBP, JFIF) into a looping MP4 video
    with optional cinematic visual motion effects (Ken Burns zoom in/out, pan, or static),
    audio-reactive rhythm beat pulse (zooms in slightly with music beat),
    and atmospheric visual effects (rain, snow, fire, bokeh, light leak, color grading, VHS retro, glitch).
    Uses NVIDIA NVENC GPU acceleration if available, falling back to libx264.
    """
    if not os.path.exists(image_path):
        raise FileNotFoundError(f"Image file not found: {image_path}")

    preset_dict = RESOLUTION_PRESETS.get(aspect_ratio, RESOLUTION_PRESETS["16:9"])
    w, h = preset_dict.get(resolution, (1920, 1080))
    total_frames = int(max(duration, 1.0) * fps)

    # 1. Build FFmpeg video filter for visual motion effects & beat pulse
    # Pre-scaling to 2x (3840x2160 or double target resolution) guarantees silky smooth zoompan
    pre_w = w * 2
    pre_h = h * 2

    # Rhythmic beat pulse term: zooms in slightly (~3.5%-4%) on every beat of the music
    freq = max(bpm, 60.0) / 60.0
    pulse_term = f"+0.038*pow(abs(cos(3.14159*time*{freq:.2f})),4)" if beat_pulse else ""

    if effect == "zoom_out":
        # Slow cinematic zoom out from 1.25x down to 1.0x centered + beat pulse
        zoom_expr = f"max(1.0, 1.20-on*(0.15/{total_frames}){pulse_term})"
        vf = (
            f"scale={pre_w}:{pre_h}:force_original_aspect_ratio=increase,crop={pre_w}:{pre_h},"
            f"zoompan=z='{zoom_expr}':d={total_frames}:"
            f"x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)':s={w}x{h}:fps={fps},"
            f"format=yuv420p"
        )
    elif effect == "pan_left":
        # Slow horizontal pan to the left + beat pulse
        zoom_expr = f"1.15{pulse_term}"
        vf = (
            f"scale={pre_w}:{pre_h}:force_original_aspect_ratio=increase,crop={pre_w}:{pre_h},"
            f"zoompan=z='{zoom_expr}':d={total_frames}:"
            f"x='(iw-iw/zoom)*(1-on/{total_frames})':y='(ih-ih/zoom)/2':s={w}x{h}:fps={fps},"
            f"format=yuv420p"
        )
    elif effect == "pan_right":
        # Slow horizontal pan to the right + beat pulse
        zoom_expr = f"1.15{pulse_term}"
        vf = (
            f"scale={pre_w}:{pre_h}:force_original_aspect_ratio=increase,crop={pre_w}:{pre_h},"
            f"zoompan=z='{zoom_expr}':d={total_frames}:"
            f"x='(iw-iw/zoom)*(on/{total_frames})':y='(ih-ih/zoom)/2':s={w}x{h}:fps={fps},"
            f"format=yuv420p"
        )
    elif effect == "static":
        if beat_pulse:
            # Static framing with gentle rhythm beat pulse
            zoom_expr = f"1.01{pulse_term}"
            vf = (
                f"scale={pre_w}:{pre_h}:force_original_aspect_ratio=increase,crop={pre_w}:{pre_h},"
                f"zoompan=z='{zoom_expr}':d={total_frames}:"
                f"x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)':s={w}x{h}:fps={fps},"
                f"format=yuv420p"
            )
        else:
            # Completely static framing
            vf = f"scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h},format=yuv420p"
    else:  # 'zoom_in' default
        # Slow cinematic zoom in from 1.0x to 1.25x centered + beat pulse
        zoom_expr = f"min(1.25, 1.0+on*(0.15/{total_frames}){pulse_term})"
        vf = (
            f"scale={pre_w}:{pre_h}:force_original_aspect_ratio=increase,crop={pre_w}:{pre_h},"
            f"zoompan=z='{zoom_expr}':d={total_frames}:"
            f"x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)':s={w}x{h}:fps={fps},"
            f"format=yuv420p"
        )

    # 2. Atmospheric Visual Overlay & Filter Styling
    from core.visual_effects import VISUAL_EFFECTS_CONFIG, ensure_effect_asset

    ve_cfg = VISUAL_EFFECTS_CONFIG.get(visual_effect, VISUAL_EFFECTS_CONFIG["none"])
    ve_type = ve_cfg.get("type", "none")

    extra_inputs = []
    filter_complex_str = None

    if ve_type == "filter":
        filter_snip = ve_cfg.get("vf", "")
        if filter_snip:
            vf = f"{vf},{filter_snip}"
    elif ve_type == "overlay":
        overlay_path = ensure_effect_asset(visual_effect)
        if overlay_path and os.path.exists(overlay_path):
            opacity = ve_cfg.get("opacity", 0.85)
            extra_inputs = ["-stream_loop", "-1", "-i", overlay_path]
            filter_complex_str = (
                f"[0:v]{vf}[base]; "
                f"[1:v]scale={w}:{h},format=yuv420p[ov]; "
                f"[base][ov]blend=all_mode=screen:all_opacity={opacity}[outv]"
            )

    # 3. Hardware Encoder selection (NVENC on NVIDIA GPU or CPU libx264)
    hw = get_hardware_info()
    if hw.get("has_nvidia") and hw.get("nvenc_h264"):
        vcodec_args = ["-c:v", "h264_nvenc", "-preset", "p4", "-cq", "20", "-b:v", "0"]
    else:
        vcodec_args = ["-c:v", "libx264", "-preset", "veryfast", "-crf", "20"]

    # 4. Execute FFmpeg conversion
    os.makedirs(os.path.dirname(os.path.abspath(output_path)), exist_ok=True)
    if filter_complex_str:
        cmd = [
            FFMPEG_EXE,
            "-y",
            "-loop", "1",
            "-i", os.path.abspath(image_path),
            *extra_inputs,
            "-t", str(duration),
            "-filter_complex", filter_complex_str,
            "-map", "[outv]",
            *vcodec_args,
            "-r", str(fps),
            "-pix_fmt", "yuv420p",
            os.path.abspath(output_path)
        ]
    else:
        cmd = [
            FFMPEG_EXE,
            "-y",
            "-loop", "1",
            "-i", os.path.abspath(image_path),
            "-t", str(duration),
            "-vf", vf,
            *vcodec_args,
            "-r", str(fps),
            "-pix_fmt", "yuv420p",
            os.path.abspath(output_path)
        ]

    proc = subprocess.run(cmd, capture_output=True, text=True, cwd="c:\\loopingapp")
    if proc.returncode != 0:
        raise RuntimeError(f"FFmpeg image conversion failed: {proc.stderr[-300:]}")

    info = get_video_info(output_path)
    info["is_from_image"] = True
    info["effect"] = effect
    info["beat_pulse"] = beat_pulse
    info["bpm"] = round(bpm, 1)
    info["visual_effect"] = visual_effect
    info["source_image_path"] = image_path
    return info
