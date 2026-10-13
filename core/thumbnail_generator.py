import os
import subprocess
import math
from typing import List, Optional, Tuple
from PIL import Image, ImageDraw, ImageFont, ImageFilter
from core.video_looper import FFMPEG_EXE
from core.media_info import clean_proper_title

# Font paths
FONT_IMPACT = "C:\\Windows\\Fonts\\impact.ttf"
FONT_BOLD = "C:\\Windows\\Fonts\\arialbd.ttf"
FONT_REGULAR = "C:\\Windows\\Fonts\\arial.ttf"
FONT_SCRIPT = "C:\\Windows\\Fonts\\segoesc.ttf"


def get_font(path: str, size: int) -> ImageFont.FreeTypeFont:
    """Helper to load a font with fallback."""
    try:
        if os.path.exists(path):
            return ImageFont.truetype(path, size)
    except Exception:
        pass
    try:
        return ImageFont.truetype("arial.ttf", size)
    except Exception:
        return ImageFont.load_default()


def extract_video_frame(
    video_path: str,
    timestamp_sec: float,
    output_image_path: str,
    width: int = 1920,
    height: int = 1080
) -> str:
    """
    Extracts a single frame from video at the specified timestamp in seconds.
    Scales and crops to width x height (16:9 1080p).
    """
    if not os.path.isabs(video_path):
        video_path = os.path.abspath(video_path)
    if not os.path.isabs(output_image_path):
        output_image_path = os.path.abspath(output_image_path)

    os.makedirs(os.path.dirname(output_image_path), exist_ok=True)

    cmd = [
        FFMPEG_EXE,
        "-y",
        "-ss", str(max(0.0, float(timestamp_sec))),
        "-i", video_path,
        "-vframes", "1",
        "-vf", f"scale={width}:{height}:force_original_aspect_ratio=increase,crop={width}:{height}",
        "-q:v", "2",
        output_image_path
    ]

    res = subprocess.run(cmd, capture_output=True, text=True, cwd="c:\\loopingapp")
    if res.returncode != 0 or not os.path.exists(output_image_path):
        # Fallback without scale/crop if input filter failed
        cmd_fallback = [
            FFMPEG_EXE,
            "-y",
            "-ss", str(max(0.0, float(timestamp_sec))),
            "-i", video_path,
            "-vframes", "1",
            "-q:v", "2",
            output_image_path
        ]
        res2 = subprocess.run(cmd_fallback, capture_output=True, text=True, cwd="c:\\loopingapp")
        if res2.returncode != 0 or not os.path.exists(output_image_path):
            raise RuntimeError(f"Gagal mengambil frame video di detik {timestamp_sec}: {res.stderr[-300:]}")

    return output_image_path


def draw_text_with_outline(
    draw: ImageDraw.ImageDraw,
    pos: Tuple[int, int],
    text: str,
    font: ImageFont.FreeTypeFont,
    fill_color: str = "#FFFFFF",
    outline_color: str = "#000000",
    outline_width: int = 3,
    align: str = "left"
):
    """
    Draws high-contrast text with a dark outline and drop-shadow.
    """
    x, y = pos
    
    # Calculate bounding box for alignment
    bbox = font.getbbox(text)
    w = bbox[2] - bbox[0]
    h = bbox[3] - bbox[1]

    if align == "center":
        x = x - w // 2
    elif align == "right":
        x = x - w

    # Drop shadow
    shadow_offset = max(2, outline_width)
    for sx in range(-outline_width, outline_width + 1):
        for sy in range(-outline_width, outline_width + 1):
            if sx != 0 or sy != 0:
                draw.text((x + sx, y + sy), text, font=font, fill=outline_color)
    
    # Extra bottom-right shadow
    draw.text((x + shadow_offset, y + shadow_offset + 1), text, font=font, fill="#000000")

    # Foreground text
    draw.text((x, y), text, font=font, fill=fill_color)


