import os
import re
import subprocess
import json
from typing import Dict, Any, List
import imageio_ffmpeg
import av
from mutagen import File as MutagenFile

FFMPEG_EXE = imageio_ffmpeg.get_ffmpeg_exe()


def clean_proper_title(raw_text: str) -> str:
    """
    Cleans audio filenames and raw titles into proper Title Case / CamelCase:
    - Removes 'audio_[hex]_', 'video_[hex]_' prefixes.
    - Removes file extensions (.mp3, .wav, etc.).
    - Removes copy suffixes like '(1)', '(2)', '[1]', '_1', etc.
    - Removes junk like '(Official Video)', '[Audio]', '(Lyrics)'.
    - Replaces underscores '_' and hyphens '-' with spaces.
    - Capitalizes each word cleanly into TitleCase.
    """
    if not raw_text:
        return "Untitled"
    text = str(raw_text).strip()

    # 1. Strip file extension
    text = re.sub(r'\.(?:mp3|wav|m4a|flac|ogg|aac|opus|mp4|webm|mkv)$', '', text, flags=re.IGNORECASE)

    # 2. Remove hash prefixes like 'audio_0cbae9d7_' or 'video_bb2e44bd_'
    text = re.sub(r'^(?:audio|video)_[0-9a-zA-Z]+_', '', text, flags=re.IGNORECASE)
    text = re.sub(r'^(?:audio|video)_', '', text, flags=re.IGNORECASE)

    # 3. Remove leading track numbering like '01. ', '1 - ', '01 '
    text = re.sub(r'^\d+[\.\-\s]+', '', text)

    # 4. Replace underscores with spaces early
    text = text.replace('_', ' ')

    # 5. Remove copy / duplicate / media tags: '(1)', '[1]', '(Official Audio)', '[Lyrics]', etc.
    text = re.sub(r'[\(\[\{]\s*\d+\s*[\)\]\}]', '', text)
    text = re.sub(r'[\(\[\{]\s*(?:official\s*(?:video|audio|music|lyrics?)?|lyrics?|audio|video|remaster(?:ed)?)\s*[\)\]\}]', '', text, flags=re.IGNORECASE)
    text = re.sub(r'[\s\-\(\[]+(?:copy|official|lyrics?|audio)[\s\-\)\]]*$', '', text, flags=re.IGNORECASE)

    # 6. Remove 'by ...' if title contains artist suffix like 'Song by Artist' or dangling 'by'
    text = re.sub(r'\s+by\s+.*$', '', text, flags=re.IGNORECASE)
    text = re.sub(r'\s+by$', '', text, flags=re.IGNORECASE)

    # 7. Replace hyphens and dashes with spaces
    text = re.sub(r'\s*-\s*', ' ', text)
    text = re.sub(r'(?<=[a-zA-Z0-9])-(?=[a-zA-Z0-9])', ' ', text)

    # 8. Normalize multiple spaces
    text = re.sub(r'\s+', ' ', text).strip()

    # 7. Convert words to proper Capitalized Title Case
    words = text.split()
    capitalized_words = []
    for w in words:
        if len(w) > 1 and w.isupper() and len(w) <= 4:
            capitalized_words.append(w)
        else:
            capitalized_words.append(w.capitalize())

    res = ' '.join(capitalized_words)
    return res if res else "Untitled"


