import os
import re
import math
import warnings
import logging
from typing import List, Dict, Any, Optional
import av
from core.subtitle_styles import build_ass_header, PRESET_THEMES
from core.hardware import setup_cuda_dll_paths
from core.media_info import clean_proper_title

# Register CUDA DLL paths for cuBLAS and cuDNN on Windows
setup_cuda_dll_paths()

# Suppress harmless Hugging Face Hub unauthenticated download notices
warnings.filterwarnings("ignore", message=".*unauthenticated requests to the HF Hub.*")
warnings.filterwarnings("ignore", message=".*HF_TOKEN.*")
logging.getLogger("huggingface_hub").setLevel(logging.ERROR)

# Fix incompatibility between faster-whisper and PyAV 14+/19+ where 'metadata_errors' was removed
_orig_av_open = av.open
def _safe_av_open(*args, **kwargs):
    kwargs.pop("metadata_errors", None)
    return _orig_av_open(*args, **kwargs)
av.open = _safe_av_open

# Global cache for loaded whisper model to prevent reloading repeatedly
_CACHED_MODEL = None
_CACHED_MODEL_SIZE = None
_CACHED_DEVICE = None

def get_whisper_model(model_size: str = "base", device: str = "cuda"):
    """
    Retrieves or initializes a cached Faster-Whisper model on GPU (CUDA) or CPU.
    """
    global _CACHED_MODEL, _CACHED_MODEL_SIZE, _CACHED_DEVICE
    from faster_whisper import WhisperModel
    from core.hardware import get_hardware_info

    hw = get_hardware_info()
    target_device = "cuda" if (device == "cuda" and hw.get("cuda_whisper")) else "cpu"
    compute_type = "float16" if target_device == "cuda" else "int8"

    if _CACHED_MODEL is not None and _CACHED_MODEL_SIZE == model_size and _CACHED_DEVICE == target_device:
        return _CACHED_MODEL

    try:
        model = WhisperModel(model_size, device=target_device, compute_type=compute_type)
    except Exception:
        # Fallback to CPU if CUDA encounters memory or library limits
        target_device = "cpu"
        compute_type = "int8"
        model = WhisperModel(model_size, device="cpu", compute_type="int8")

    _CACHED_MODEL = model
    _CACHED_MODEL_SIZE = model_size
    _CACHED_DEVICE = target_device
    return model


def transcribe_audio_karaoke(
    audio_path: str,
    model_size: str = "base",
    language: Optional[str] = None,
    device: str = "cuda"
) -> List[Dict[str, Any]]:
    """
    Transcribes audio with word-level timestamps suitable for karaoke.
    Uses NVIDIA CUDA acceleration if available.
    """
    if not os.path.exists(audio_path):
        raise FileNotFoundError(f"Audio file not found: {audio_path}")

    try:
        model = get_whisper_model(model_size, device=device)
        segments_gen, info = model.transcribe(
            audio_path,
            language=language if language and language != "auto" else None,
            word_timestamps=True,
            beam_size=5,
            vad_filter=True,
            vad_parameters=dict(min_silence_duration_ms=500)
        )
        segments = list(segments_gen)
    except Exception as e:
        if device == "cuda":
            # Automatic fallback to CPU if CUDA encounters missing libraries or limits
            model = get_whisper_model(model_size, device="cpu")
            segments_gen, info = model.transcribe(
                audio_path,
                language=language if language and language != "auto" else None,
                word_timestamps=True,
                beam_size=5,
                vad_filter=True,
                vad_parameters=dict(min_silence_duration_ms=500)
            )
            segments = list(segments_gen)
        else:
            raise e

    lines = []
    for seg in segments:
        text = seg.text.strip()
        if not text:
            continue

        words_data = []
        if seg.words:
            for w in seg.words:
                w_text = w.word.strip()
                if w_text:
                    words_data.append({
                        "word": w_text,
                        "start": round(w.start, 2),
                        "end": round(w.end, 2)
                    })
        
        # If no words were detected inside segment, distribute segment text
        if not words_data and text:
            words = text.split()
            seg_dur = max(seg.end - seg.start, 0.5)
            w_dur = seg_dur / len(words)
            for idx, w in enumerate(words):
                w_start = seg.start + (idx * w_dur)
                w_end = w_start + w_dur
                words_data.append({
                    "word": w,
                    "start": round(w_start, 2),
                    "end": round(w_end, 2)
                })

        if words_data:
            lines.append({
                "start": round(words_data[0]["start"], 2),
                "end": round(words_data[-1]["end"], 2),
                "text": text,
                "words": words_data
            })

    return lines


