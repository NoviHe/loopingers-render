# Efek Visualizer

Efek di `dist/audioVisualEditEffect` dipelajari melalui thumbnail, metadata,
parameter, dan shader. Implementasi aplikasi menggunakan geometri egui native
dan shader OpenGL khusus untuk Ring Glow;
tidak memerlukan folder Filmora tersebut saat aplikasi berjalan.

| Referensi | Pilihan aplikasi | Bentuk |
| --- | --- | --- |
| `34_Audio_Visualization_11` | Halo Spectrum | Batang frekuensi melingkar dan dicerminkan |
| `34_Audio_Visualization_16` | Halo Geometry | Cincin reaktif dengan gradasi magenta–ungu |
| `34_Music_Visualizer_Ring_Glow` | Ring Glow | Cincin emas bercahaya dengan respons bass |

Ini merupakan adaptasi visual native, bukan pemuatan plugin atau reproduksi
pixel identik dari engine Filmora. Bar Horizontal tetap tersedia.

Semua efek memakai analisis frekuensi audio lagu yang dipilih. Penghalusan
menggunakan waktu frame, dengan respons naik lebih cepat daripada turun.
Pergantian lagu dan seek mereset interpolasi agar tidak membawa respons lama.
Pengaturan jenis, warna, ukuran, posisi, sensitivitas, kehalusan, opasitas,
serta glow, ketebalan, dan rotasi cincin disimpan bersama konfigurasi aplikasi.

Efek ini berlaku pada Preview Visualizer. Integrasi ke render video belum
tersedia. Preview tidak mengambil judul, daftar lagu, atau vignette Thumbnail.

Untuk memeriksa bentuk renderer tanpa membuka jendela aplikasi:

```powershell
cargo run --example visualizer_style_preview
```

Contoh tersebut mengekspor PNG ke direktori sementara menggunakan data
frekuensi deterministik untuk inspeksi bentuk, bukan rekaman audio nyata.

## Ring Glow cair

Ring Glow menggunakan profil melingkar 128 titik dari audio, dengan normalisasi
adaptif yang dibatasi, penghalusan antarfrekuensi, dan pegas teredam. Tonjolan
berasal dari distribusi frekuensi lagu, bukan pola gerigi tetap. Kontrol
Kehalusan mengatur kelenturan respons; Sensitivitas mengatur besarnya respons.
Saat lagu diganti atau seek, profil dan kecepatan pegas direset. Saat pause,
gerakan mereda sampai stabil tanpa meminta repaint terus-menerus.

Shader menginterpolasi profil secara periodik dan menggambar inti cincin serta
pendar Gaussian per piksel. Latar preview tetap terlihat melalui bagian
transparan. Jika inisialisasi shader gagal, aplikasi memakai renderer geometri
cadangan dengan profil gerak yang sama dan menampilkan alasannya di pengaturan.
Resource GPU dilepas saat aplikasi ditutup.

## Tab pengaturan preview

`Setting Visualizer` memisahkan kontrol ke lima tab:

- **Visualizer:** jenis spektrum dan seluruh pengaturan overlay sebelumnya.
- **Teks:** judul lagu yang dipilih atau teks khusus, font, ukuran, warna,
  opasitas, dan posisi.
- **Background:** blur, vignette, dim, tint, serta zoom kecil mengikuti bass
  melalui Beat Pulse. Perubahan latar tidak memengaruhi teks, logo, atau spektrum.
- **Logo:** impor PNG/JPEG, visibilitas, ukuran, opasitas, posisi, dan beat pulse.
  PNG transparan mempertahankan transparansinya. Hapus Logo hanya melepaskan
  gambar dari preview, tidak menghapus berkas asli.
- **VFX:** belum berisi kontrol.

Pengaturan tiap lapisan disimpan secara terpisah di `visualizer_preview` dalam
settings aplikasi. Konfigurasi lama tetap dimuat dengan lapisan baru nonaktif.
Memilih preset spektrum tidak mereset pengaturan teks, background, atau logo.
Checkbox **Tampilkan** di kanan atas header mengatur visibilitas lapisan pada
tab Visualizer, Teks, Background, atau Logo yang sedang dibuka. Menyembunyikan
background menyisakan latar hitam tanpa mengubah teks, logo, atau spektrum.
Tab VFX tetap kosong. Efek ini tetap hanya untuk preview.

Untuk menguji shader di konteks GPU nyata dan mengekspor PNG:

```powershell
cargo run --example liquid_ring_gpu_preview
```
