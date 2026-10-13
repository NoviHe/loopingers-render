# 🎤 LoopKaraoke Studio

Aplikasi praktis dan mudah digunakan untuk:
1. **Looping Video Background**: Mengulang video 1 menit atau lebih secara otomatis agar pas dengan total durasi lagu.
2. **Kompilasi Banyak Lagu (Audio Playlist)**: Menggabungkan beberapa lagu favorit sekaligus dalam satu video.
3. **Auto Lirik Karaoke (AI Whisper)**: Deteksi lirik otomatis dan sinkronisasi efek karaoke kata-per-kata (*word-by-word wipe glow*).
4. **UI/UX Modern, Simpel & Mudah**: Antarmuka gelap yang bersih, responsif, dan interaktif dengan pemutar pratinjau langsung di browser.

---

---

## 🖥️ Versi Desktop (Standalone & GitHub Releases)

Aplikasi ini dapat dijalankan sebagai **Aplikasi Desktop Standalone** (jendela mandiri tanpa browser) dan dapat diunduh langsung lewat tab **GitHub Releases**:

### 📥 Unduh Langsung via GitHub Releases (Untuk Pengguna):
1. Buka halaman repository GitHub ini, lalu klik tab **Releases** di sisi kanan (atau buka link `https://github.com/<username>/<repo>/releases`).
2. Unduh file **`LoopKaraokeStudio-Windows-x64.zip`** pada versi terbaru.
3. Ekstrak file zip tersebut di komputer Anda.
4. Klik dua kali **`LoopKaraokeStudio.exe`** (atau `Buka_Aplikasi.bat`) untuk langsung menggunakannya tanpa perlu install Python!

---

### 🚀 Cara Menjalankan Versi Desktop di Komputer Ini:
* **Cara Cepat (1-Klik)**: Klik dua kali file **`LoopKaraokeStudio.bat`**.
* **Lewat Terminal**: Jalankan perintah:
  ```bash
  python desktop.py
  ```
Aplikasi akan langsung membuka jendela desktop mandiri (*native desktop window*) dengan akselerasi penuh GPU NVIDIA RTX 2060.

---

### 📦 Cara Build Paket Rilis Desktop Sendiri:
Untuk membuat file installer / zip rilis desktop secara lokal di komputer:
```bash
python build_desktop.py
```
Output paket siap bagi akan otomatis dibuat di folder `dist/LoopKaraokeStudio-Windows-x64.zip`.

---