def parse_lrc(lrc_content: str) -> List[Dict[str, Any]]:
    """
    Parses standard or enhanced LRC content into structured karaoke line data.
    """
    lines = []
    # Regular expression for [mm:ss.xx]
    lrc_pattern = re.compile(r'\[(\d+):(\d+(?:\.\d+)?)\](.*)')

    raw_lines = lrc_content.strip().splitlines()
    temp_entries = []

    for r_line in raw_lines:
        match = lrc_pattern.match(r_line.strip())
        if match:
            mins, secs, lyric = match.groups()
            start_time = int(mins) * 60 + float(secs)
            lyric_clean = lyric.strip()
            if lyric_clean:
                temp_entries.append((start_time, lyric_clean))

    # Sort by start time
    temp_entries.sort(key=lambda x: x[0])

    for i, (start_time, lyric) in enumerate(temp_entries):
        # Estimate end time based on next entry or default 4 seconds
        if i < len(temp_entries) - 1:
            end_time = min(start_time + 8.0, temp_entries[i+1][0])
            if end_time <= start_time:
                end_time = start_time + 3.0
        else:
            end_time = start_time + 4.0

        # Split words and distribute timing
        words = lyric.split()
        if not words:
            continue
        duration = end_time - start_time
        w_dur = duration / len(words)

        words_data = []
        for w_idx, w in enumerate(words):
            w_s = start_time + (w_idx * w_dur)
            w_e = w_s + w_dur
            words_data.append({
                "word": w,
                "start": round(w_s, 2),
                "end": round(w_e, 2)
            })

        lines.append({
            "start": round(start_time, 2),
            "end": round(end_time, 2),
            "text": lyric,
            "words": words_data
        })

    return lines