def create_vignette_mask(width: int, height: int, opacity: float = 0.5) -> Image.Image:
    """
    Creates a smooth dark vignette overlay focused on edges to enhance text readability.
    """
    overlay = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    draw = ImageDraw.Draw(overlay)

    # Gradient from edges
    max_alpha = int(255 * min(1.0, max(0.0, opacity)))
    
    # Left edge gradient (for left song list)
    edge_w = int(width * 0.42)
    for x in range(edge_w):
        factor = (1.0 - (x / edge_w)) ** 1.8
        alpha = int(max_alpha * factor * 0.95)
        draw.line([(x, 0), (x, height)], fill=(0, 0, 0, alpha))

    # Right edge gradient (for right song list)
    for x in range(width - edge_w, width):
        factor = ((x - (width - edge_w)) / edge_w) ** 1.8
        alpha = int(max_alpha * factor * 0.95)
        draw.line([(x, 0), (x, height)], fill=(0, 0, 0, alpha))

    # Top edge gradient (for main title)
    top_h = int(height * 0.28)
    for y in range(top_h):
        factor = (1.0 - (y / top_h)) ** 1.8
        alpha = int(max_alpha * factor * 0.9)
        draw.line([(0, y), (width, y)], fill=(0, 0, 0, alpha))

    # Bottom edge gradient (for signature / artist)
    bot_h = int(height * 0.25)
    for y in range(height - bot_h, height):
        factor = ((y - (height - bot_h)) / bot_h) ** 1.8
        alpha = int(max_alpha * factor * 0.85)
        draw.line([(0, y), (width, y)], fill=(0, 0, 0, alpha))

    return overlay


