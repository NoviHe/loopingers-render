# Changelog

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
