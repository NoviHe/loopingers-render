import os
import sys
import uuid
import time
import shutil
import threading
import subprocess
import warnings
import logging
from typing import List, Optional
import av

# Suppress harmless Hugging Face Hub unauthenticated download notices
warnings.filterwarnings("ignore", message=".*unauthenticated requests to the HF Hub.*")
warnings.filterwarnings("ignore", message=".*HF_TOKEN.*")
logging.getLogger("huggingface_hub").setLevel(logging.ERROR)

# Patch PyAV 14+/19+ metadata_errors incompatibility with faster-whisper
_orig_av_open = av.open
def _safe_av_open(*args, **kwargs):
    kwargs.pop("metadata_errors", None)
    return _orig_av_open(*args, **kwargs)
av.open = _safe_av_open

from fastapi import FastAPI, File, UploadFile, Form, HTTPException, BackgroundTasks
from fastapi.responses import HTMLResponse, FileResponse, JSONResponse
from fastapi.staticfiles import StaticFiles
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel

from core.media_info import get_video_info, get_audio_info, format_duration, clean_proper_title, generate_youtube_timestamps
from core.karaoke_engine import transcribe_audio_karaoke, parse_lrc, build_full_ass_file
from core.video_looper import render_looping_karaoke_video, RenderJob, RESOLUTION_PRESETS
from core.subtitle_styles import PRESET_THEMES
from core.storage_manager import get_full_storage_stats, cleanup_storage, delete_specific_file
from core.hardware import get_hardware_info
from core.thumbnail_generator import extract_video_frame, build_thumbnail_image
from core.image_converter import convert_image_to_video, is_image_file, estimate_tempo_bpm
from core.visual_effects import get_visual_effects_list

app = FastAPI(title="LoopKaraoke Studio", description="Loop Video Background, Merge Songs, & Auto Karaoke Subtitles")

# Allow CORS for development flexibility
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

@app.on_event("startup")
async def startup_cleanup():
    """Auto-clean temporary files (.ass and concat) older than 24 hours on boot."""
    try:
        cleanup_storage(clean_temp_only=True, max_age_hours=24)
    except Exception:
        pass

def get_bundle_dir():
    if getattr(sys, "frozen", False) and hasattr(sys, "_MEIPASS"):
        return sys._MEIPASS
    return os.path.abspath(os.path.dirname(__file__))

BUNDLE_DIR = get_bundle_dir()

# If running as PyInstaller executable, set working directory to executable directory
if getattr(sys, "frozen", False):
    try:
        os.chdir(os.path.dirname(sys.executable))
    except Exception:
        pass

# Ensure writable user directories exist
os.makedirs("uploads", exist_ok=True)
os.makedirs("outputs", exist_ok=True)

static_path = os.path.join(BUNDLE_DIR, "static")
if not os.path.exists(static_path):
    static_path = "static"

app.mount("/static", StaticFiles(directory=static_path), name="static")
app.mount("/uploads", StaticFiles(directory="uploads"), name="uploads")
app.mount("/outputs", StaticFiles(directory="outputs"), name="outputs")

# Memory store for jobs and transcripts
ACTIVE_JOBS = {}


class TranscribeRequest(BaseModel):
    audio_path: str
    model_size: str = "base"
    language: Optional[str] = "auto"
    device: Optional[str] = "cuda"


class ParseLrcRequest(BaseModel):
    lrc_content: str


class LyricWord(BaseModel):
    word: str
    start: float
    end: float


class LyricLine(BaseModel):
    start: float
    end: float
    text: str
    words: List[LyricWord]


class TrackData(BaseModel):
    id: str
    path: str
    filename: str
    title: str
    artist: str
    duration: float
    lyrics: List[LyricLine] = []