def format_ass_time(seconds: float) -> str:
    """Formats seconds into ASS timestamp format: H:MM:SS.cc"""
    if seconds < 0:
        seconds = 0
    h = int(seconds // 3600)
    m = int((seconds % 3600) // 60)
    s = int(seconds % 60)
    cs = int(round((seconds - int(seconds)) * 100))
    if cs >= 100:
        s += 1
        cs -= 100
        if s >= 60:
            m += 1
            s -= 60
            if m >= 60:
                h += 1
                m -= 60
    return f"{h}:{m:02d}:{s:02d}.{cs:02d}"


def generate_karaoke_dialogue_line(
    line: Dict[str, Any],
    time_offset: float = 0.0,
    wipe_mode: str = "smooth"  # 'smooth' (\kf) or 'jump' (\k)
) -> str:
    """
    Builds an ASS Karaoke Dialogue line with progressive highlight tags.
    """
    line_start = line["start"] + time_offset
    line_end = line["end"] + time_offset
    ass_start = format_ass_time(line_start)
    ass_end = format_ass_time(line_end)

    words = line.get("words", [])
    if not words:
        # Fallback if no words: just display text
        return f"Dialogue: 0,{ass_start},{ass_end},Karaoke,,0,0,0,,{line.get('text', '')}"

    tag = "\\kf" if wipe_mode == "smooth" else "\\k"

    ass_text_parts = []
    current_time = line_start

    for i, w in enumerate(words):
        w_start = w["start"] + time_offset
        w_end = w["end"] + time_offset
        w_text = w["word"]

        # If there's a pre-word gap
        gap = w_start - current_time
        if gap > 0.05:
            gap_cs = int(round(gap * 100))
            ass_text_parts.append(f"{{\\k{gap_cs}}}")

        # Active singing duration for this word
        w_duration = max(w_end - w_start, 0.1)
        dur_cs = max(int(round(w_duration * 100)), 10)
        ass_text_parts.append(f"{{{tag}{dur_cs}}}{w_text} ")

        current_time = w_end

    dialogue_content = "".join(ass_text_parts).rstrip()
    return f"Dialogue: 0,{ass_start},{ass_end},Karaoke,,0,0,0,,{dialogue_content}"


def build_full_ass_file(
    tracks: List[Dict[str, Any]],
    output_ass_path: str,
    video_width: int = 1920,
    video_height: int = 1080,
    style_config: Optional[Dict[str, Any]] = None
) -> str:
    """
    Generates a complete .ass subtitle file for all tracks in playlist.
    Each track's lyrics are shifted according to cumulative playlist duration.
    """
    cfg = style_config or {}
    font_name = cfg.get("font_name", "Arial")
    font_size = int(cfg.get("font_size", 48 if video_height >= 1080 else 32))
    normal_color = cfg.get("normal_color", "#FFFFFF")
    singing_color = cfg.get("singing_color", "#FFD700")
    outline_color = cfg.get("outline_color", "#000000")
    shadow_color = cfg.get("shadow_color", "#000000")
    alignment = int(cfg.get("alignment", 2))  # 2: bottom center
    margin_v = int(cfg.get("margin_v", 70))
    show_intro = cfg.get("show_intro", True)
    wipe_mode = cfg.get("wipe_mode", "smooth")

    header = build_ass_header(
        width=video_width,
        height=video_height,
        font_name=font_name,
        font_size=font_size,
        normal_color=normal_color,
        singing_color=singing_color,
        outline_color=outline_color,
        shadow_color=shadow_color,
        alignment=alignment,
        margin_v=margin_v,
        title_banner=show_intro
    )

    events = []
    cumulative_time = 0.0

    for track_idx, track in enumerate(tracks):
        track_title = track.get("title", f"Track {track_idx+1}")
        track_artist = track.get("artist", "")
        track_duration = track.get("duration", 0.0)
        lyrics = track.get("lyrics", [])

        # Add Song Title Intro Splash Card (displays for first 5 seconds of the track)
        if show_intro and track_duration > 5.0:
            splash_start = format_ass_time(cumulative_time + 0.5)
            splash_end = format_ass_time(cumulative_time + 5.5)
            clean_t = clean_proper_title(track_title)
            clean_a = clean_proper_title(track_artist) if track_artist and "unknown" not in track_artist.lower() else ""

            # Professional intro banner: clean CamelCase/TitleCase, without "by"
            if clean_a and clean_a.lower() != clean_t.lower():
                intro_text = f"{{\\fad(600,600)}}🎵 {clean_t} \\N {{\\fs{int(font_size*0.65)}}}{clean_a}"
            else:
                intro_text = f"{{\\fad(600,600)}}🎵 {clean_t}"

            events.append(f"Dialogue: 1,{splash_start},{splash_end},SongTitle,,0,0,0,,{intro_text}")

        # Add Karaoke Lines
        for line in lyrics:
            dialogue_line = generate_karaoke_dialogue_line(
                line=line,
                time_offset=cumulative_time,
                wipe_mode=wipe_mode
            )
            events.append(dialogue_line)

        cumulative_time += track_duration

    ass_content = header + "\n".join(events) + "\n"

    # Write ASS file with UTF-8 encoding
    os.makedirs(os.path.dirname(os.path.abspath(output_ass_path)), exist_ok=True)
    with open(output_ass_path, "w", encoding="utf-8") as f:
        f.write(ass_content)

    return output_ass_path
