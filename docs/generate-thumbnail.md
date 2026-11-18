# Thumbnail Generate

Bagian Thumbnail pada Generate menyediakan tiga kontrol:

- **Thumbnail** membuka editor Thumbnail dengan sumber video Generate.
- **Pilih Thumbnail** memilih gambar siap pakai dan beralih ke mode manual.
- **Auto Thumbnail** membuat JPG Full HD dari video dan playlist Generate.

Mode otomatis memakai frame pada waktu yang dipilih di editor Thumbnail, judul,
urutan lagu, font, warna, ukuran, tata letak, dan vignette yang sama. Desain dan
playlist diambil saat render dimulai, sehingga perubahan editor setelah itu
berlaku untuk render berikutnya. Gambar manual tidak digunakan dalam mode ini.

Hasil `thumbnail_<kodeunik>.jpg` disimpan di folder hasil Generate yang sama dengan video,
`playlist.txt`, dan `Timestamp.txt`. Pilihan Auto Thumbnail disimpan dalam settings
aplikasi. Menonaktifkannya kembali memakai gambar pilihan jika tersedia.

Kode unik thumbnail sama dengan video: `video_a1b2c3.mp4` dipasangkan dengan
`thumbnail_a1b2c3.jpg`. Gambar manual juga memakai kode video tersebut, dengan
ekstensi asli tetap dipertahankan, misalnya `thumbnail_a1b2c3.png`.

Jika pembuatan thumbnail gagal, video yang selesai tetap dipertahankan dan
aplikasi menampilkan peringatan. Berkas gambar asli tidak dihapus.

Validasi render nyata (memerlukan FFmpeg dan FFprobe pada PATH):

```powershell
cargo test --test generate_thumbnail
```