class RenderRequest(BaseModel):
    video_path: str
    tracks: List[TrackData]
    aspect_ratio: str = "16:9"
    resolution: str = "1080p"
    fit_mode: str = "crop"
    fps: int = 30
    video_volume: float = 0.0
    crossfade_duration: float = 0.0
    style_config: dict = {}
    encoder: str = "nvenc_h264"
    preset: str = "p4"
    quality_cq: int = 22
    use_hwaccel: bool = True
    beat_pulse: bool = True
    is_from_image: bool = False
    source_image_path: Optional[str] = None
    image_effect: str = "zoom_in"
    visual_effect: str = "none"


class StorageCleanRequest(BaseModel):
    clean_uploads: bool = False
    clean_outputs: bool = False
    clean_temp_only: bool = False
    max_age_hours: Optional[float] = None


class DeleteFileRequest(BaseModel):
    folder: str
    filename: str


class ExtractFrameRequest(BaseModel):
    video_path: str
    timestamp: float = 0.0


class SaveThumbnailRequest(BaseModel):
    image_data: str
    filename: Optional[str] = None


class GenerateServerThumbnailRequest(BaseModel):
    video_path: str
    timestamp: float = 0.0
    title: str = "Powerful Worship"
    subtitle: str = "Hillsong Worship"
    track_titles: List[str] = []
    column_mode: str = "auto"
    number_style: str = "mirrored"
    title_font_size: int = 120
    track_font_size: int = 36
    vignette_opacity: float = 0.55
    title_color: str = "#FFFFFF"
    track_color: str = "#FFFFFF"
    subtitle_color: str = "#FFD700"


class TimestampTrackItem(BaseModel):
    title: Optional[str] = None
    filename: Optional[str] = None
    duration: float = 0.0
    artist: Optional[str] = None


class TimestampsRequest(BaseModel):
    tracks: List[TimestampTrackItem]


@app.get("/", response_class=HTMLResponse)
async def serve_index():
    index_file = os.path.join(BUNDLE_DIR, "templates", "index.html")
    if not os.path.exists(index_file):
        index_file = os.path.join("templates", "index.html")
    if os.path.exists(index_file):
        with open(index_file, "r", encoding="utf-8") as f:
            return HTMLResponse(content=f.read())
    return HTMLResponse("<h1>LoopKaraoke Studio</h1><p>Index template loading...</p>")


@app.get("/api/presets")
async def get_presets():
    return {
        "themes": PRESET_THEMES,
        "resolutions": RESOLUTION_PRESETS
    }


@app.get("/api/visual-effects")
async def get_visual_effects():
    """Returns available visual atmosphere effects and filters."""
    return get_visual_effects_list()


@app.get("/api/hardware")
async def get_hardware():
    """Returns detected GPU, CUDA and NVENC hardware encoder status."""
    return get_hardware_info()


@app.post("/api/upload/image-to-video")
async def upload_image_to_video_endpoint(
    file: UploadFile = File(...),
    duration: float = Form(10.0),
    fps: int = Form(30),
    aspect_ratio: str = Form("16:9"),
    resolution: str = Form("1080p"),
    effect: str = Form("zoom_in"),
    beat_pulse: bool = Form(True),
    bpm: float = Form(105.0),
    visual_effect: str = Form("none")
):
    """Uploads an image (JPG, PNG, WEBP, JFIF) and converts it into a looping MP4 video background."""
    if not is_image_file(file.filename):
        raise HTTPException(
            status_code=400,
            detail="Format gambar tidak didukung. Harap gunakan format JPG, JPEG, PNG, WEBP, atau JFIF."
        )

    img_uid = uuid.uuid4().hex[:8]
    img_name = f"image_{img_uid}_{file.filename}"
    img_path = os.path.abspath(os.path.join("uploads", img_name))

    with open(img_path, "wb") as buffer:
        shutil.copyfileobj(file.file, buffer)

    out_video_name = f"video_from_img_{img_uid}.mp4"
    out_video_path = os.path.abspath(os.path.join("uploads", out_video_name))

    try:
        info = convert_image_to_video(
            image_path=img_path,
            output_path=out_video_path,
            duration=duration,
            fps=fps,
            aspect_ratio=aspect_ratio,
            resolution=resolution,
            effect=effect,
            beat_pulse=beat_pulse,
            bpm=bpm,
            visual_effect=visual_effect
        )
        info["web_url"] = f"/uploads/{out_video_name}"
        info["source_image_url"] = f"/uploads/{img_name}"
        info["source_image_name"] = file.filename
        info["source_image_path"] = img_path
        info["is_from_image"] = True
        info["beat_pulse"] = beat_pulse
        info["visual_effect"] = visual_effect
        return info
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Gagal mengonversi gambar ke video background: {str(e)}")