def generate_youtube_timestamps(tracks: List[Dict[str, Any]]) -> str:
    """
    Generates YouTube-compatible video description chapters (timestamps):
    00:00 Title 1
    03:45 Title 2
    """
    lines = []
    cumulative_sec = 0.0
    for i, t in enumerate(tracks):
        raw = t.get("title") or t.get("filename") or f"Track {i+1}"
        title = clean_proper_title(raw)
        
        h = int(cumulative_sec // 3600)
        m = int((cumulative_sec % 3600) // 60)
        s = int(cumulative_sec % 60)
        
        if h > 0:
            time_str = f"{h:02d}:{m:02d}:{s:02d}"
        else:
            time_str = f"{m:02d}:{s:02d}"
            
        lines.append(f"{time_str} {title}")
        cumulative_sec += float(t.get("duration", 0.0))
    return "\n".join(lines)


def get_video_info(file_path: str) -> dict:
    """
    Extracts video duration, width, height, aspect ratio, fps, and audio stream info.
    """
    if not os.path.exists(file_path):
        raise FileNotFoundError(f"File not found: {file_path}")

    info = {
        "path": file_path,
        "filename": os.path.basename(file_path),
        "duration": 0.0,
        "duration_str": "00:00",
        "width": 0,
        "height": 0,
        "fps": 30.0,
        "aspect_ratio": "16:9",
        "has_audio": False,
        "size_bytes": os.path.getsize(file_path),
        "size_mb": round(os.path.getsize(file_path) / (1024 * 1024), 2)
    }

    try:
        container = av.open(file_path)
        if container.duration:
            info["duration"] = float(container.duration) / av.time_base
        
        video_streams = [s for s in container.streams if s.type == 'video']
        if video_streams:
            v = video_streams[0]
            info["width"] = v.width or 0
            info["height"] = v.height or 0
            if v.average_rate:
                info["fps"] = float(v.average_rate)
            elif v.base_rate:
                info["fps"] = float(v.base_rate)
            
            if info["width"] > 0 and info["height"] > 0:
                ratio = info["width"] / info["height"]
                if abs(ratio - 16/9) < 0.1:
                    info["aspect_ratio"] = "16:9"
                elif abs(ratio - 9/16) < 0.1:
                    info["aspect_ratio"] = "9:16"
                elif abs(ratio - 1.0) < 0.1:
                    info["aspect_ratio"] = "1:1"
                elif abs(ratio - 4/3) < 0.1:
                    info["aspect_ratio"] = "4:3"
                else:
                    info["aspect_ratio"] = f"{info['width']}:{info['height']}"

        audio_streams = [s for s in container.streams if s.type == 'audio']
        info["has_audio"] = len(audio_streams) > 0
        container.close()
    except Exception as e:
        # Fallback to mutagen or estimation
        pass

    info["duration_str"] = format_duration(info["duration"])
    return info


def get_audio_info(file_path: str) -> dict:
    """
    Extracts audio duration, title, artist, bitrate, etc.
    """
    if not os.path.exists(file_path):
        raise FileNotFoundError(f"File not found: {file_path}")

    filename = os.path.basename(file_path)
    base_title = os.path.splitext(filename)[0]

    info = {
        "path": file_path,
        "filename": filename,
        "title": clean_proper_title(base_title),
        "artist": "",
        "duration": 0.0,
        "duration_str": "00:00",
        "size_bytes": os.path.getsize(file_path),
        "size_mb": round(os.path.getsize(file_path) / (1024 * 1024), 2)
    }

    # Extract metadata using mutagen
    try:
        audio = MutagenFile(file_path)
        if audio is not None:
            if hasattr(audio.info, 'length'):
                info["duration"] = float(audio.info.length)
            
            # Read tags
            if audio.tags:
                title_tag = audio.tags.get("TIT2") or audio.tags.get("title") or audio.tags.get("\xa9nam")
                artist_tag = audio.tags.get("TPE1") or audio.tags.get("artist") or audio.tags.get("\xa9ART")
                if title_tag:
                    raw_t = str(title_tag[0] if isinstance(title_tag, (list, tuple)) else title_tag)
                    info["title"] = clean_proper_title(raw_t)
                if artist_tag:
                    raw_a = str(artist_tag[0] if isinstance(artist_tag, (list, tuple)) else artist_tag)
                    if "unknown" not in raw_a.lower():
                        info["artist"] = clean_proper_title(raw_a)
    except Exception:
        pass

    # If duration still 0, try PyAV
    if info["duration"] <= 0:
        try:
            container = av.open(file_path)
            if container.duration:
                info["duration"] = float(container.duration) / av.time_base
            container.close()
        except Exception:
            pass

    info["duration_str"] = format_duration(info["duration"])
    return info


def format_duration(seconds: float) -> str:
    """Formats seconds into MM:SS or HH:MM:SS."""
    sec = int(round(seconds))
    hrs = sec // 3600
    mins = (sec % 3600) // 60
    secs = sec % 60
    if hrs > 0:
        return f"{hrs:02d}:{mins:02d}:{secs:02d}"
    return f"{mins:02d}:{secs:02d}"
