import os
import subprocess
import shutil
import imageio_ffmpeg
from typing import Dict, Any

def setup_cuda_dll_paths():
    """
    Ensures NVIDIA cuBLAS 12 and cuDNN DLL directories from Python site-packages
    are registered in Windows DLL search path so CTranslate2 can load cublas64_12.dll.
    """
    if os.name != 'nt':
        return
    import site
    try:
        for p in site.getsitepackages():
            nvidia_dir = os.path.join(p, 'nvidia')
            if os.path.exists(nvidia_dir):
                for sub in os.listdir(nvidia_dir):
                    bin_dir = os.path.join(nvidia_dir, sub, 'bin')
                    if os.path.exists(bin_dir):
                        try:
                            os.add_dll_directory(bin_dir)
                            if bin_dir not in os.environ.get("PATH", ""):
                                os.environ["PATH"] = bin_dir + os.pathsep + os.environ["PATH"]
                        except Exception:
                            pass
    except Exception:
        pass

# Call immediately on module load
setup_cuda_dll_paths()

_HARDWARE_INFO = None

def get_hardware_info() -> Dict[str, Any]:
    """
    Detects NVIDIA GPU, CUDA capability, and available FFmpeg hardware encoders.
    Caches result for fast access.
    """
    global _HARDWARE_INFO
    if _HARDWARE_INFO is not None:
        return _HARDWARE_INFO

    info = {
        "has_nvidia": False,
        "gpu_name": "CPU Only",
        "cuda_whisper": False,
        "nvenc_h264": False,
        "nvenc_hevc": False,
        "recommended_device": "cpu",
        "recommended_encoder": "cpu_x264"
    }

    # 1. Detect NVIDIA GPU via nvidia-smi
    if shutil.which("nvidia-smi"):
        try:
            res = subprocess.run(
                ["nvidia-smi", "--query-gpu=name", "--format=csv,noheader"],
                capture_output=True,
                text=True,
                timeout=3
            )
            if res.returncode == 0 and res.stdout.strip():
                info["has_nvidia"] = True
                info["gpu_name"] = res.stdout.strip().splitlines()[0]
        except Exception:
            pass

    # 2. Check CUDA support in ctranslate2 (for Faster-Whisper)
    try:
        import ctranslate2
        if ctranslate2.get_cuda_device_count() > 0:
            info["cuda_whisper"] = True
    except Exception:
        pass

    # 3. Check FFmpeg NVENC encoders
    try:
        ffmpeg_exe = imageio_ffmpeg.get_ffmpeg_exe()
        res_ff = subprocess.run(
            [ffmpeg_exe, "-encoders"],
            capture_output=True,
            text=True,
            timeout=3
        )
        if "h264_nvenc" in res_ff.stdout and info["has_nvidia"]:
            info["nvenc_h264"] = True
        if "hevc_nvenc" in res_ff.stdout and info["has_nvidia"]:
            info["nvenc_hevc"] = True
    except Exception:
        pass

    # 4. Set defaults
    if info["nvenc_h264"] and info["has_nvidia"]:
        info["recommended_device"] = "cuda"
        info["recommended_encoder"] = "nvenc_h264"

    _HARDWARE_INFO = info
    return info
