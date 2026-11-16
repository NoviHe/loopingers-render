#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;

use std::sync::Arc;

use loopkaraoke::{config::AppPaths, ffmpeg::Tools, karaoke::transcribe::Transcriber};

fn load_window_icon() -> Option<Arc<eframe::egui::IconData>> {
    let img = image::load_from_memory(include_bytes!("../assets/logo/app-icon-color.png"))
        .ok()?
        .into_rgba8();
    let (width, height) = img.dimensions();
    Some(Arc::new(eframe::egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    }))
}

fn main() -> eframe::Result {
    let paths = AppPaths::detect().expect("data folders can be created");
    let tools = Tools::locate();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("async runtime starts");
    let transcriber = Arc::new(Transcriber::default());

    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("Loopingers Render")
        .with_inner_size([1280.0, 840.0])
        .with_min_inner_size([980.0, 640.0]);
    if let Some(icon) = load_window_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "Loopingers Render",
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
