import os
import math
import time
import subprocess
import numpy as np
from PIL import Image, ImageDraw
import sys
import imageio_ffmpeg
from typing import Dict, Any, Optional, Tuple

FFMPEG_EXE = imageio_ffmpeg.get_ffmpeg_exe()

def get_effects_dir():
    if getattr(sys, 'frozen', False) and hasattr(sys, '_MEIPASS'):
        bundle_effects = os.path.join(sys._MEIPASS, "assets", "effects")
        if os.path.exists(bundle_effects):
            return bundle_effects
    return os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "assets", "effects"))

EFFECTS_DIR = get_effects_dir()

VISUAL_EFFECTS_CONFIG = {
    "none": {
        "id": "none",
        "name": "Tanpa Efek (Asli)",
        "desc": "Tampilan video murni tanpa efek overlay",
        "type": "none",
        "icon": "ban"
    },
    "rain": {
        "id": "rain",
        "name": "Hujan (Rain)",
        "desc": "Tetesan air dan kilasan rintik hujan lembut",
        "type": "overlay",
        "icon": "cloud-rain",
        "overlay_file": "rain.mp4",
        "blend_mode": "screen",
        "opacity": 0.85
    },
    "snow": {
        "id": "snow",
        "name": "Salju (Snow)",
        "desc": "Kepingan salju melayang lembut bergoyang",
        "type": "overlay",
        "icon": "snowflake",
        "overlay_file": "snow.mp4",
        "blend_mode": "screen",
        "opacity": 0.85
    },
    "fire": {
        "id": "fire",
        "name": "Bara Api (Fire Embers)",
        "desc": "Percikan bara api keemasan membubung ke atas",
        "type": "overlay",
        "icon": "flame",
        "overlay_file": "fire.mp4",
        "blend_mode": "screen",
        "opacity": 0.90
    },
    "bokeh": {
        "id": "bokeh",
        "name": "Bokeh Sinematik",
        "desc": "Bulatan cahaya keemasan hangat melayang lembut",
        "type": "overlay",
        "icon": "sparkles",
        "overlay_file": "bokeh.mp4",
        "blend_mode": "screen",
        "opacity": 0.80
    },
    "light_leak": {
        "id": "light_leak",
        "name": "Light Leak (Sunflare)",
        "desc": "Kilau cahaya sinematik hangat menyapu sudut",
        "type": "overlay",
        "icon": "sun",
        "overlay_file": "light_leak.mp4",
        "blend_mode": "screen",
        "opacity": 0.75
    },
    "color_grading": {
        "id": "color_grading",
        "name": "Color Grading (Teal & Orange)",
        "desc": "Grading warna blockbuster Hollywood profesional",
        "type": "filter",
        "icon": "palette",
        "vf": "colorbalance=rs=0.08:gs=-0.02:bs=-0.08:rh=-0.06:gh=0.02:bh=0.08:rm=0.04:gm=0:bm=-0.03,eq=saturation=1.18:contrast=1.10:brightness=-0.01"
    },
    "vhs": {
        "id": "vhs",
        "name": "VHS Retro (90s Analog)",
        "desc": "Scanline televisi tabung, glitch RGB & noise pita analog",
        "type": "filter",
        "icon": "tv",
        "vf": "rgbashift=rh=-3:bh=3:rv=0:bv=0,drawgrid=width=iw:height=4:thickness=1:color=black@0.22,noise=alls=13:allf=t+u,eq=contrast=1.08:saturation=0.88:brightness=0.02,vignette=PI/4"
    },
    "glitch": {
        "id": "glitch",
        "name": "Glitch Digital (Cyberpunk)",
        "desc": "Distorsi RGB split cybernetic & kedipan sinyal digital",
        "type": "filter",
        "icon": "zap",
        "vf": "rgbashift=rh='if(lt(mod(n,48),4),12,0)':bh='if(lt(mod(n,48),4),-12,0)':rv='if(lt(mod(n,48),4),2,0)',noise=alls='if(lt(mod(n,48),4),35,4)':allf=t+u"
    }
}


