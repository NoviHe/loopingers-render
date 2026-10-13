import os
import sys
import shutil
import zipfile
import subprocess


def build():
    print("=" * 65)
    print("  LoopKaraoke Studio - Build Desktop Executable & Release Package")
    print("=" * 65)

    base_dir = os.path.abspath(os.path.dirname(__file__))
    os.chdir(base_dir)

    # 1. Clean previous builds
    for d in ["build", "dist"]:
        if os.path.exists(d):
            print(f"Membersihkan folder lama {d}...")
            shutil.rmtree(d, ignore_errors=True)

    # 2. Run PyInstaller
    print("\n[1/3] Menjalankan PyInstaller...")
    cmd = [sys.executable, "-m", "PyInstaller", "--noconfirm", "LoopKaraokeStudio.spec"]
    ret = subprocess.call(cmd)
    if ret != 0:
        print("[ERROR] PyInstaller build gagal!")
        sys.exit(ret)

    dist_app_dir = os.path.join(base_dir, "dist", "LoopKaraokeStudio")
    if not os.path.exists(dist_app_dir):
        print(f"[ERROR] Direktori output tidak ditemukan: {dist_app_dir}")
        sys.exit(1)

    # 3. Create required runtime directories in dist folder
    print("\n[2/3] Menyiapkan struktur folder runtime (uploads & outputs)...")
    os.makedirs(os.path.join(dist_app_dir, "uploads"), exist_ok=True)
    os.makedirs(os.path.join(dist_app_dir, "outputs"), exist_ok=True)

    # Copy 1-click batch launcher to dist folder as well
    launcher_src = os.path.join(base_dir, "LoopKaraokeStudio.bat")
    if os.path.exists(launcher_src):
        shutil.copy(launcher_src, os.path.join(dist_app_dir, "Buka_Aplikasi.bat"))

    # 4. Create ZIP package for GitHub Releases
    print("\n[3/3] Mengompres paket rilis ke format ZIP...")
    zip_filename = os.path.join(base_dir, "dist", "LoopKaraokeStudio-Windows-x64.zip")
    with zipfile.ZipFile(zip_filename, "w", zipfile.ZIP_DEFLATED) as zipf:
        for root, dirs, files in os.walk(dist_app_dir):
            for file in files:
                abs_path = os.path.join(root, file)
                rel_path = os.path.relpath(abs_path, os.path.join(base_dir, "dist"))
                zipf.write(abs_path, rel_path)

    zip_size_mb = os.path.getsize(zip_filename) / (1024 * 1024)
    print("\n" + "=" * 65)
    print(" SUKSES! PAKET RILIS DESKTOP BERHASIL DIBUAT!")
    print("=" * 65)
    print(f"Lokasi File: {zip_filename}")
    print(f"Ukuran File: {zip_size_mb:.2f} MB")
    print("\nFile zip ini siap di-upload ke GitHub Releases atau didistribusikan langsung!")


if __name__ == "__main__":
    build()