@app.post("/api/upload/video")
async def upload_video(file: UploadFile = File(...)):
    """Uploads background video or automatically converts uploaded images to video background."""
    # Check if user dragged/dropped an image instead of video
    if is_image_file(file.filename):
        img_uid = uuid.uuid4().hex[:8]
        img_name = f"image_{img_uid}_{file.filename}"
        img_path = os.path.abspath(os.path.join("uploads", img_name))
        with open(img_path, "wb") as buffer:
            shutil.copyfileobj(file.file, buffer)

        out_video_name = f"video_from_img_{img_uid}.mp4"
        out_video_path = os.path.abspath(os.path.join("uploads", out_video_name))
        try:
            info = convert_image_to_video(
                image_path=img_path,
                output_path=out_video_path,
                duration=10.0,
                fps=30,
                aspect_ratio="16:9",
                resolution="1080p",
                effect="zoom_in",
                beat_pulse=True,
                bpm=105.0
            )
            info["web_url"] = f"/uploads/{out_video_name}"
            info["source_image_url"] = f"/uploads/{img_name}"
            info["source_image_name"] = file.filename
            info["source_image_path"] = img_path
            info["is_from_image"] = True
            info["beat_pulse"] = True
            return info
        except Exception as e:
            raise HTTPException(status_code=500, detail=f"Gagal mengonversi gambar ke video: {str(e)}")

    file_ext = os.path.splitext(file.filename)[1].lower()
    if file_ext not in [".mp4", ".mov", ".mkv", ".webm", ".avi"]:
        raise HTTPException(
            status_code=400,
            detail="Format tidak didukung. Harap gunakan file video (MP4, MOV, WEBM, MKV) atau gambar (JPG, PNG, WEBP, JFIF)."
        )

    unique_name = f"video_{uuid.uuid4().hex[:8]}_{file.filename}"
    save_path = os.path.join("uploads", unique_name)

    with open(save_path, "wb") as buffer:
        shutil.copyfileobj(file.file, buffer)

    info = get_video_info(save_path)
    info["web_url"] = f"/uploads/{unique_name}"
    info["is_from_image"] = False
    return info


@app.post("/api/upload/audios")
async def upload_audios(files: List[UploadFile] = File(...)):
    """Uploads multiple songs and extracts metadata for each."""
    uploaded_tracks = []
    allowed_exts = [".mp3", ".wav", ".m4a", ".aac", ".flac", ".ogg", ".opus"]

    for file in files:
        file_ext = os.path.splitext(file.filename)[1].lower()
        if file_ext not in allowed_exts:
            continue

        unique_name = f"audio_{uuid.uuid4().hex[:8]}_{file.filename}"
        save_path = os.path.join("uploads", unique_name)

        with open(save_path, "wb") as buffer:
            shutil.copyfileobj(file.file, buffer)

        info = get_audio_info(save_path)
        info["id"] = uuid.uuid4().hex[:8]
        info["web_url"] = f"/uploads/{unique_name}"
        info["lyrics"] = []
        uploaded_tracks.append(info)

    if not uploaded_tracks:
        raise HTTPException(status_code=400, detail="Tidak ada file audio yang valid (gunakan MP3, WAV, M4A, FLAC).")

    return uploaded_tracks


