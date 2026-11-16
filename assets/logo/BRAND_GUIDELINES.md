# LoopKaraoke Studio — Panduan Identitas Logo

## 1. Logo & Konsep
- **Nama merek**: LoopKaraoke Studio (`LOOPKARAOKE` pada wordmark lockup)
- **Konsep (Loop-L Frame)**: Monogram huruf **L** geometris yang dibentuk oleh pita *loop* berpanah 45° searah jarum jam, membingkai tombol **Play** (`▶`) di tengah sebagai simbol penggabungan banyak lagu menjadi satu video *looping*.
- **Varian utama**:
  - **Horizontal lockup**: [final-horizontal.svg](final-horizontal.svg) (untuk header, README, web) · versi gelap: [final-horizontal-on-dark.svg](final-horizontal-on-dark.svg)
  - **Stacked / vertikal**: [final-stacked.svg](final-stacked.svg) (untuk kotak, poster, kartu) · versi gelap: [final-stacked-on-dark.svg](final-stacked-on-dark.svg)
  - **Simbol berdiri sendiri**: [final-symbol.svg](final-symbol.svg) · ukuran kecil 16–24 px: [final-symbol-small.svg](final-symbol-small.svg) · versi gelap (tipis 5%): [final-symbol-on-dark.svg](final-symbol-on-dark.svg) · putih solid: [final-symbol-reversed.svg](final-symbol-reversed.svg)
  - **Wordmark saja**: [final-wordmark.svg](final-wordmark.svg)
  - **Ikon aplikasi & web**: [app-icon-color.svg](app-icon-color.svg), `favicon.ico`, `favicon.svg`, `icon-192.png`, `icon-512.png`, `maskable-512.png`, `apple-touch-icon.png`, dan set `loopkaraoke-symbol-*` / `loopkaraoke-horizontal-*`.

## 2. Ruang Bebas (Clear Space)
Sisakan ruang kosong minimal sebesar **`1 × W`** di keempat sisi logo, dengan `W` = lebar batang vertikal huruf **L** pada simbol (38 unit pada kanvas 256×256, atau sekitar 15% tinggi simbol). Jangan menempatkan teks, garis batas, atau elemen UI lain di dalam zona ini.

## 3. Ukuran Minimum
| Varian | Layar (Screen) | Cetak (Print) | Berkas yang dipakai |
|---|---|---|---|
| Lockup horizontal | 96 px lebar | 25 mm lebar | `final-horizontal.svg` / `loopkaraoke-horizontal-*-1200.png` |
| Lockup bertumpuk | 72 px lebar | 20 mm lebar | `final-stacked.svg` |
| Simbol saja (≥ 32 px) | 32 px | 8 mm | `final-symbol.svg` |
| Simbol ukuran kecil | 16 px | 6 mm | `final-symbol-small.svg` / `favicon.ico` |

## 4. Palet Warna
Palet diturunkan langsung dari token antarmuka gelap aplikasi ([theme.rs](../../src/app/theme.rs)):

| Nama | HEX | RGB | CMYK | Pantone (perkiraan) | Pemakaian |
|---|---|---|---|---|---|
| **Studio Blue** | `#2F6BDF` | `47 107 223` | `79 52 0 13` | `2728 C` | Segitiga Play (latar terang / tile), aksen utama |
| **Focus Sky** | `#5794EC` | `87 148 236` | `63 37 0 7` | `279 C` | Segitiga Play pada latar gelap (`*-on-dark.svg`) |
| **Studio Ink** | `#141820` | `20 24 32` | `38 25 0 87` | `Black 6 C` | Bingkai L & wordmark pada latar terang, tile ikon aplikasi |
| **White** | `#FFFFFF` | `255 255 255` | `0 0 0 0` | `—` | Versi reversed dan latar gelap (`#1D1D1D` / `#000000`) |

**Pasangan latar yang disetujui**:
- Latar terang (`#FFFFFF` / `#F7F7F4`): gunakan `final-symbol.svg` / `final-horizontal.svg` / `final-stacked.svg`, atau versi satu warna hitam (`*-black.svg` / `*-mono-141820.svg`).
- Latar gelap (`#141820` / `#1D1D1D` / `#000000`): gunakan `*-on-dark.svg` (batang sudah ditipiskan ~5% untuk mencegah efek mekar/iradiasi) atau `final-symbol-reversed.svg` / `*-white.svg`.
- Di atas foto/video: letakkan versi putih pada area tenang yang gelap, atau gunakan wadah ikon `app-icon-color.svg`.

## 5. Tipografi
- **Wordmark**: Huruf geometris khusus yang seluruhnya dikonstruksi sebagai kurva vektor `<path>` (tanpa ketergantungan font sistem).
- **UI & teks pendamping**: **Inter** (Regular, Medium, SemiBold) untuk judul dan antarmuka; **JetBrains Mono** untuk angka durasi, FPS, dan kode teknis (keduanya berlisensi terbuka **SIL OFL 1.1** di folder `assets/fonts/`).
- **Fallback web**: `Inter, system-ui, -apple-system, "Segoe UI", sans-serif`.

## 6. Larangan Pemakaian (Don'ts)
- Jangan meregangkan, memipihkan, atau memutar logo di luar orientasi horizontalnya.
- Jangan mengganti warna di luar palet resmi atau menambah gradasi, bayangan (*drop shadow*), maupun garis luar (*outline*).
- Jangan mengubah proporsi atau jarak antara simbol **Loop-L** dan wordmark `LOOPKARAOKE`.
- Jangan mengetik ulang wordmark `LOOPKARAOKE` dengan font biasa untuk menggantikan berkas master.
- Jangan memakai versi `final-symbol.svg` standar di bawah 24 px jika versi `final-symbol-small.svg` / `favicon.ico` tersedia.

## 7. Catatan Teknis & Hak Cipta
- Seluruh berkas master SVG menggunakan kurva vektor penuh (tanpa `<text>`, tanpa raster, tanpa filter) dan dapat langsung dipakai di web, cetak, atau dikonversi ke `.ico` / aset aplikasi.
- Sebelum rilis komersial atau pendaftaran merek dagang resmi, lakukan penelusuran merek dagang (*trademark search*) dan *reverse image search* secara independen.
