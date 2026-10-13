import os
import time
import shutil
from typing import Dict, Any, List, Optional

UPLOADS_DIR = "uploads"
OUTPUTS_DIR = "outputs"


def get_dir_size_and_count(dir_path: str) -> Dict[str, Any]:
    """Calculates total size in bytes and number of files in a directory."""
    if not os.path.exists(dir_path):
        return {"size_bytes": 0, "size_mb": 0.0, "count": 0, "files": []}

    total_bytes = 0
    file_list = []

    for entry in os.scandir(dir_path):
        if entry.is_file():
            stat = entry.stat()
            size = stat.st_size
            total_bytes += size
            file_list.append({
                "name": entry.name,
                "path": entry.path,
                "size_bytes": size,
                "size_mb": round(size / (1024 * 1024), 2),
                "modified": stat.st_mtime,
                "modified_str": time.strftime("%Y-%m-%d %H:%M:%S", time.localtime(stat.st_mtime))
            })

    # Sort newest first
    file_list.sort(key=lambda x: x["modified"], reverse=True)

    return {
        "size_bytes": total_bytes,
        "size_mb": round(total_bytes / (1024 * 1024), 2),
        "count": len(file_list),
        "files": file_list
    }


def get_hf_cache_size() -> Dict[str, Any]:
    """Estimates size of Hugging Face Whisper model cache on user's system."""
    home_dir = os.path.expanduser("~")
    hf_cache_dir = os.path.join(home_dir, ".cache", "huggingface", "hub")

    if not os.path.exists(hf_cache_dir):
        return {"size_mb": 0.0, "path": hf_cache_dir, "models": []}

    total_bytes = 0
    models = []

    for root, dirs, files in os.walk(hf_cache_dir):
        for f in files:
            fp = os.path.join(root, f)
            try:
                total_bytes += os.path.getsize(fp)
            except OSError:
                pass

    for item in os.listdir(hf_cache_dir):
        if "faster-whisper" in item or "whisper" in item:
            models.append(item)

    return {
        "size_mb": round(total_bytes / (1024 * 1024), 2),
        "path": hf_cache_dir,
        "models": models
    }


def get_full_storage_stats() -> Dict[str, Any]:
    """Returns comprehensive storage metrics for uploads, outputs, and cache."""
    uploads_info = get_dir_size_and_count(UPLOADS_DIR)
    outputs_info = get_dir_size_and_count(OUTPUTS_DIR)
    hf_info = get_hf_cache_size()

    total_app_bytes = uploads_info["size_bytes"] + outputs_info["size_bytes"]

    return {
        "uploads": uploads_info,
        "outputs": outputs_info,
        "hf_cache": hf_info,
        "total_app_mb": round(total_app_bytes / (1024 * 1024), 2)
    }


def cleanup_storage(
    clean_uploads: bool = False,
    clean_outputs: bool = False,
    clean_temp_only: bool = True,
    max_age_hours: Optional[float] = None
) -> Dict[str, int]:
    """
    Cleans up unused files according to specified criteria.
    - clean_temp_only: deletes only .ass and .txt concat files.
    - clean_uploads: deletes uploaded videos/audios.
    - clean_outputs: deletes rendered videos.
    - max_age_hours: if specified, only deletes files older than this age.
    """
    now = time.time()
    deleted_counts = {"uploads": 0, "outputs": 0, "temp": 0}

    # 1. Clean uploads directory
    if os.path.exists(UPLOADS_DIR):
        for entry in os.scandir(UPLOADS_DIR):
            if entry.is_file():
                is_temp = entry.name.endswith(".ass") or entry.name.startswith("concat_")
                
                # Check age if max_age_hours specified
                if max_age_hours is not None:
                    age_seconds = now - entry.stat().st_mtime
                    if age_seconds < (max_age_hours * 3600):
                        continue

                should_delete = False
                if is_temp:
                    should_delete = True
                    deleted_counts["temp"] += 1
                elif clean_uploads:
                    should_delete = True
                    deleted_counts["uploads"] += 1

                if should_delete:
                    try:
                        os.remove(entry.path)
                    except Exception:
                        pass

    # 2. Clean outputs directory
    if clean_outputs and os.path.exists(OUTPUTS_DIR):
        for entry in os.scandir(OUTPUTS_DIR):
            if entry.is_file():
                if max_age_hours is not None:
                    age_seconds = now - entry.stat().st_mtime
                    if age_seconds < (max_age_hours * 3600):
                        continue
                try:
                    os.remove(entry.path)
                    deleted_counts["outputs"] += 1
                except Exception:
                    pass

    return deleted_counts


def delete_specific_file(folder: str, filename: str) -> bool:
    """Deletes a specific file inside 'uploads' or 'outputs' safely."""
    if folder not in ["uploads", "outputs"]:
        return False

    clean_filename = os.path.basename(filename)
    target_path = os.path.join(folder, clean_filename)

    if os.path.exists(target_path) and os.path.isfile(target_path):
        try:
            os.remove(target_path)
            return True
        except Exception:
            return False
    return False