@app.post("/api/transcribe")
async def transcribe_audio(req: TranscribeRequest):
    """Generates word-level timestamps for karaoke using Faster-Whisper."""
    full_path = req.audio_path
    if not os.path.isabs(full_path):
        full_path = os.path.abspath(full_path)

    if not os.path.exists(full_path):
        raise HTTPException(status_code=404, detail="File audio tidak ditemukan.")

    try:
        lyrics = transcribe_audio_karaoke(
            audio_path=full_path,
            model_size=req.model_size,
            language=req.language,
            device=req.device or "cuda"
        )
        return {"success": True, "lyrics": lyrics}
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Gagal mentranskripsi audio: {str(e)}")


@app.post("/api/parse-lrc")
async def parse_lrc_endpoint(req: ParseLrcRequest):
    """Parses LRC format lyrics into structured karaoke format."""
    try:
        lyrics = parse_lrc(req.lrc_content)
        return {"success": True, "lyrics": lyrics}
    except Exception as e:
        raise HTTPException(status_code=400, detail=f"Gagal memproses file LRC: {str(e)}")


def run_render_task(job: RenderJob, req_data: RenderRequest):
    """Background rendering worker."""
    try:
        job.status = "preparing"
        video_path = req_data.video_path
        if not os.path.isabs(video_path):
            video_path = os.path.abspath(video_path)

        audio_paths = []
        tracks_dict = []
        total_audio_duration = 0.0

        for t in req_data.tracks:
            a_path = t.path
            if not os.path.isabs(a_path):
                a_path = os.path.abspath(a_path)
            audio_paths.append(a_path)
            total_audio_duration += t.duration

            lyrics_converted = []
            for line in t.lyrics:
                lyrics_converted.append({
                    "start": line.start,
                    "end": line.end,
                    "text": line.text,
                    "words": [{"word": w.word, "start": w.start, "end": w.end} for w in line.words]
                })

            clean_title = clean_proper_title(t.title or t.filename)
            clean_artist = clean_proper_title(t.artist) if t.artist and "unknown" not in t.artist.lower() else ""
            tracks_dict.append({
                "title": clean_title,
                "artist": clean_artist,
                "duration": t.duration,
                "lyrics": lyrics_converted
            })

        # If background is an image, regenerate background video matching playlist tempo, beat pulse, and visual effect
        is_effect_baked = False
        if req_data.is_from_image and req_data.source_image_path and os.path.exists(req_data.source_image_path):
            try:
                first_audio = audio_paths[0] if audio_paths else None
                bpm_val = estimate_tempo_bpm(first_audio) if first_audio else 105.0
                img_bg_video = os.path.abspath(f"uploads/bg_render_{job.job_id}.mp4")
                convert_image_to_video(
                    image_path=req_data.source_image_path,
                    output_path=img_bg_video,
                    duration=10.0,
                    fps=req_data.fps,
                    aspect_ratio=req_data.aspect_ratio,
                    resolution=req_data.resolution,
                    effect=req_data.image_effect or "zoom_in",
                    beat_pulse=req_data.beat_pulse,
                    bpm=bpm_val,
                    visual_effect=req_data.visual_effect or "none"
                )
                video_path = img_bg_video
                is_effect_baked = True
            except Exception:
                pass

        # 1. Generate ASS subtitle file
        ass_path = os.path.abspath(f"uploads/subtitles_{job.job_id}.ass")
        preset_dict = RESOLUTION_PRESETS.get(req_data.aspect_ratio, RESOLUTION_PRESETS["16:9"])
        res_w, res_h = preset_dict.get(req_data.resolution, (1920, 1080))

        build_full_ass_file(
            tracks=tracks_dict,
            output_ass_path=ass_path,
            video_width=res_w,
            video_height=res_h,
            style_config=req_data.style_config
        )

        # 2. Render output video
        output_filename = f"karaoke_video_{job.job_id}.mp4"
        output_path = os.path.abspath(os.path.join("outputs", output_filename))

        # Only apply effect in render looper if it wasn't already baked into image background video
        render_looper_effect = "none" if is_effect_baked else (req_data.visual_effect or "none")

        render_looping_karaoke_video(
            video_path=video_path,
            audio_paths=audio_paths,
            output_path=output_path,
            total_duration=total_audio_duration,
            ass_subtitle_path=ass_path,
            aspect_ratio=req_data.aspect_ratio,
            resolution=req_data.resolution,
            fit_mode=req_data.fit_mode,
            fps=req_data.fps,
            video_volume=req_data.video_volume,
            crossfade_duration=req_data.crossfade_duration,
            encoder=req_data.encoder,
            preset=req_data.preset,
            quality_cq=req_data.quality_cq,
            use_hwaccel=req_data.use_hwaccel,
            visual_effect=render_looper_effect,
            job=job
        )

    except Exception as e:
        job.status = "failed"
        job.error_message = str(e)


