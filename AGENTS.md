# Repository Guidelines

## Project Structure & Module Organization

LoopKaraoke Studio is a native Rust 2021 desktop application using eframe/egui. `src/main.rs` launches the application; `src/app/` contains UI modules organized by tab and shared theme components. Rendering belongs in `src/render/`, karaoke parsing and transcription in `src/karaoke/`, and media probing in `src/media/`. Integration tests live in `tests/`. Fonts, logos, and video overlays live in `assets/`; `examples/gen_effects/` generates overlays. `build.bat` packages Windows releases into `dist/`.

## Build, Test, and Development Commands

Use Rust 1.85 or newer. Windows builds require C++ Build Tools, CMake, and LLVM/libclang; set `LIBCLANG_PATH` when necessary. Vulkan is enabled by default and requires its SDK. For CPU-only Whisper builds, add `--no-default-features`.

- `cargo run`: launch the debug application.
- `cargo check`: check compilation without producing the executable.
- `cargo build --release`: build an optimized executable.
- `cargo build --release --no-default-features --features cuda`: build with CUDA instead of default Vulkan.
- `cargo fmt --check`: verify Rust formatting.
- `cargo clippy --all-targets -- -D warnings`: run lint checks.
- `cargo test`: run unit and integration tests; rendering tests require FFmpeg/FFprobe on `PATH`.
- `.\build.bat`: build and package the Windows application.

## Coding Style & Naming Conventions

Use rustfmt formatting and four-space indentation. Use `snake_case` for modules, functions, and variables, `PascalCase` for types, and `SCREAMING_SNAKE_CASE` for constants. Reuse shared egui theme and widget helpers. Keep media processing in backend modules and follow the existing Indonesian wording for UI text.

## Testing Guidelines

Use Rust `#[test]` and Tokio `#[tokio::test]`, with descriptive behavior-based names. Keep unit tests beside implementation and end-to-end tests in `tests/`. Add regression coverage for changed parsing, planning, or rendering behavior; no numeric coverage threshold is configured. Run relevant tests first, then the full suite when dependencies are available. Download tests are opt-in: `cargo test --test install -- --ignored`. Whisper tests also require `WHISPER_TEST_AUDIO` and download a model.

## Commit & Pull Request Guidelines

Follow existing prefixes such as `feat(ui):`, `feat(whisper):`, and `docs:` with concise imperative summaries. Keep commits focused. PRs should describe the resulting behavior, link relevant issues, report validation and dependency limitations, and include screenshots for UI changes.

## Configuration & Generated Files

Use `LOOPKARAOKE_HOME` to isolate runtime data. Keep generated `target/`, `dist/`, `temp/`, `outputs/`, downloaded tools, and models out of commits. Preserve unrelated working-tree changes.

## Native Codex Subagents

Gunakan hanya native Codex subagents. Jangan gunakan Orca Orchestration, membuat Run, Task, Dispatch, atau Worker orchestration, maupun melakukan polling status orchestration.

### Strategi Eksekusi

- Default: kerjakan langsung tanpa subagent, terutama untuk perubahan kecil, perencanaan sederhana, diagnosis terarah, dan pengujian ringan.
- Gunakan satu subagent hanya jika memberi manfaat jelas untuk pekerjaan independen atau implementasi substansial. Maksimal dua subagent bersamaan, mengikuti batas runtime yang lebih ketat jika ada.
- Jangan membuat subagent hanya untuk mengikuti alur kerja, menduplikasi tugas, mendelegasikan perubahan trivial, atau membuat subagent bersarang.
- Tentukan lingkup tugas dan kepemilikan file dengan jelas. Ingatkan subagent bahwa workspace digunakan bersama dan perubahan pihak lain harus dipertahankan.
- Inspeksi file yang relevan, gunakan ulang kode serta pengujian yang ada, dan minimalkan pengulangan konteks.

### Routing Agent

| Agent | Model | Effort | Penggunaan |
| --- | --- | --- | --- |
| `brainstorm` | `gpt-6-luna` | `low` | Ide atau riset independen yang bermanfaat |
| `coding` | `gpt-6.1-sol` | `medium` | Implementasi substansial, arsitektur kompleks, atau debugging lanjutan |
| `testing` | `gpt-6-luna` | `low` | Pengembangan pengujian atau verifikasi independen |
| `review` | `gpt-6.1-sol` | `medium` | Review perubahan kompleks, kode sensitif keamanan, atau verifikasi independen |

Gunakan tipe agent native dan konfigurasi custom yang tersedia; jangan menerapkan flag CLI Orca pada tool subagent. Model agent utama tidak otomatis berubah mengikuti routing. Naikkan effort hanya bila diperlukan dan didukung runtime.

### Verifikasi Kemampuan

Pada awal sesi, baca konfigurasi Codex yang berlaku dan definisi custom agent. Periksa versi terpasang, dukungan tool native subagents, serta ketersediaan model dan effort sebelum routing. Jangan menjalankan subagent hanya untuk verifikasi. Bedakan kemampuan yang tersedia dari eksekusi yang benar-benar telah diuji.

Jika model atau tipe agent tidak tersedia, periksa alternatif yang didukung, pilih yang paling sesuai, dan laporkan fallback sekali. Jangan mengarang ID model atau opsi yang tidak didukung. Patuhi permissions dan sandbox aktual.

### Validasi dan Pelaporan

Selesaikan pekerjaan development biasa secara mandiri: analisis, implementasi, pengujian yang sesuai, lalu perbaiki error dengan aman. Minta persetujuan eksplisit untuk perubahan destruktif, kehilangan data, publikasi, pembelian, atau perubahan keamanan kritis.

Pertahankan kode, data, dan perubahan working tree yang tidak terkait. Jalankan validasi sesuai perubahan; jangan menyatakan pengujian yang belum dijalankan sebagai lulus. Untuk perubahan dokumentasi saja, periksa isi dan diff tanpa menjalankan build yang tidak relevan.

Laporkan singkat dalam Bahasa Indonesia dengan label: Status, Hasil, Testing, Masalah. Hindari konfirmasi, perencanaan panjang, dan laporan progres berulang yang tidak diperlukan.
