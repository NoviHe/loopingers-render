# Standar kontrol UI

Sumber ukuran dan gaya kontrol ada di `src/app/theme.rs`. Generate, Batch Render,
Playlist, Thumbnail, Karaoke, dan tab lainnya memakai standar yang sama.

- Tinggi kontrol dan input satu baris: 32 px.
- Radius sudut kontrol: 8 px.
- Input: latar `Palette::bg`, border `Palette::border`, placeholder `Palette::text_muted`.
- Input teks biasa: gunakan `theme::text_field` atau trait `UiInputs`.
  `InputWidth` menyatakan lebar total termasuk bingkai dan padding; lebar dibatasi
  ruang yang tersedia. Gunakan `Fill` untuk kolom panjang, `Standard` atau `Fixed`
  untuk kolom dalam baris.
- Select: gunakan `egui::ComboBox` dengan gaya global. Batasi lebar sesuai kolom,
  gunakan `.truncate()` untuk label panjang, dan jangan menimpa radius atau tinggi.
- Slider pengaturan: gunakan `theme::control_slider`. Nilai dapat diedit melalui
  input angka di samping slider; range dan satuan tetap ditentukan pemanggil.
- Tombol: gunakan `StyledButton` atau helper tombol bersama.
- Gunakan token `theme::metrics` untuk ukuran baru, bukan angka khusus per tab.

Kontrol transport, seek audio/video, editor tabel, serta input gabungan dengan
stepper atau badge mempertahankan tata letak sesuai fungsinya. Input gabungan
memakai token, warna, dan radius yang sama. Tinggi editor multiline mengikuti
jumlah baris, bukan tinggi input satu baris.

`apply_control_style` diterapkan pada akar konten tab. Jangan menambahkan override
gaya per tab untuk meniru kontrol di tab lain. Tambahkan variasi yang memang
dibutuhkan ke komponen bersama. Pengujian headless di `theme::control_tests`
memeriksa kesamaan tinggi kontrol dan ukuran input pada panel sempit.