@app.post("/api/render")
async def start_render(req: RenderRequest, background_tasks: BackgroundTasks):
    """Starts the video looping & karaoke burning render process."""
    if not req.tracks:
        raise HTTPException(status_code=400, detail="Harap masukkan minimal 1 lagu dalam daftar putar.")
    if not req.video_path or not os.path.exists(req.video_path):
        raise HTTPException(status_code=400, detail="File video background tidak ditemukan.")

    job_id = uuid.uuid4().hex[:10]
    job = RenderJob(job_id)
    ACTIVE_JOBS[job_id] = job

    background_tasks.add_task(run_render_task, job, req)
    return {"job_id": job_id, "status": "queued"}


@app.get("/api/render/status/{job_id}")
async def get_render_status(job_id: str):
    job = ACTIVE_JOBS.get(job_id)
    if not job:
        raise HTTPException(status_code=404, detail="Pekerjaan render tidak ditemukan.")
    
    data = job.to_dict()
    if job.output_file and os.path.exists(job.output_file):
        data["download_url"] = f"/outputs/{os.path.basename(job.output_file)}"
    return data


@app.post("/api/open-folder")
async def open_outputs_folder():
    """Opens the outputs directory in Windows File Explorer."""
    out_dir = os.path.abspath("outputs")
    try:
        subprocess.Popen(["explorer.exe", out_dir])
        return {"success": True, "folder": out_dir}
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


@app.get("/api/storage/stats")
async def get_storage_stats_endpoint():
    """Returns storage metrics for uploads, outputs, and model cache."""
    return get_full_storage_stats()


@app.post("/api/storage/clean")
async def clean_storage_endpoint(req: StorageCleanRequest):
    """Purges selected files or temporary cache."""
    result = cleanup_storage(
        clean_uploads=req.clean_uploads,
        clean_outputs=req.clean_outputs,
        clean_temp_only=req.clean_temp_only,
        max_age_hours=req.max_age_hours
    )
    return {
        "success": True,
        "deleted": result,
        "stats": get_full_storage_stats()
    }


@app.post("/api/storage/delete-file")
async def delete_file_endpoint(req: DeleteFileRequest):
    """Deletes a specific file safely."""
    success = delete_specific_file(req.folder, req.filename)
    if not success:
        raise HTTPException(status_code=400, detail="Gagal menghapus file atau file tidak ditemukan.")
    return {
        "success": True,
        "stats": get_full_storage_stats()
    }


@app.post("/api/thumbnail/extract-frame")
async def extract_thumbnail_frame_endpoint(req: ExtractFrameRequest):
    """Extracts a frame from video at the given timestamp for thumbnail generation."""
    v_path = req.video_path
    if not os.path.isabs(v_path):
        v_path = os.path.abspath(v_path)

    if not os.path.exists(v_path):
        raise HTTPException(status_code=404, detail="File video tidak ditemukan.")
    
    unique_name = f"frame_{uuid.uuid4().hex[:8]}_{int(req.timestamp*1000)}.jpg"
    out_path = os.path.abspath(os.path.join("uploads", unique_name))
    try:
        extract_video_frame(v_path, req.timestamp, out_path)
        return {
            "success": True,
            "frame_url": f"/uploads/{unique_name}",
            "frame_path": out_path,
            "timestamp": req.timestamp
        }
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