def build_thumbnail_image(
    background_source: str,  # Path to image or video
    output_path: str,
    title: str = "Powerful Worship",
    subtitle: str = "Hillsong Worship",
    track_titles: Optional[List[str]] = None,
    timestamp_sec: float = 1.0,
    width: int = 1920,
    height: int = 1080,
    column_mode: str = "auto",  # auto, 1, 2
    number_style: str = "mirrored",  # mirrored ("Title .01"), sequential ("11. Title")
    title_font_size: int = 110,
    track_font_size: int = 34,
    vignette_opacity: float = 0.55,
    title_color: str = "#FFFFFF",
    track_color: str = "#FFFFFF",
    subtitle_color: str = "#FFD700"
) -> str:
    """
    Renders a stunning YouTube-style playlist thumbnail based on a video frame or image.
    Matches the layout of worship/music playlist compilations:
    - Bold Main Title at Top Center
    - Tracklist on Left Side (if few songs <= 10)
    - Sequential Tracklist on Left & Right Sides (if many songs > 10)
    - Stylish Script Subtitle / Artist signature at Bottom Center
    """
    if track_titles is None:
        track_titles = []

    # 1. Obtain base background image
    clean_bg_path = background_source
    temp_frame_path = None

    # Check if background is video
    ext = os.path.splitext(background_source)[1].lower()
    if ext in [".mp4", ".mov", ".mkv", ".webm", ".avi"]:
        temp_frame_path = os.path.abspath(f"uploads/temp_thumb_frame_{os.path.basename(background_source)}.jpg")
        extract_video_frame(background_source, timestamp_sec, temp_frame_path, width, height)
        clean_bg_path = temp_frame_path

    if not os.path.exists(clean_bg_path):
        # Create solid gradient background if no file
        base_img = Image.new("RGB", (width, height), (20, 24, 33))
    else:
        base_img = Image.open(clean_bg_path).convert("RGB")
        if base_img.size != (width, height):
            # Scale & crop to 16:9
            orig_w, orig_h = base_img.size
            scale = max(width / orig_w, height / orig_h)
            new_w = int(orig_w * scale)
            new_h = int(orig_h * scale)
            base_img = base_img.resize((new_w, new_h), Image.Resampling.LANCZOS)
            left = (new_w - width) // 2
            top = (new_h - height) // 2
            base_img = base_img.crop((left, top, left + width, top + height))

    # 2. Apply dark vignette overlay for maximum text contrast
    if vignette_opacity > 0:
        vignette = create_vignette_mask(width, height, vignette_opacity)
        base_img.paste(Image.new("RGB", (width, height), (0, 0, 0)), (0, 0), vignette)

    draw = ImageDraw.Draw(base_img)

    # 3. Load Fonts
    font_title = get_font(FONT_IMPACT, title_font_size)
    font_track = get_font(FONT_BOLD, track_font_size)
    font_sub = get_font(FONT_SCRIPT, int(title_font_size * 0.72))

    # 4. Draw Main Title (Top Center)
    if title.strip():
        title_y = int(height * 0.055)
        draw_text_with_outline(
            draw=draw,
            pos=(width // 2, title_y),
            text=title.strip(),
            font=font_title,
            fill_color=title_color,
            outline_color="#000000",
            outline_width=5,
            align="center"
        )

    # 5. Format & Layout Tracklist
    # Determine column mode
    num_tracks = len(track_titles)
    is_two_columns = False
    if column_mode == "2":
        is_two_columns = True
    elif column_mode == "1":
        is_two_columns = False
    else:  # "auto"
        is_two_columns = (num_tracks > 10)

    # Starting coordinates
    start_y = int(height * 0.24)
    max_rows = 11
    line_spacing = int(track_font_size * 1.55)

    if not is_two_columns:
        # SINGLE COLUMN (LEFT SIDE ONLY)
        left_x = int(width * 0.055)
        for i, t in enumerate(track_titles[:15]):
            num_str = f"{i+1:02d}."
            clean_title = clean_proper_title(t)
            # Truncate if too long
            if len(clean_title) > 30:
                clean_title = clean_title[:28] + "..."
            
            line_text = f"{num_str}  {clean_title}"
            cur_y = start_y + (i * line_spacing)
            if cur_y > height - 120:
                break
            
            draw_text_with_outline(
                draw=draw,
                pos=(left_x, cur_y),
                text=line_text,
                font=font_track,
                fill_color=track_color,
                outline_color="#000000",
                outline_width=3,
                align="left"
            )
    else:
        # TWO COLUMNS (LEFT & RIGHT)
        # Split songs into 2 halves
        half = math.ceil(num_tracks / 2.0)
        left_songs = track_titles[:half]
        right_songs = track_titles[half:]

        # Limit to max rows to keep within screen bounds
        left_songs = left_songs[:max_rows]
        right_songs = right_songs[:max_rows]

        # Draw Left Column (01. Title)
        left_x = int(width * 0.055)
        for i, t in enumerate(left_songs):
            num_str = f"{i+1:02d}."
            clean_title = clean_proper_title(t)
            if len(clean_title) > 24:
                clean_title = clean_title[:22] + "..."
            
            line_text = f"{num_str}  {clean_title}"
            cur_y = start_y + (i * line_spacing)
            draw_text_with_outline(
                draw=draw,
                pos=(left_x, cur_y),
                text=line_text,
                font=font_track,
                fill_color=track_color,
                outline_color="#000000",
                outline_width=3,
                align="left"
            )

        # Draw Right Column (Right side, aligned)
        right_x = int(width * 0.945)
        for j, t in enumerate(right_songs):
            song_idx = half + j + 1
            clean_title = clean_proper_title(t)
            if len(clean_title) > 24:
                clean_title = clean_title[:22] + "..."

            if number_style == "mirrored":
                # Sample image style: "King Of Kings .01" or "Title .11"
                line_text = f"{clean_title}  .{j+1:02d}"
            else:
                # Sequential style: "11. King Of Kings"
                line_text = f"{song_idx:02d}.  {clean_title}"

            cur_y = start_y + (j * line_spacing)
            draw_text_with_outline(
                draw=draw,
                pos=(right_x, cur_y),
                text=line_text,
                font=font_track,
                fill_color=track_color,
                outline_color="#000000",
                outline_width=3,
                align="right"
            )

    # 6. Draw Subtitle / Script Signature (Bottom Center)
    if subtitle.strip():
        sub_y = int(height * 0.82)
        draw_text_with_outline(
            draw=draw,
            pos=(width // 2, sub_y),
            text=subtitle.strip(),
            font=font_sub,
            fill_color=subtitle_color,
            outline_color="#000000",
            outline_width=3,
            align="center"
        )

    # 7. Save output thumbnail
    os.makedirs(os.path.dirname(output_path), exist_ok=True)
    base_img.save(output_path, "JPEG", quality=95)

    # Cleanup temp extracted frame if created
    if temp_frame_path and os.path.exists(temp_frame_path):
        try:
            os.remove(temp_frame_path)
        except Exception:
            pass

    return output_path
