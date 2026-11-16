use rand::Rng;

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

/// A short random lowercase alphanumeric identifier (e.g. `k3x9ab`) for unique video names.
pub fn random_code(length: usize) -> String {
    const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut rng = rand::thread_rng();
    (0..length)
        .map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char)
        .collect()
}

/// Unique benign MP4 comment tag (`RenderPro <uuid-v4>`) so repeated renders never match by hash.
pub fn unique_render_comment() -> String {
    format!("RenderPro {}", uuid::Uuid::new_v4())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_render_comment_uses_uuid_v4_prefix() {
        let a = unique_render_comment();
        let b = unique_render_comment();
        assert_ne!(a, b);
        assert!(a.starts_with("RenderPro "));
        let uuid_part = a.trim_start_matches("RenderPro ");
        assert!(uuid::Uuid::parse_str(uuid_part).is_ok());
    }

    #[test]
    fn random_code_generates_requested_lowercase_alnum_length() {
        let code = random_code(6);
        assert_eq!(code.len(), 6);
        assert!(code
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
    }
}
