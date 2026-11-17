//! Hover explanations shown next to the ⓘ icons.

pub const TAB_BATCH_RENDER: &str = "Dari satu project (daftar lagu) ke banyak video dengan urutan lagu, durasi, visual & thumbnail berbeda.";
pub const TAB_PLAYLIST: &str =
    "Kelola daftar lagu Generate: susun atau acak urutan, pratinjau audio, serta salin dan ekspor daftar lagu atau timestamp YouTube. Sinkron dengan Generate dan Batch Render.";
pub const TAB_LYRICS: &str = "Studio LRC: ambil lagu dari Generate atau impor audio, buat lirik dengan Whisper, koreksi teks dan waktu, lalu simpan berkas LRC.";
pub const TAB_THUMBNAIL: &str =
    "Buat thumbnail mandiri dari gambar yang diunggah atau gunakan video Generate. Atur judul, sub judul, font, dan daftar lagu, lalu ekspor JPG Full HD.";
pub const TAB_VISUALIZER: &str = "Buat visualizer musik reaktif audio (ala VIZ) dengan spektrum frekuensi, teks dinamis, logo, dan partikel.";
pub const TAB_STORAGE: &str = "Lihat pemakaian disk dan bersihkan berkas yang tidak diperlukan.";

pub mod background {
    #[allow(dead_code)]
    pub const DEMO: &str = "Memuat video latar dan 2 lagu contoh berlirik supaya seluruh alur bisa dicoba tanpa menyiapkan file sendiri.";
    #[allow(dead_code)]
    pub const OPEN_PLAYER: &str =
        "Membuka berkas background di pemutar bawaan sistem untuk memeriksanya.";
    pub const ASPECT: &str = "Bentuk layar video akhir: 16:9 untuk YouTube biasa, 9:16 untuk Shorts/Reels/TikTok, 1:1 untuk persegi. Bersama resolusi, ini menentukan ukuran keluaran.";
    pub const FIT: &str = "Cara background mengisi layar jika rasionya berbeda. Crop Penuh memotong tepi agar layar terisi; Letterbox menampilkan seluruh video dengan pita hitam; Blur Estetik mengisi pita itu dengan versi buram dari video.";
    pub const EFFECT: &str = "Overlay animasi atau filter warna yang ditumpuk di atas background. Arahkan kursor ke tiap pilihan untuk melihat deskripsinya.";
    pub const MOTION: &str = "Gerakan kamera pada gambar diam, zoom atau geser perlahan, agar tidak terlihat statis. \"Diam\" berarti tanpa gerakan.";
    pub const BEAT_PULSE: &str =
        "Gambar berdenyut halus mengikuti ketukan, dengan tempo tetap 105 BPM.";
    pub const APPLY_IMAGE: &str = "Membuat ulang klip dari gambar dengan rasio, efek, gerakan, dan denyut yang sedang dipilih.";
}

pub mod lyrics {
    pub const HEADING: &str = "Buat dan edit berkas LRC dari audio atau lagu Generate: transkripsi Whisper, impor teks, sinkronisasi waktu, dan ekspor LRC.";
    pub const WHISPER: &str = "Whisper adalah AI pengenal suara yang mendengarkan lagu dan membuat lirik berstempel waktu per kata. Berjalan di komputer sendiri.";
    #[cfg(any(feature = "cuda", feature = "vulkan"))]
    pub const MODEL: &str = "Model lebih besar lebih akurat tetapi lebih lambat dan lebih besar untuk diunduh: tiny sekitar 75 MB, base 142 MB, small 466 MB, medium 1.5 GB.";
    #[cfg(not(any(feature = "cuda", feature = "vulkan")))]
    pub const MODEL: &str = "Model lebih besar lebih akurat tetapi lebih lambat dan lebih besar untuk diunduh: tiny sekitar 75 MB, base 142 MB, small 466 MB.";
    pub const LANGUAGE: &str = "Bahasa lirik. Memilih bahasa yang benar membuat hasil lebih akurat; \"Deteksi otomatis\" membiarkan Whisper menebak.";
    pub const GPU: &str = "Mempercepat Whisper dengan GPU. Hanya aktif bila build ini mendukungnya (--features cuda atau vulkan); jika tidak, Whisper berjalan di CPU.";
    pub const STYLE: &str =
        "Mengatur tampilan teks karaoke di video: tema warna, ukuran, posisi, dan efek sapuan.";
    pub const COLORS: &str = "Normal: warna kata sebelum dinyanyikan. Saat bernyanyi: warna yang menyapu kata ketika dinyanyikan. Garis tepi dan bayangan menjaga teks tetap terbaca di atas background terang.";
    pub const FONT_CHOICE: &str =
        "Pilih jenis font lirik karaoke dari preset populer (Impact, Arial Black, Segoe UI, dll) atau unggah berkas font kustom (.ttf / .otf).";
    pub const FONT_SIZE: &str =
        "Ukuran huruf lirik pada video. Naikkan jika teks sulit dibaca di layar kecil.";
    pub const MARGIN: &str =
        "Jarak teks dari tepi layar, mengikuti posisi atas atau bawah yang dipilih.";
    pub const POSITION: &str =
        "Letak teks lirik karaoke di layar: geser slider X (horizontal) dan Y (vertikal) secara leluasa atau gunakan preset cepat Bawah, Tengah, Atas.";
    pub const WIPE: &str = "Halus: warna menyapu kata secara mulus saat dinyanyikan. Per kata: seluruh kata berganti warna sekaligus.";
    pub const INTRO: &str =
        "Menampilkan judul dan nama penyanyi sebagai banner singkat di awal setiap lagu.";
}