### 🔄 Otomatisasi Rilis GitHub (GitHub Actions CI/CD):
Repository ini sudah dilengkapi workflow otomatis di [`.github/workflows/release.yml`](file:///.github/workflows/release.yml).
Setiap kali Anda membuat tag rilis baru di GitHub:
```bash
git tag v1.0.0
git push origin v1.0.0
```
Server GitHub Actions akan otomatis meng-compile aplikasi Windows Desktop menjadi file zip dan langsung mengunggahnya ke halaman **GitHub Releases** untuk diunduh oleh siapa saja!

---

## 🌟 Fitur Utama & Panduan Penggunaan

### 1. 🎬 Langkah 1: Video Background
- **Unggah Video**: Tarik & lepas video latar (MP4, MOV, WEBM) durasi 1 menit atau lebih.
- **Pilihan Rasio Layar**:
  - `16:9 Landscape` (YouTube, PC, TV)
  - `9:16 Portrait` (TikTok, YouTube Shorts, Instagram Reels)
  - `1:1 Persegi` (Instagram Feed)
- **Mode Tampilan (Fit)**:
  - *Crop Penuh (Fill)*: Memenuhi layar tanpa garis hitam tepi.
  - *Blur Estetik*: Latar samping dibuat blur artistik sementara video asli di tengah.
  - *Letterbox*: Tampilan utuh dengan garis hitam rapi.
- **Mute Video Asli**: Otomatis membisukan suara video agar hanya lagu yang terdengar jernih.

### 2. 🎵 Langkah 2: Daftar Lagu (Playlist)
- Tambahkan banyak lagu sekaligus (MP3, WAV, M4A, FLAC, dll).
- Urutkan lagu dengan tombol ⬆️ / ⬇️.
- Putar preview lagu secara langsung.
- **Kalkulasi Looping Otomatis**: Aplikasi langsung menghitung total durasi dan menampilkan berapa kali video 1 menit akan berputar (contoh: *Total 15 menit • Video meloop 15 kali*).

### 3. 🎤 Langkah 3: Auto Lirik & Karaoke Studio
- **✨ Auto Deteksi AI (Faster-Whisper)**:
  - AI mendengarkan lagu dan otomatis membuat lirik dengan timing kata-demi-kata (*word timestamps*).
  - Pilihan model: *Tiny (cepat)*, *Base (seimbang)*, *Small (akurasi tinggi)*.
  - Pilihan bahasa: Indonesia, Inggris, Jepang, Korea, atau Deteksi Otomatis.
- **📂 Impor File .LRC / .SRT / Tempel Teks**:
  - Jika sudah punya file lirik atau ingin menempel lirik manual, cukup klik tombol impor.
- **Pemutar Monitor Karaoke Interaktif**:
  - Dengarkan lagu sambil melihat kata yang disorot (*highlight glow*) secara real-time.
- **Kustomisasi Gaya Teks**:
  - Preset Tema: *Golden Glow (Kuning Emas)*, *Neon Cyan (Biru Neon)*, *Sunset Pink*, *Emerald Spark*, dll.
  - Penyesuaian ukuran font, warna teks normal, warna saat bernyanyi, dan garis tepi.
  - Posisi teks: Bawah (standar karaoke), Tengah, atau Atas.
  - Banner Judul Lagu & Penyanyi di awal setiap lagu.

### 4. ⚡ Langkah 4: Render & Ekspor Video
- Pilihan Resolusi: **1080p Full HD**, **720p HD**, atau **4K Ultra HD**.
- Pilihan FPS: 30 FPS atau 60 FPS.
- **Pantau Proses Real-Time**: Persentase render (%), kecepatan encode, dan estimasi waktu sisa (ETA).
- Pemutar video hasil render langsung di browser setelah selesai.
- Tombol **"Buka Folder di Komputer"** untuk langsung membuka hasil video di Windows Explorer.

### 🎲 Coba Cepat dengan Tombol "Coba Demo Sampel"
Tersedia tombol **"Coba Demo Sampel"** di pojok kanan atas aplikasi. Tombol ini otomatis membuat sampel video latar bergerak dan lagu sampel dengan lirik karaoke tersinkronisasi, sehingga Anda bisa langsung mencoba alur kerja aplikasi dalam 1 klik.

### 🧹 Manajemen Penyimpanan & Pembersihan Cache Otomatis
Tersedia tombol **"Kelola Cache & File"** di header navigasi:
- **Pantau Penggunaan Disk**: Lihat kapasitas penyimpanan folder `uploads/` (file sementara), `outputs/` (video jadi), dan Cache Model AI Whisper (`~/.cache/huggingface/`).
- **Pembersihan Sekali Klik**:
  - Hapus file upload sementara yang sudah tidak terpakai.
  - Hapus seluruh video hasil render lama untuk melegakan ruang harddisk.
  - Hapus file subtitle `.ass` dan daftar demuxer sementara.
- **Pembersihan Otomatis**: Berkas sementara (.ass dan demuxer) berumur lebih dari 24 jam otomatis dibersihkan secara berkala saat server dinyalakan.
- **Hapus Satuan**: Dapat menghapus video tertentu langsung dari daftar di dalam modal.

---

## 🛠️ Spesifikasi Teknologi

- **Backend**: Python 3.14 + FastAPI + Uvicorn
- **Media Engine**: FFmpeg 7.1 + PyAV (Hardware decoding & stream loop)
- **Subtitle Engine**: Advanced SubStation Alpha (.ass) dengan timing `\kf` karaoke halus
- **AI Speech-to-Text**: Faster-Whisper (CTranslate2 ONNX Engine)
- **Frontend**: HTML5, Vanilla JavaScript, CSS3 Glassmorphic Dark Design, Lucide Icons
