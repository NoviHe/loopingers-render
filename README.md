# LoopKaraoke Studio

Aplikasi desktop native (`.exe`, ditulis dengan Rust dan egui — tanpa browser, tanpa HTML) untuk:

1. **Looping video background**: video pendek diulang otomatis sepanjang total durasi lagu. Gambar juga bisa dipakai (diubah menjadi video bergerak dengan zoom/pan dan denyut mengikuti ketukan).
2. **Kompilasi banyak lagu**: beberapa lagu digabung berurutan menjadi satu video, lengkap dengan timestamp YouTube.
3. **Auto lirik karaoke (Whisper AI)**: deteksi lirik per kata dengan timing, atau impor file `.lrc`. Efek *word-by-word wipe* dibakar ke video lewat subtitle ASS.
4. **49 efek visual** (ditambah opsi tanpa efek): 37 overlay animasi dan 12 filter warna/gaya. Overlay antara lain hujan, salju, bara api, bokeh, light leak, kilau bintang, kunang-kunang, konfeti, kembang api, aurora, petir, kelopak sakura, nebula, matrix, dan synthwave. Filter antara lain teal & orange, sepia, hitam putih, dreamy, bloom, sinema, VHS, dan glitch. Daftar lengkap ada di dropdown **Efek visual**.
5. **Thumbnail YouTube** dari frame video dan daftar lagu.
6. **Manajemen penyimpanan**: lihat ukuran, hapus berkas sementara dan video lama.

## Dukungan GPU

| Fungsi | NVIDIA | AMD | Lainnya |
|--------|--------|-----|---------|
| Render video | NVENC (H.264 / HEVC) | AMF (H.264 / HEVC) | CPU x264 |
| Whisper AI | build `--features cuda` | build `--features vulkan` | CPU |

Encoder GPU terdeteksi otomatis: aplikasi mencoba encode satu frame per encoder, jadi hanya yang benar-benar berfungsi di PC Anda yang bisa dipilih. Tanpa GPU aplikasi memakai CPU.

## Pengguna: menjalankan rilis

1. Unduh `LoopKaraokeStudio-Windows-x64.zip` dari tab **Releases** GitHub, ekstrak.
2. Jalankan `LoopKaraokeStudio.exe`. Jika FFmpeg belum ada (di folder aplikasi atau `PATH`), aplikasi mengunduhnya otomatis sekali saja (±80 MB) ke folder `tools/`.

Model Whisper diunduh otomatis sekali saat pertama dipakai (tiny ≈ 75 MB, base ≈ 142 MB, small ≈ 466 MB) ke folder `models/`.

## Developer

### Prasyarat

- Rust stable (≥ 1.85) dan Cargo
- Windows: Visual Studio Build Tools (C++), CMake
- LLVM/libclang untuk binding Whisper; set `LIBCLANG_PATH` ke folder yang berisi `libclang.dll` bila tidak terdeteksi
- FFmpeg dan FFprobe: opsional untuk dev, karena aplikasi mengunduh sendiri bila tidak ditemukan (urutan pencarian: folder executable, `tools/`, `PATH`)
- Opsional: CUDA Toolkit (`--features cuda`) atau Vulkan SDK (`--features vulkan`) untuk Whisper di GPU

### Perintah

```bash
cargo run                          # jalankan (debug)
cargo run --release                # jalankan (optimasi)
cargo build --release --features vulkan   # rilis dengan Whisper GPU (AMD/NVIDIA via Vulkan)
cargo build --release --features cuda     # rilis dengan Whisper GPU NVIDIA via CUDA

cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test                         # unit test + uji end-to-end render (butuh FFmpeg)
cargo test --test install -- --ignored   # uji unduh otomatis FFmpeg (internet)
build.bat                          # build release + paket zip di dist/

# Uji transkripsi (mengunduh model tiny)
WHISPER_TEST_AUDIO=path/ke/ucapan.wav cargo test --test whisper -- --ignored --nocapture
```

Rilis otomatis: push tag `v*` memicu `.github/workflows/release.yml` yang membangun zip Windows dan mengunggahnya ke GitHub Releases.

### Konfigurasi

| Variabel | Fungsi |
|----------|--------|
| `LOOPKARAOKE_HOME` | Folder data (`temp/`, `outputs/`, `models/`, `tools/`). Default: folder executable (release) atau folder kerja (debug) |

Folder `assets/effects/` (video overlay) dicari di samping executable, lalu di folder proyek. Video overlay dibuat ulang dengan `cargo run --release --example gen_effects [nama...]` (loop mulus 8 detik, 1920x1080, butuh FFmpeg).

### Struktur

```
src/
  main.rs            titik masuk, jendela eframe
  app/               antarmuka egui, satu berkas per tab
  config.rs          folder data dan aset
  ffmpeg.rs          lokasi dan eksekusi ffmpeg/ffprobe
  hardware.rs        deteksi GPU dan encoder (NVENC, AMF)
  media/             probe file (ffprobe) dan pembersih judul
  karaoke/           parser LRC, ASS, Whisper
  render/            filter graph, encoder, efek, job, pipeline render
  thumbnail.rs       penggambar thumbnail
  storage.rs         statistik dan pembersihan
  demo.rs            data contoh
tests/               uji end-to-end
examples/gen_effects generator video overlay
assets/effects/      video overlay efek visual
assets/fonts/        Inter dan JetBrains Mono (lisensi OFL)
build.bat            build release dan paket zip
```

### Perbedaan dari versi Python/web

- Antarmuka native; pemutar audio/video dan monitor karaoke di dalam aplikasi diganti tombol **Buka di pemutar** (pemutar bawaan Windows).
- Whisper memakai `whisper.cpp` (model GGML) menggantikan `faster-whisper`; filter VAD belum ada.
- Efek overlay memakai video di `assets/effects/`, dirender ulang dengan generator Rust (`examples/gen_effects/`) agar halus dan loop mulus; blend dilakukan di ruang RGB sehingga warna video tidak bergeser.
- Tidak ada server HTTP, jadi tidak ada lagi unggah berkas: file dipilih langsung dari disk.