@app.post("/api/thumbnail/save")
async def save_thumbnail_endpoint(req: SaveThumbnailRequest):
    """Saves a client-rendered canvas thumbnail to the outputs folder."""
    import base64
    try:
        img_str = req.image_data
        if "," in img_str:
            img_str = img_str.split(",", 1)[1]
        
        img_bytes = base64.b64decode(img_str)
        fname = req.filename or f"thumbnail_{int(time.time())}.jpg"
        if not fname.lower().endswith(".jpg") and not fname.lower().endswith(".jpeg"):
            fname += ".jpg"
        
        out_path = os.path.abspath(os.path.join("outputs", fname))
        with open(out_path, "wb") as f:
            f.write(img_bytes)
        
        return {
            "success": True,
            "download_url": f"/outputs/{fname}",
            "filename": fname,
            "file_path": out_path
        }
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Gagal menyimpan thumbnail: {str(e)}")


@app.post("/api/thumbnail/generate")
async def generate_thumbnail_server_endpoint(req: GenerateServerThumbnailRequest):
    """Renders thumbnail server-side with Pillow and returns saved file."""
    v_path = req.video_path
    if not os.path.isabs(v_path):
        v_path = os.path.abspath(v_path)

    if not os.path.exists(v_path):
        raise HTTPException(status_code=404, detail="File video sumber tidak ditemukan.")
    
    fname = f"thumbnail_{uuid.uuid4().hex[:8]}.jpg"
    out_path = os.path.abspath(os.path.join("outputs", fname))
    try:
        build_thumbnail_image(
            background_source=v_path,
            output_path=out_path,
            title=req.title,
            subtitle=req.subtitle,
            track_titles=req.track_titles,
            timestamp_sec=req.timestamp,
            column_mode=req.column_mode,
            number_style=req.number_style,
            title_font_size=req.title_font_size,
            track_font_size=req.track_font_size,
            vignette_opacity=req.vignette_opacity,
            title_color=req.title_color,
            track_color=req.track_color,
            subtitle_color=req.subtitle_color
        )
        return {
            "success": True,
            "download_url": f"/outputs/{fname}",
            "filename": fname,
            "file_path": out_path
        }
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Gagal memproses thumbnail: {str(e)}")


