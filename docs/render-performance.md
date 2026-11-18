# Pengukuran dan optimasi render

Settings → Kecepatan render menyediakan **Seimbang** dan **Lebih Cepat**.
Seimbang mempertahankan perilaku sebelumnya dan menjadi default untuk settings lama.
Lebih Cepat memilih NVENC p3, AMF speed, atau CPU veryfast. Codec, target kualitas
CQ/CRF, resolusi, FPS, audio, dan efek tidak diubah. Hasil gambar serta ukuran file
dapat berbeda karena usaha kompresi encoder berubah.

Pemeriksaan durasi audio berjalan maksimal empat proses bersamaan, dengan hasil
tetap mengikuti urutan playlist. File audio tetap digabung dan dinormalisasi
sesuai pengaturan pengguna.

## Hasil benchmark lokal

Diukur pada NVIDIA GeForce RTX 2060, driver 617.42, FFmpeg 9.0.2. Menggunakan alur
Generate aplikasi: video sintetis 720p berdurasi 2 detik diulang menjadi 30 detik,
output 1080p30 H.264 NVENC CQ 27, AAC 192k, dua lagu berdurasi 12 dan 18 detik,
normalisasi aktif, Geser Pelan, dan Anti-Duplicate aktif. Parameter kamera sama
untuk semua percobaan. Urutan percobaan dibalik pada putaran kedua untuk mengurangi
bias cache. Kompilasi tidak termasuk waktu pengukuran.

| Mode | Preset | Rata-rata total | Rata-rata tahap video | Ukuran video |
| --- | --- | --- | --- | --- |
| Seimbang | p5 | 5,802 detik | 4,082 detik | 27.611.357 byte |
| Lebih Cepat | p3 | 3,750 detik | 2,081 detik | 27.666.941 byte |

Pada sampel ini, total waktu berkurang **35,4%** (1,55× lebih cepat), sedangkan ukuran
file meningkat sekitar 0,2%. SSIM antara kedua hasil adalah **0,996987**; ini
menunjukkan kemiripan tinggi, bukan gambar identik atau jaminan kualitas untuk
semua materi. Pengukuran sintetis pendek ini tidak menjamin peningkatan yang sama
untuk album panjang, HEVC, CPU, AMF, atau efek yang lebih berat.

Keempat hasil diperiksa dengan FFprobe: 1920×1080, 30 FPS, H.264, audio tersedia,
durasi sekitar 30 detik, serta chapter lagu kedua tetap mulai pada detik 12.
Preset dan encoder aktual juga diperiksa melalui laporan hasil render.

## Mengukur materi sendiri

Generate menampilkan tahap aktif dan waktu berjalan. Setelah selesai, folder
hasil berisi `render_report.txt`: encoder/preset aktual, pengaturan dasar, waktu
persiapan, audio, video, finalisasi, total, dan kecepatan tahap video.
Log penyelesaian juga memuat ringkasan waktu.

Untuk mengulang benchmark sintetis yang sama:

```powershell
cargo run --example render_benchmark -- "C:\folder\berisi\ffmpeg-dan-ffprobe"
```

Contoh menyimpan media uji, CSV, ringkasan, dan SSIM dalam folder unik di temp sistem.
Data pengguna tidak digunakan. Laporan membedakan waktu kompilasi dari render.

Optimasi ini tidak mematikan Anti-Duplicate, mengurangi resolusi/FPS, atau memanggang
gerakan kamera menjadi potongan berulang yang dapat mengubah kontinuitas efek.