def ensure_effect_asset(effect_key: str) -> Optional[str]:
    """
    Ensures that the requested overlay video file exists in assets/effects.
    If missing, automatically generates the seamless looping 1280x720 30fps MP4.
    """
    cfg = VISUAL_EFFECTS_CONFIG.get(effect_key)
    if not cfg or cfg.get("type") != "overlay":
        return None

    os.makedirs(EFFECTS_DIR, exist_ok=True)
    overlay_filename = cfg.get("overlay_file")
    overlay_path = os.path.join(EFFECTS_DIR, overlay_filename)

    if os.path.exists(overlay_path) and os.path.getsize(overlay_path) > 10000:
        return overlay_path

    # Generate overlay video procedurally
    w, h = 1280, 720
    fps = 30
    duration = 4.0
    total_frames = int(duration * fps)

    cmd = [
        FFMPEG_EXE, "-y",
        "-f", "rawvideo", "-vcodec", "rawvideo",
        "-s", f"{w}x{h}", "-pix_fmt", "rgb24", "-r", str(fps),
        "-i", "-",
        "-c:v", "libx264", "-preset", "ultrafast", "-crf", "20",
        "-pix_fmt", "yuv420p",
        overlay_path
    ]

    proc = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    if effect_key == "rain":
        np.random.seed(101)
        n_drops = 220
        rx = np.random.uniform(0, w, n_drops)
        ry = np.random.uniform(0, h, n_drops)
        r_speed = np.random.uniform(900, 1600, n_drops)
        r_len = np.random.uniform(18, 42, n_drops)

        for frame_idx in range(total_frames):
            t = frame_idx / fps
            img = Image.new("RGB", (w, h), (0, 0, 0))
            draw = ImageDraw.Draw(img)
            for i in range(n_drops):
                cy = (ry[i] + r_speed[i] * t) % (h + 60) - 30
                cx = (rx[i] + (r_speed[i] * 0.22) * t) % (w + 40) - 20
                lx = cx - r_len[i] * 0.22
                ly = cy - r_len[i]
                draw.line([(lx, ly), (cx, cy)], fill=(190, 215, 245), width=1)
            proc.stdin.write(img.tobytes())

    elif effect_key == "snow":
        np.random.seed(202)
        n_snow = 120
        sx = np.random.uniform(0, w, n_snow)
        sy = np.random.uniform(0, h, n_snow)
        s_speed = np.random.uniform(60, 180, n_snow)
        s_rad = np.random.uniform(2, 6, n_snow)
        s_phase = np.random.uniform(0, 2 * math.pi, n_snow)

        for f in range(total_frames):
            t = f / fps
            img = Image.new("RGB", (w, h), (0, 0, 0))
            draw = ImageDraw.Draw(img)
            for i in range(n_snow):
                cy = (sy[i] + s_speed[i] * t) % (h + 20) - 10
                cx = (sx[i] + 16 * math.sin(2 * math.pi * 0.5 * t + s_phase[i])) % w
                r = s_rad[i]
                val = int(180 + 75 * (r / 6))
                draw.ellipse([cx - r, cy - r, cx + r, cy + r], fill=(val, val, min(255, val + 15)))
            proc.stdin.write(img.tobytes())

    elif effect_key == "fire":
        np.random.seed(303)
        n_fire = 110
        fx = np.random.uniform(0, w, n_fire)
        fy = np.random.uniform(0, h, n_fire)
        f_speed = np.random.uniform(100, 280, n_fire)
        f_rad = np.random.uniform(2.5, 6.0, n_fire)
        f_phase = np.random.uniform(0, 2 * math.pi, n_fire)
        f_flicker = np.random.uniform(3, 8, n_fire)

        for f in range(total_frames):
            t = f / fps
            img = Image.new("RGB", (w, h), (0, 0, 0))
            draw = ImageDraw.Draw(img)
            for i in range(n_fire):
                cy = (fy[i] - f_speed[i] * t) % (h + 30) - 15
                cx = (fx[i] + 24 * math.sin(2 * math.pi * 0.8 * t + f_phase[i])) % w
                flicker = 0.6 + 0.4 * math.sin(2 * math.pi * f_flicker[i] * t)
                r = f_rad[i]
                rg = r * 2.2
                draw.ellipse([cx - rg, cy - rg, cx + rg, cy + rg], fill=(int(180 * flicker), int(45 * flicker), 0))
                draw.ellipse([cx - r, cy - r, cx + r, cy + r], fill=(int(255 * flicker), int(140 * flicker), int(10 * flicker)))
                rc = max(1.2, r * 0.5)
                draw.ellipse([cx - rc, cy - rc, cx + rc, cy + rc], fill=(255, int(230 * flicker), int(120 * flicker)))
            proc.stdin.write(img.tobytes())

    elif effect_key == "bokeh":
        np.random.seed(404)
        n_bokeh = 35
        bx = np.random.uniform(0, w, n_bokeh)
        by = np.random.uniform(0, h, n_bokeh)
        b_speed = np.random.uniform(30, 80, n_bokeh)
        b_rad = np.random.uniform(25, 65, n_bokeh)
        b_phase = np.random.uniform(0, 2 * math.pi, n_bokeh)

        for f in range(total_frames):
            t = f / fps
            img = Image.new("RGB", (w, h), (0, 0, 0))
            draw = ImageDraw.Draw(img)
            for i in range(n_bokeh):
                cy = (by[i] - b_speed[i] * t) % (h + 120) - 60
                cx = (bx[i] + 35 * math.sin(2 * math.pi * 0.25 * t + b_phase[i])) % w
                r = b_rad[i]
                pulse = 0.7 + 0.3 * math.sin(2 * math.pi * 0.5 * t + b_phase[i])
                steps = 5
                for s in range(steps, 0, -1):
                    cur_r = r * (s / steps)
                    alpha_factor = ((steps - s + 1) / steps) * pulse
                    c_r = int(255 * alpha_factor * 0.8)
                    c_g = int(200 * alpha_factor * 0.7)
                    c_b = int(100 * alpha_factor * 0.5)
                    draw.ellipse([cx - cur_r, cy - cur_r, cx + cur_r, cy + cur_r], fill=(c_r, c_g, c_b))
            proc.stdin.write(img.tobytes())

    elif effect_key == "light_leak":
        for f in range(total_frames):
            t = f / fps
            img = Image.new("RGB", (w, h), (0, 0, 0))
            draw = ImageDraw.Draw(img)

            # Golden sweep
            p1 = 0.5 + 0.5 * math.sin(2 * math.pi * 0.25 * t)
            r1 = 380 + 80 * math.sin(2 * math.pi * 0.25 * t + 1)
            cx1 = 120 + 80 * math.cos(2 * math.pi * 0.25 * t)
            cy1 = 80 + 60 * math.sin(2 * math.pi * 0.25 * t)
            for s in range(7, 0, -1):
                cur_r = r1 * (s / 7)
                af = ((8 - s) / 7) * p1 * 0.75
                draw.ellipse([cx1 - cur_r, cy1 - cur_r, cx1 + cur_r, cy1 + cur_r], fill=(int(255 * af), int(160 * af), int(30 * af)))

            # Warm magenta sweep
            p2 = 0.5 + 0.5 * math.cos(2 * math.pi * 0.25 * t)
            r2 = 420 + 90 * math.cos(2 * math.pi * 0.25 * t)
            cx2 = w - 150 + 90 * math.sin(2 * math.pi * 0.25 * t)
            cy2 = h - 100 + 70 * math.cos(2 * math.pi * 0.25 * t)
            for s in range(7, 0, -1):
                cur_r = r2 * (s / 7)
                af = ((8 - s) / 7) * p2 * 0.65
                draw.ellipse([cx2 - cur_r, cy2 - cur_r, cx2 + cur_r, cy2 + cur_r], fill=(int(240 * af), int(80 * af), int(120 * af)))

            proc.stdin.write(img.tobytes())

    proc.stdin.close()
    proc.wait()
    return overlay_path


def ensure_all_overlay_effects():
    """Generates all 5 particle overlay videos in background on startup."""
    for key, cfg in VISUAL_EFFECTS_CONFIG.items():
        if cfg.get("type") == "overlay":
            ensure_effect_asset(key)


def get_visual_effects_list():
    """Returns list of visual effect configurations for API and frontend UI."""
    return list(VISUAL_EFFECTS_CONFIG.values())
