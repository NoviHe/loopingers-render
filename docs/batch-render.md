# Batch Render

1. Siapkan playlist melalui Generate atau Playlist.
2. Pilih folder visual dan atur jumlah serta durasi video di Batch Render.
3. Pilih folder thumbnail dengan nama dasar yang cocok dengan visual, atau
   aktifkan Auto Thumbnail untuk memakai desain editor Thumbnail.
4. Klik Buat Rencana, periksa urutan lagu, lalu Mulai Batch.

Semua kolom durasi boleh dikosongkan. Dalam mode ini, video mengikuti durasi
playlist sebenarnya termasuk crossfade Generate. Isi minimum dan maksimum
bersama-sama jika ingin durasi acak, atau isi daftar durasi manual.

Render berjalan berurutan memakai kualitas, encoder, resolusi, normalisasi,
crossfade, fade out, dan variasi visual dari Generate/pengaturan video.
Durasi dan urutan lagu mengikuti rencana yang diambil saat Mulai Batch.
Mengubah playlist setelah mulai tidak mengubah antrean yang sedang berjalan.

Folder output mengikuti pilihan folder Generate dan dapat dipilih dari Batch.
Setiap antrean memiliki folder induk `Batch_YYYY-MM-DD_001` (nomor bertambah
jika nama sudah digunakan), beserta `ringkasan_batch.txt` yang diperbarui setiap
video selesai. Ringkasan memuat status, durasi, lokasi, peringatan, dan error.
Setiap video memiliki subfolder bernomor seperti `01_abc123` berisi video, thumbnail,
playlist.txt, Timestamp.txt, dan laporan render. Kode thumbnail sama dengan
video, misalnya video_abc123.mp4 dan thumbnail_abc123.jpg. Thumbnail manual
mempertahankan ekstensi gambar aslinya. Thumbnail otomatis mengikuti urutan
lagu masing-masing video dan desain editor, termasuk pilihan judul lagu pertama.

Progres menampilkan tahap video aktif, jumlah yang sudah selesai, lokasi hasil,
peringatan, serta error. Satu video gagal tidak menghalangi video berikutnya.
Gunakan Buka Folder Batch untuk melihat seluruh antrean, atau Buka Video dan
Buka Folder pada masing-masing hasil.
Hentikan setelah video ini menyelesaikan video aktif sebelum menghentikan
antrean; hasil yang sudah selesai tetap disimpan. Saat masih menyiapkan antrean,
permintaan berhenti mencegah video pertama dimulai.
