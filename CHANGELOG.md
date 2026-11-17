# Changelog

## v2.1.2 — 2026-11-17

Pembaruan ini menyeragamkan tampilan kontrol aplikasi dan memperbaiki build
Windows dengan dukungan Vulkan untuk Whisper.

### Tampilan lebih konsisten

- Standar kontrol bersama: tinggi 32 px, sudut membulat 8 px, serta padding,
  border, dan warna placeholder yang seragam.
- Input Thumbnail dan Karaoke memakai komponen teks bersama; kontrol terkait
  pada Batch Render, Generate, dan Visualizer juga disesuaikan.
- Slider pengaturan Thumbnail, Karaoke, dan sejumlah pengaturan Visualizer
  memakai komponen yang sama dengan nilai angka yang dapat diedit.
- Input menyesuaikan ruang pada panel sempit agar tidak melebar keluar panel.

### Penyempurnaan Karaoke dan log

- Istilah “Pratinjau” pada menu Karaoke diganti menjadi “Preview”.
- Nama tema disederhanakan: Golden Glow, Neon Cyan, Sunset Pink,
  Emerald Spark, dan Classic Karaoke.
- Ikon tooltip di sebelah kiri pilihan tema dihapus.
- Informasi GPU tetap rata kanan saat panel log dibuka maupun diciutkan.

### Perbaikan build Windows

- Vulkan SDK dipasang dan environment disiapkan otomatis sebelum kompilasi.
- Installer SDK diverifikasi dengan SHA-256 sebelum dijalankan.
- Direktori build diperpendek untuk menghindari batas panjang path MSBuild
  saat mengompilasi Whisper/Vulkan.
- Pengaturan debug egui dibatasi pada mode debug agar build release berhasil.
- Checkout diperbarui ke v6; workflow mendukung pengulangan rilis berdasarkan
  tag dan penyimpanan cache Cargo saat build gagal.

### Validasi

- 106 unit test lulus, termasuk pengujian ukuran kontrol dan panel sempit.
- Clippy dengan `-D warnings`, pemeriksaan format, dan build release Windows lulus.
- Checksum paket ZIP hasil rilis telah diverifikasi.

### Unduh dan perbarui

1. Unduh **LoopKaraokeStudio-Windows-x64.zip** dari aset rilis.
2. Ekstrak seluruh isi ZIP ke folder aplikasi.
3. Tutup versi lama, lalu jalankan **LoopKaraokeStudio.exe** dari folder baru.

Paket Windows x64 menyertakan FFmpeg, FFprobe, dan aset efek. Versi 2.1.2
mencakup perubahan dari tag 2.1.1 yang build rilisnya belum berhasil.

[Unduh v2.1.2](https://github.com/NoviHe/loopingers-render/releases/tag/v2.1.2)
· [Seluruh perubahan sejak v2.1.0](https://github.com/NoviHe/loopingers-render/compare/v2.1.0...v2.1.2)

## v2.1.0 — 2026-11-17

### Fitur dan tampilan
- Dashboard baru, navigasi sidebar lebih rapi, dan Render Old dipisahkan dari alur utama.
- Batch Render dengan pembuatan rencana, daftar video, serta input jumlah dan durasi yang diperbarui.
- Playlist berfokus pada lagu dari Generate, daftar album, timestamps YouTube, dan deteksi LRC.
- Studio Lirik dengan impor audio/LRC, Whisper AI, editor dan sinkronisasi waktu.
- Editor Thumbnail mendukung unggah gambar manual, frame video Generate, frame acak, font, dan pengaturan teks serta daftar lagu.
- Pengaturan penyimpanan dan halaman Tentang diperbarui.

### Perbaikan
- Toolbar editor lirik menjadi dua baris saat jendela dimaksimalkan dan satu baris saat jendela normal.
- Tampilan baris lirik konsisten pada kedua ukuran; editor saat dimaksimalkan mengikuti batas bawah kolom kiri, sedangkan jendela normal dibatasi lima baris.
- Perbaikan font thumbnail yang belum terdaftar untuk mencegah panic.
- File .gitkeep disembunyikan dari daftar hasil dan dilindungi saat pembersihan.
- Perapian input, slider, padding, ikon, tooltip, dan posisi informasi GPU.
- Generate dan Batch Render menggunakan alur media sendiri.

### Pengembangan
- Panduan native Codex subagents diperbarui.