@app.post("/api/timestamps")
async def get_youtube_timestamps_endpoint(req: TimestampsRequest):
    """Generates formatted YouTube video description timestamps."""
    tracks_data = [t.dict() for t in req.tracks]
    timestamps_text = generate_youtube_timestamps(tracks_data)

    chapters = []
    cum = 0.0
    for i, t in enumerate(tracks_data):
        raw = t.get("title") or t.get("filename") or f"Track {i+1}"
        title = clean_proper_title(raw)
        h = int(cum // 3600)
        m = int((cum % 3600) // 60)
        s = int(cum % 60)
        time_str = f"{h:02d}:{m:02d}:{s:02d}" if h > 0 else f"{m:02d}:{s:02d}"
        chapters.append({
            "index": i + 1,
            "seconds": cum,
            "time_str": time_str,
            "title": title,
            "duration": float(t.get("duration", 0.0))
        })
        cum += float(t.get("duration", 0.0))

    return {
        "success": True,
        "timestamps_text": timestamps_text,
        "chapters": chapters,
        "total_duration": cum
    }


@app.post("/api/sample-demo")
async def generate_sample_demo():
    """Generates synthetic demo video and 2 demo songs with synced karaoke lyrics for instant testing."""
    import imageio_ffmpeg
    ffmpeg = imageio_ffmpeg.get_ffmpeg_exe()

    demo_video_path = os.path.abspath("uploads/demo_background_loop.mp4")
    demo_song1_path = os.path.abspath("uploads/demo_lagu_1.mp3")
    demo_song2_path = os.path.abspath("uploads/demo_lagu_2.mp3")

    # 1. Generate 5-second animated gradient loop video if not already existing
    if not os.path.exists(demo_video_path):
        subprocess.run([
            ffmpeg, "-y",
            "-f", "lavfi", "-i", "mandelbrot=size=1280x720:rate=30",
            "-t", "5",
            "-pix_fmt", "yuv420p",
            "-c:v", "libx264",
            demo_video_path
        ], check=True, cwd="c:\\loopingapp")

    # 2. Generate 10-second audio track 1
    if not os.path.exists(demo_song1_path):
        subprocess.run([
            ffmpeg, "-y",
            "-f", "lavfi", "-i", "sine=frequency=523.25:duration=10",
            "-c:a", "libmp3lame",
            demo_song1_path
        ], check=True, cwd="c:\\loopingapp")

    # 3. Generate 8-second audio track 2
    if not os.path.exists(demo_song2_path):
        subprocess.run([
            ffmpeg, "-y",
            "-f", "lavfi", "-i", "sine=frequency=659.25:duration=8",
            "-c:a", "libmp3lame",
            demo_song2_path
        ], check=True, cwd="c:\\loopingapp")

    v_info = get_video_info(demo_video_path)
    v_info["web_url"] = "/uploads/demo_background_loop.mp4"

    t1_info = get_audio_info(demo_song1_path)
    t1_info["id"] = "demo-1"
    t1_info["title"] = "Kemesraan (Sampel Demo 1)"
    t1_info["artist"] = "Iwan Fals"
    t1_info["web_url"] = "/uploads/demo_lagu_1.mp3"
    t1_info["lyrics"] = [
        {
            "start": 1.0,
            "end": 5.0,
            "text": "Kemesraan ini janganlah cepat berlalu",
            "words": [
                {"word": "Kemesraan", "start": 1.0, "end": 2.0},
                {"word": "ini", "start": 2.0, "end": 2.5},
                {"word": "janganlah", "start": 2.5, "end": 3.4},
                {"word": "cepat", "start": 3.4, "end": 4.1},
                {"word": "berlalu", "start": 4.1, "end": 5.0}
            ]
        },
        {
            "start": 5.5,
            "end": 9.5,
            "text": "Hatiku damai jiwaku tentram bersamamu",
            "words": [
                {"word": "Hatiku", "start": 5.5, "end": 6.3},
                {"word": "damai", "start": 6.3, "end": 7.1},
                {"word": "jiwaku", "start": 7.1, "end": 7.9},
                {"word": "tentram", "start": 7.9, "end": 8.7},
                {"word": "bersamamu", "start": 8.7, "end": 9.5}
            ]
        }
    ]

    t2_info = get_audio_info(demo_song2_path)
    t2_info["id"] = "demo-2"
    t2_info["title"] = "Pelangi (Sampel Demo 2)"
    t2_info["artist"] = "AT Mahmud"
    t2_info["web_url"] = "/uploads/demo_lagu_2.mp3"
    t2_info["lyrics"] = [
        {
            "start": 1.0,
            "end": 4.5,
            "text": "Pelangi pelangi alangkah indahmu",
            "words": [
                {"word": "Pelangi", "start": 1.0, "end": 1.8},
                {"word": "pelangi", "start": 1.8, "end": 2.6},
                {"word": "alangkah", "start": 2.6, "end": 3.5},
                {"word": "indahmu", "start": 3.5, "end": 4.5}
            ]
        },
        {
            "start": 5.0,
            "end": 7.8,
            "text": "Merah kuning hijau di langit yang biru",
            "words": [
                {"word": "Merah", "start": 5.0, "end": 5.6},
                {"word": "kuning", "start": 5.6, "end": 6.1},
                {"word": "hijau", "start": 6.1, "end": 6.7},
                {"word": "di langit", "start": 6.7, "end": 7.2},
                {"word": "yang biru", "start": 7.2, "end": 7.8}
            ]
        }
    ]

    return {
        "video": v_info,
        "tracks": [t1_info, t2_info]
    }