pub mod thumbnail {
    pub const HEADING: &str = "Unggah gambar sendiri untuk membuat thumbnail mandiri, atau ambil frame video Generate. Tambahkan judul dan daftar lagu, lalu ekspor JPG 1920x1080.";
    pub const TITLE: &str = "Teks besar utama pada thumbnail.";
    pub const SUBTITLE: &str = "Teks kecil di bawah judul, biasanya nama penyanyi atau kanal.";
    pub const FRAME: &str = "Detik pada video background yang diambil sebagai latar thumbnail. Pilih momen paling menarik.";
    pub const COLUMNS: &str =
        "Daftar lagu dibagi menjadi satu atau dua kolom. Otomatis memilih berdasarkan jumlah lagu.";
    pub const NUMBERING: &str = "Cermin: kolom kanan memakai Judul .01 dan mengulang nomor dari 01. Berurutan: nomor melanjutkan kolom kiri, misalnya 11. Judul. Cermin Berurutan: nomor tetap berlanjut, dengan format Judul .11.";
    pub const TITLE_SIZE: &str = "Ukuran huruf judul utama.";
    pub const TRACK_SIZE: &str = "Ukuran huruf daftar lagu. Kecilkan bila daftar terlalu panjang.";
    pub const VIGNETTE: &str =
        "Menggelapkan tepi gambar supaya teks lebih terbaca. 0 berarti tanpa gelap.";
    pub const COLORS: &str =
        "Warna judul utama, daftar lagu, dan sub judul, berurutan dari kiri ke kanan.";
}

pub mod visualizer {
    pub const HEADING: &str = "Studio Visualizer Musik: buat animasi visual spektrum audio yang bergerak mengikuti ritme lagu, atur gaya, warna, teks, logo, partikel, dan efek latar belakang.";
    pub const PRESET: &str = "Bentuk visualizer spektrum: balok spektrum (bars), lingkaran memutar (radial NCS), lampu LED, gelombang halus, atau grid cyber.";
    pub const POSITION: &str =
        "Penempatan visualizer di layar: Tengah (Center), Bawah (Bottom), atau Layar Penuh (Full).";
    pub const SIZE: &str = "Skala / ukuran visualizer terhadap kanvas video.";
    pub const SENSITIVITY: &str =
        "Tingkat sensitivitas / reaksi ketinggian spektrum terhadap volume dan ketukan audio.";
    pub const ROTATION: &str =
        "Sudut putaran visualizer. Tombol Rata 0° mengembalikan orientasi tegak lurus.";
    pub const COLORS: &str = "Gradien warna spektrum: Warna A untuk bagian bawah/pangkal, dan Warna B untuk bagian atas/ujung.";
    pub const BG_FX: &str = "Efek latar belakang: tingkat gelap (dim), keburaman (blur), warna nuansa (tint), getaran ketukan (pulse), dan bingkai neon RGB bercahaya.";
    pub const TEXT: &str =
        "Teks informasi lagu atau watermark kustom dengan efek neon glow dan bayangan.";
    pub const OVERLAY: &str = "Gambar logo atau ikon kanal transparan yang ditempatkan di depan atau di belakang visualizer, dapat bergetar mengikuti ketukan.";
    pub const PARTICLES: &str =
        "Efek partikel animasi mengambang (bintik debu, bintang, percikan api, salju, hujan neon).";
}

pub mod storage {
    pub const HEADING: &str =
        "Melihat pemakaian disk aplikasi dan membersihkan berkas yang tidak diperlukan.";
    pub const TEMP: &str =
        "Berkas perantara dari render dan klip gambar. Aman dihapus; dibuat ulang saat dibutuhkan.";
    pub const OUTPUTS: &str =
        "Video dan thumbnail yang sudah jadi. Menghapus semua video hasil tidak bisa dibatalkan.";
    pub const MODELS: &str =
        "Model Whisper yang sudah diunduh. Diunduh lagi otomatis saat dipakai.";
    pub const CLEAN_TEMP: &str = "Menghapus semua berkas sementara.";
    pub const CLEAN_OUTPUTS: &str = "Menghapus semua video hasil render. Tidak bisa dibatalkan.";
    pub const FILES: &str =
        "Berkas hasil satu per satu. Buka untuk memeriksa, atau hapus yang tidak dipakai.";
}
