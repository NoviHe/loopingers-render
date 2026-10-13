import os
import re
import subprocess
import threading
import time
from typing import List, Dict, Any, Optional, Callable
import imageio_ffmpeg
from core.media_info import get_audio_info

FFMPEG_EXE = imageio_ffmpeg.get_ffmpeg_exe()

# Preset aspect ratios and resolutions
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


def build_video_filter(
    width: int,
    height: int,
    fit_mode: str = "crop",
    ass_path: Optional[str] = None,
    filter_effect_snippet: Optional[str] = None
) -> str:
    """
    Builds video filter chain for aspect ratio fitting, visual effect styling, and subtitle overlay.
    """
    filters = []

    if fit_mode == "blur_pad":
        # Background blurred and foreground fitted in center
        filter_str = (
            f"split[bg][fg];"
            f"[bg]scale={width}:{height}:force_original_aspect_ratio=increase,crop={width}:{height},boxblur=20:5[bgblur];"
            f"[fg]scale={width}:{height}:force_original_aspect_ratio=decrease[fgfit];"
            f"[bgblur][fgfit]overlay=(W-w)/2:(H-h)/2"
        )
        if filter_effect_snippet:
            filter_str += f"[vcomposed];[vcomposed]{filter_effect_snippet}"
        if ass_path:
            rel_ass = os.path.relpath(ass_path, start="c:\\loopingapp").replace("\\", "/")
            filter_str += f"[vcomposed];[vcomposed]ass={rel_ass}"
        return filter_str
    elif fit_mode == "fit":
        # Letterbox / Pillarbox with black bars
        filters.append(f"scale={width}:{height}:force_original_aspect_ratio=decrease")
        filters.append(f"pad={width}:{height}:(ow-iw)/2:(oh-ih)/2:black")
    else:  # 'crop' or default
        # Fill screen, crop edges
        filters.append(f"scale={width}:{height}:force_original_aspect_ratio=increase")
        filters.append(f"crop={width}:{height}")

    if filter_effect_snippet:
        filters.append(filter_effect_snippet)

    if ass_path:
        # Convert path to relative with forward slashes for robust libass loading
        rel_ass = os.path.relpath(ass_path, start="c:\\loopingapp").replace("\\", "/")
        filters.append(f"ass={rel_ass}")

    return ",".join(filters)


class RenderJob:
    def __init__(self, job_id: str):
        self.job_id = job_id
        self.status = "queued"  # queued, rendering, completed, failed, cancelled
        self.progress_percent = 0.0
        self.current_time_sec = 0.0
        self.total_time_sec = 0.0
        self.speed = "0x"
        self.fps = 0.0
        self.eta_seconds = 0
        self.error_message = ""
        self.output_file = ""
        self.encoder_name = "NVIDIA NVENC (GPU RTX 2060)"
        self.process: Optional[subprocess.Popen] = None
        self._cancel_requested = False

    def to_dict(self) -> dict:
        return {
            "job_id": self.job_id,
            "status": self.status,
            "progress_percent": round(self.progress_percent, 1),
            "current_time_sec": round(self.current_time_sec, 1),
            "total_time_sec": round(self.total_time_sec, 1),
            "speed": self.speed,
            "fps": round(self.fps, 1),
            "eta_seconds": int(self.eta_seconds),
            "encoder_name": self.encoder_name,
            "error_message": self.error_message,
            "output_file": self.output_file
        }


