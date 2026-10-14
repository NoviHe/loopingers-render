#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;

use std::sync::Arc;

use loopkaraoke::{config::AppPaths, ffmpeg::Tools, karaoke::transcribe::Transcriber};

fn main() -> eframe::Result {
    let paths = AppPaths::detect().expect("data folders can be created");
    let tools = Tools::locate();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("async runtime starts");
    let transcriber = Arc::new(Transcriber::default());

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("LoopKaraoke Studio")
            .with_inner_size([1280.0, 840.0])
            .with_min_inner_size([980.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native(
        "LoopKaraoke Studio",
        options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            app::install_fonts(&cc.egui_ctx);
            app::theme::apply(&cc.egui_ctx);
            Ok(Box::new(app::StudioApp::new(
                cc.egui_ctx.clone(),
                paths,
                tools,
                runtime,
                transcriber,
            )))
        }),
    )
}
