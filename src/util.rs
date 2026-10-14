pub fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

pub fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

/// A short random hex identifier, used in generated file names and job ids.
pub fn random_id(length: usize) -> String {
    let mut id = uuid::Uuid::new_v4().simple().to_string();
    id.truncate(length);
    id
}

/// Opens a file or folder with the operating system's default application.
pub fn open_path(path: &std::path::Path) -> std::io::Result<()> {
    let opener = if cfg!(windows) {
        "explorer.exe"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(opener)
        .arg(path)
        .spawn()
        .map(drop)
}