def prepare_merged_audio(audio_paths: List[str], output_merged_path: str) -> float:
    """
    Robustly concatenates and normalizes all playlist audio tracks into a single
    standardized 48kHz stereo master audio file using FFmpeg filter_complex.
    Guarantees that 100% of all songs (regardless of different sample rates 44.1k/48k,
    formats MP3/M4A/WAV, bit depths, or whether they have lyrics or not) are included
    sequentially without dropped streams, desync, or silence.
    Returns the exact total duration in seconds.
    """
    if not audio_paths:
        raise ValueError("Daftar file audio kosong.")

    os.makedirs(os.path.dirname(os.path.abspath(output_merged_path)), exist_ok=True)

    if len(audio_paths) == 1:
        cmd = [
            FFMPEG_EXE, "-y",
            "-i", os.path.abspath(audio_paths[0]),
            "-af", "aformat=sample_rates=48000:channel_layouts=stereo",
            "-c:a", "aac",
            "-b:a", "256k",
            os.path.abspath(output_merged_path)
        ]
        res = subprocess.run(cmd, capture_output=True, text=True, cwd="c:\\loopingapp")
        if res.returncode != 0:
            raise RuntimeError(f"Gagal memproses audio: {res.stderr[-300:]}")
    else:
        in_args = []
        filter_parts = []
        for idx, p in enumerate(audio_paths):
            in_args.extend(["-i", os.path.abspath(p)])
            filter_parts.append(f"[{idx}:a]aformat=sample_rates=48000:channel_layouts=stereo[a{idx}];")

        filter_concat = "".join(filter_parts) + "".join([f"[a{i}]" for i in range(len(audio_paths))]) + f"concat=n={len(audio_paths)}:v=0:a=1[aout]"
        cmd = [
            FFMPEG_EXE, "-y",
            *in_args,
            "-filter_complex", filter_concat,
            "-map", "[aout]",
            "-c:a", "aac",
            "-b:a", "256k",
            os.path.abspath(output_merged_path)
        ]
        res = subprocess.run(cmd, capture_output=True, text=True, cwd="c:\\loopingapp")
        if res.returncode != 0:
            raise RuntimeError(f"Gagal menggabungkan audio playlist: {res.stderr[-300:]}")

    info = get_audio_info(output_merged_path)
    return float(info.get("duration", 0.0))


def render_looping_karaoke_video(
    video_path: str,
    audio_paths: List[str],
    output_path: str,
    total_duration: float,
    ass_subtitle_path: Optional[str] = None,
    aspect_ratio: str = "16:9",
    resolution: str = "1080p",
    fit_mode: str = "crop",
    fps: int = 30,
    video_volume: float = 0.0,
    crossfade_duration: float = 0.0,
    encoder: str = "nvenc_h264",
    preset: str = "p4",
    quality_cq: int = 22,
    use_hwaccel: bool = True,
    visual_effect: str = "none",
    job: Optional[RenderJob] = None
) -> str:
    """
    Renders the looped background video with merged songs and karaoke subtitles.
    Supports atmospheric visual effects (rain, snow, fire, bokeh, light leak, color grading, VHS retro, glitch),
    NVIDIA NVENC (RTX 2060) GPU acceleration, and CPU fallback.
    """
    # 1. Determine target resolution dimensions
    preset_dict = RESOLUTION_PRESETS.get(aspect_ratio, RESOLUTION_PRESETS["16:9"])
    width, height = preset_dict.get(resolution, (1920, 1080))

    # 2. Robustly prepare and merge all audio tracks into a standardized 48kHz master
    temp_files_to_clean = []
    master_audio_file = os.path.abspath(f"uploads/master_audio_{int(time.time()*1000)}.m4a")
    temp_files_to_clean.append(master_audio_file)

    actual_duration = prepare_merged_audio(audio_paths, master_audio_file)
    if actual_duration > 0:
        total_duration = actual_duration

    if job:
        job.status = "rendering"
        job.total_time_sec = total_duration

    audio_input_args = ["-i", master_audio_file]
    stream_map_args = ["-map", "0:v:0", "-map", "1:a:0"]

    # 3. Build Video filter & Atmospheric Visual Effect
    from core.visual_effects import VISUAL_EFFECTS_CONFIG, ensure_effect_asset

    ve_cfg = VISUAL_EFFECTS_CONFIG.get(visual_effect, VISUAL_EFFECTS_CONFIG["none"])
    ve_type = ve_cfg.get("type", "none")

    extra_video_input = []
    filter_complex_str = None

    if ve_type == "filter":
        filter_snip = ve_cfg.get("vf")
        vfilter = build_video_filter(width, height, fit_mode=fit_mode, ass_path=ass_subtitle_path, filter_effect_snippet=filter_snip)
    elif ve_type == "overlay":
        overlay_path = ensure_effect_asset(visual_effect)
        if overlay_path and os.path.exists(overlay_path):
            opacity = ve_cfg.get("opacity", 0.85)
            extra_video_input = ["-stream_loop", "-1", "-i", overlay_path]
            base_filter = build_video_filter(width, height, fit_mode=fit_mode, ass_path=None)
            ass_sub = ""
            if ass_subtitle_path:
                rel_ass = os.path.relpath(ass_subtitle_path, start="c:\\loopingapp").replace("\\", "/")
                ass_sub = f",ass={rel_ass}"
            filter_complex_str = (
                f"[0:v]{base_filter}[base]; "
                f"[2:v]scale={width}:{height},format=yuv420p[ov]; "
                f"[base][ov]blend=all_mode=screen:all_opacity={opacity}[blended]; "
                f"[blended]format=yuv420p{ass_sub}[outv]"
            )
            stream_map_args = ["-map", "[outv]", "-map", "1:a:0"]
        else:
            vfilter = build_video_filter(width, height, fit_mode=fit_mode, ass_path=ass_subtitle_path)
    else:
        vfilter = build_video_filter(width, height, fit_mode=fit_mode, ass_path=ass_subtitle_path)

    # 4. Hardware Encoder & Codec Setup
    from core.hardware import get_hardware_info
    hw = get_hardware_info()

    selected_encoder = encoder
    if "nvenc" in selected_encoder and not hw["has_nvidia"]:
        selected_encoder = "cpu_x264"

    # Input decoding is handled via safe CPU decoders (supports all codecs including AV1 without freezing)
    # The heavy workload is fully accelerated on the NVIDIA GeForce RTX 2060 via NVENC (-c:v h264_nvenc / hevc_nvenc)
    hw_input_args = []

    if job:
        if selected_encoder == "nvenc_h264":
            job.encoder_name = "NVIDIA NVENC H.264 (RTX 2060 GPU)"
        elif selected_encoder == "nvenc_hevc":
            job.encoder_name = "NVIDIA NVENC HEVC (RTX 2060 GPU)"
        else:
            job.encoder_name = "CPU Software (x264)"

    if selected_encoder == "nvenc_h264":
        nv_preset = preset if preset.startswith("p") else "p4"
        vcodec_args = [
            "-c:v", "h264_nvenc",
            "-preset", nv_preset,
            "-tune", "hq",
            "-rc", "vbr",
            "-cq", str(quality_cq),
            "-b:v", "0",
            "-pix_fmt", "yuv420p"
        ]
    elif selected_encoder == "nvenc_hevc":
        nv_preset = preset if preset.startswith("p") else "p4"
        vcodec_args = [
            "-c:v", "hevc_nvenc",
            "-preset", nv_preset,
            "-tune", "hq",
            "-rc", "vbr",
            "-cq", str(quality_cq),
            "-b:v", "0",
            "-pix_fmt", "yuv420p"
        ]
    else:  # cpu_x264
        cpu_preset = preset if preset in ["ultrafast", "superfast", "veryfast", "faster", "fast", "medium", "slow"] else "veryfast"
        vcodec_args = [
            "-c:v", "libx264",
            "-preset", cpu_preset,
            "-crf", str(quality_cq),
            "-pix_fmt", "yuv420p",
            "-threads", "0"
        ]

    # 5. Construct FFmpeg command
    if filter_complex_str:
        cmd = [
            FFMPEG_EXE,
            "-y",
            *hw_input_args,
            "-stream_loop", "-1",
            "-i", video_path,
            *audio_input_args,
            *extra_video_input,
            "-t", str(total_duration),
            "-filter_complex", filter_complex_str,
            *stream_map_args,
            *vcodec_args,
            "-r", str(fps),
            "-c:a", "aac",
            "-b:a", "192k",
            "-progress", "pipe:1",
            output_path
        ]
    else:
        cmd = [
            FFMPEG_EXE,
            "-y",
            *hw_input_args,
            "-stream_loop", "-1",
            "-i", video_path,
            *audio_input_args,
            "-t", str(total_duration),
            "-vf", vfilter,
            *stream_map_args,
            *vcodec_args,
            "-r", str(fps),
            "-c:a", "aac",
            "-b:a", "192k",
            "-progress", "pipe:1",
            output_path
        ]

    try:
        proc = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
            cwd="c:\\loopingapp"
        )
        if job:
            job.process = proc

        # Concurrently drain stderr in a daemon thread to prevent pipe buffer deadlock
        stderr_buffer = []
        def _drain_stderr():
            try:
                for err_line in proc.stderr:
                    stderr_buffer.append(err_line)
                    if len(stderr_buffer) > 100:
                        stderr_buffer.pop(0)
            except Exception:
                pass

        err_thread = threading.Thread(target=_drain_stderr, daemon=True)
        err_thread.start()

        # Progress monitor loop
        start_time = time.time()
        time_pattern = re.compile(r'out_time_us=(\d+)|out_time_ms=(\d+)|out_time=(\d+):(\d+):(\d+(?:\.\d+)?)')
        speed_pattern = re.compile(r'speed=\s*([\d\.]+)x')
        fps_pattern = re.compile(r'fps=\s*([\d\.]+)')

        for line in proc.stdout:
            if job and job._cancel_requested:
                proc.terminate()
                job.status = "cancelled"
                break

            line_str = line.strip()
            
            # Check time
            if "out_time_ms=" in line_str:
                val = line_str.split("=")[1]
                if val.isdigit():
                    cur_sec = float(val) / 1000000.0
                    if job and total_duration > 0:
                        job.current_time_sec = min(cur_sec, total_duration)
                        job.progress_percent = min((cur_sec / total_duration) * 100.0, 99.9)
            elif "out_time=" in line_str:
                val = line_str.split("=")[1]
                parts = val.split(":")
                if len(parts) == 3:
                    try:
                        cur_sec = int(parts[0]) * 3600 + int(parts[1]) * 60 + float(parts[2])
                        if job and total_duration > 0:
                            job.current_time_sec = min(cur_sec, total_duration)
                            job.progress_percent = min((cur_sec / total_duration) * 100.0, 99.9)
                    except ValueError:
                        pass

            # Check speed & fps
            if "speed=" in line_str:
                m = speed_pattern.search(line_str)
                if m and job:
                    job.speed = f"{m.group(1)}x"
                    try:
                        spd = float(m.group(1))
                        if spd > 0 and job.total_time_sec > job.current_time_sec:
                            rem_media_sec = job.total_time_sec - job.current_time_sec
                            job.eta_seconds = rem_media_sec / spd
                    except ValueError:
                        pass

            if "fps=" in line_str:
                m = fps_pattern.search(line_str)
                if m and job:
                    try:
                        job.fps = float(m.group(1))
                    except ValueError:
                        pass

        proc.wait()
        err_thread.join(timeout=1.0)
        stderr_output = "".join(stderr_buffer)

        if proc.returncode == 0:
            if job:
                job.status = "completed"
                job.progress_percent = 100.0
                job.current_time_sec = total_duration
                job.eta_seconds = 0
                job.output_file = output_path
            return output_path
        else:
            err_msg = f"FFmpeg failed with code {proc.returncode}: {stderr_output[-400:]}"
            if job:
                job.status = "failed"
                job.error_message = err_msg
            raise RuntimeError(err_msg)

    finally:
        # Cleanup temporary concat files
        for f in temp_files_to_clean:
            if os.path.exists(f):
                try:
                    os.remove(f)
                except Exception:
                    pass
