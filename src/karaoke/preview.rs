//! Owned audio previews, isolated from render intermediates and other app instances.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime},
};

const SESSION_PREFIX: &str = "lrc-preview-";
const PREVIEW_PREFIX: &str = "preview-";
const LOCK_NAME: &str = ".lock";
const LOCK_MARKER: &[u8] = b"LoopKaraoke Studio preview session v1\n";
const LEGACY_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// Clones share the same session; it survives while an import or preview owns it.
#[derive(Clone, Debug)]
pub struct PreviewSession(Arc<SessionInner>);

#[derive(Debug)]
struct SessionInner {
    directory: PathBuf,
    lock: Option<File>,
}

impl PreviewSession {
    pub fn new(temp_dir: &Path) -> io::Result<Self> {
        fs::create_dir_all(temp_dir)?;
        let temp_dir = fs::canonicalize(temp_dir)?;
        cleanup_abandoned(&temp_dir);
        let directory = temp_dir.join(format!("{SESSION_PREFIX}{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory)?;
        let created = (|| {
            let mut lock = open_lock(&directory.join(LOCK_NAME), true)?;
            lock.write_all(LOCK_MARKER)?;
            lock.flush()?;
            Ok(Self(Arc::new(SessionInner {
                directory: directory.clone(),
                lock: Some(lock),
            })))
        })();
        if created.is_err() {
            let _ = fs::remove_file(directory.join(LOCK_NAME));
            let _ = fs::remove_dir(&directory);
        }
        created
    }

    pub fn directory(&self) -> &Path {
        &self.0.directory
    }

    /// Allocates a unique output path. FFmpeg creates the contents later.
    pub fn create_file(&self) -> PreviewFile {
        PreviewFile {
            path: self
                .0
                .directory
                .join(format!("{PREVIEW_PREFIX}{}.wav", uuid::Uuid::new_v4())),
            session: self.clone(),
        }
    }
}

/// Dropping a track, failed import, or unreceived event releases its preview.
#[derive(Debug)]
pub struct PreviewFile {
    path: PathBuf,
    session: PreviewSession,
}

impl PreviewFile {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PreviewFile {
    fn drop(&mut self) {
        // The path is generated inside this session, never supplied by the user.
        if self.path.parent() == Some(self.session.directory()) {
            let _ = fs::remove_file(&self.path);
        }
    }
}

impl Drop for SessionInner {
    fn drop(&mut self) {
        remove_session_contents(&self.directory);
        // Windows does not allow removing the sentinel while its exclusive handle is open.
        drop(self.lock.take());
        remove_session_if_empty(&self.directory);
    }
}

fn open_lock(path: &Path, create_new: bool) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    if create_new {
        options.create_new(true);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    let file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        unsafe extern "C" {
            fn flock(
                fd: std::os::raw::c_int,
                operation: std::os::raw::c_int,
            ) -> std::os::raw::c_int;
        }
        // LOCK_EX | LOCK_NB: never wait for an active application instance.
        if unsafe { flock(file.as_raw_fd(), 2 | 4) } != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(file)
}

fn session_name(name: &str) -> bool {
    name.strip_prefix(SESSION_PREFIX)
        .is_some_and(|suffix| uuid::Uuid::parse_str(suffix).is_ok())
}

fn preview_name(name: &str) -> bool {
    name.strip_prefix(PREVIEW_PREFIX)
        .and_then(|suffix| suffix.strip_suffix(".wav"))
        .is_some_and(|suffix| uuid::Uuid::parse_str(suffix).is_ok())
}

/// Returns only owned WAV filenames directly inside recognized session directories.
/// This is read-only and also includes previews belonging to active sessions.
pub(crate) fn managed_preview_files(temp_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(temp_dir) = fs::canonicalize(temp_dir) else {
        return files;
    };
    let Ok(sessions) = fs::read_dir(temp_dir) else {
        return files;
    };
    for session in sessions.flatten() {
        if !session.file_type().is_ok_and(|kind| kind.is_dir())
            || !session_name(&session.file_name().to_string_lossy())
        {
            continue;
        }
        let Ok(entries) = fs::read_dir(session.path()) else {
            continue;
        };
        files.extend(entries.flatten().filter_map(|entry| {
            (entry.file_type().is_ok_and(|kind| kind.is_file())
                && preview_name(&entry.file_name().to_string_lossy()))
            .then(|| entry.path())
        }));
    }
    files
}

fn remove_session_contents(directory: &Path) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        // No recursive traversal: preserve unknown files, subdirectories, and symlinks.
        if entry.file_type().is_ok_and(|kind| kind.is_file())
            && preview_name(&entry.file_name().to_string_lossy())
        {
            let _ = fs::remove_file(entry.path());
        }
    }
}

fn remove_session_if_empty(directory: &Path) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries {
        let Ok(entry) = entry else {
            return;
        };
        if entry.file_name() != LOCK_NAME {
            // A Windows playback/FFmpeg handle may still be closing. Retain the
            // marker so the next startup can retry rather than orphaning this file.
            return;
        }
    }
    let _ = fs::remove_file(directory.join(LOCK_NAME));
    let _ = fs::remove_dir(directory);
}

fn cleanup_abandoned(temp_dir: &Path) {
    let Ok(entries) = fs::read_dir(temp_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if kind.is_dir() && session_name(&name) {
            cleanup_session(&entry.path());
        } else if kind.is_file() && legacy_preview_name(&name) {
            // Old releases have no ownership lock. Keep recent files because an older
            // instance might still be using them, then retire them after one day.
            let old = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .ok()
                .and_then(|modified| SystemTime::now().duration_since(modified).ok())
                .is_some_and(|age| age >= LEGACY_MAX_AGE);
            if old {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

fn legacy_preview_name(name: &str) -> bool {
    if let Some(id) = name
        .strip_prefix("gen_preview_")
        .and_then(|name| name.strip_suffix(".wav"))
    {
        return !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit());
    }
    name.strip_prefix("lrc_studio_preview_")
        .and_then(|suffix| suffix.strip_suffix(".wav"))
        .is_some_and(|suffix| {
            suffix.len() == 8 && suffix.bytes().all(|b| b.is_ascii_alphanumeric())
        })
}

fn cleanup_session(directory: &Path) {
    // On unsupported platforms we cannot establish whether another app is alive.
    if !cfg!(any(windows, unix)) {
        return;
    }
    let sentinel = directory.join(LOCK_NAME);
    if !fs::symlink_metadata(&sentinel).is_ok_and(|metadata| metadata.is_file()) {
        return;
    }
    let Ok(mut lock) = open_lock(&sentinel, false) else {
        return;
    };
    let mut marker = vec![0; LOCK_MARKER.len()];
    if lock.read_exact(&mut marker).is_err() || marker != LOCK_MARKER {
        return;
    }
    remove_session_contents(directory);
    drop(lock);
    remove_session_if_empty(directory);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_root() -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("loopkaraoke-preview-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        root
    }

    #[test]
    fn preview_is_removed_when_its_owner_is_dropped() {
        let root = test_root();
        let session = PreviewSession::new(&root).unwrap();
        let preview = session.create_file();
        let path = preview.path().to_path_buf();
        fs::write(&path, b"preview").unwrap();
        drop(preview);
        assert!(!path.exists());
        drop(session);
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn import_handle_keeps_session_alive_until_result_is_dropped() {
        let root = test_root();
        let session = PreviewSession::new(&root).unwrap();
        let preview = session.create_file();
        let directory = session.directory().to_path_buf();
        fs::write(preview.path(), b"pending import").unwrap();
        drop(session);
        assert!(directory.is_dir());
        drop(preview);
        assert!(!directory.exists());
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn second_session_preserves_live_session_and_its_preview() {
        let root = test_root();
        let first = PreviewSession::new(&root).unwrap();
        let preview = first.create_file();
        fs::write(preview.path(), b"active preview").unwrap();
        let second = PreviewSession::new(&root).unwrap();
        assert!(preview.path().is_file());
        assert!(first.directory().is_dir());
        drop(preview);
        drop(first);
        drop(second);
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn new_session_removes_abandoned_managed_files_but_preserves_other_data() {
        let root = test_root();
        let abandoned = root.join(format!("{SESSION_PREFIX}{}", uuid::Uuid::new_v4()));
        fs::create_dir(&abandoned).unwrap();
        fs::write(abandoned.join(LOCK_NAME), LOCK_MARKER).unwrap();
        fs::write(
            abandoned.join(format!("{PREVIEW_PREFIX}{}.wav", uuid::Uuid::new_v4())),
            b"old",
        )
        .unwrap();
        let unknown = root.join(format!("{SESSION_PREFIX}{}", uuid::Uuid::new_v4()));
        fs::create_dir(&unknown).unwrap();
        fs::write(unknown.join(LOCK_NAME), b"not an app marker").unwrap();
        fs::write(unknown.join("original.wav"), b"original").unwrap();
        let original = root.join("original.mp3");
        fs::write(&original, b"original").unwrap();
        let session = PreviewSession::new(&root).unwrap();
        assert!(!abandoned.exists());
        assert!(unknown.join("original.wav").is_file());
        assert!(original.is_file());
        drop(session);
        fs::remove_file(original).unwrap();
        fs::remove_file(unknown.join(LOCK_NAME)).unwrap();
        fs::remove_file(unknown.join("original.wav")).unwrap();
        fs::remove_dir(unknown).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn legacy_cleanup_only_removes_old_recognized_previews() {
        let root = test_root();
        let old = root.join("lrc_studio_preview_abcd1234.wav");
        let recent = root.join("lrc_studio_preview_1234abcd.wav");
        let old_playlist = root.join("gen_preview_1.wav");
        let recent_playlist = root.join("gen_preview_2.wav");
        let original = root.join("lrc_studio_preview_original.wav");
        for path in [&old, &recent, &original, &old_playlist, &recent_playlist] {
            fs::write(path, b"audio").unwrap();
        }
        File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_modified(SystemTime::now() - LEGACY_MAX_AGE - Duration::from_secs(1))
            .unwrap();
        let session = PreviewSession::new(&root).unwrap();
        assert!(!old.exists());
        // A subsequent startup also retires legacy playlist previews once old enough.
        File::options()
            .write(true)
            .open(&old_playlist)
            .unwrap()
            .set_modified(SystemTime::now() - LEGACY_MAX_AGE - Duration::from_secs(1))
            .unwrap();
        let other = PreviewSession::new(&root).unwrap();
        assert!(!old_playlist.exists());
        assert!(recent_playlist.is_file());
        drop(other);
        assert!(recent.is_file());
        assert!(original.is_file());
        drop(session);
        fs::remove_file(recent).unwrap();
        fs::remove_file(recent_playlist).unwrap();
        fs::remove_file(original).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn locked_preview_keeps_ownership_marker_until_a_later_startup_can_remove_it() {
        use std::os::windows::fs::OpenOptionsExt;

        let root = test_root();
        let session = PreviewSession::new(&root).unwrap();
        let preview = session.create_file();
        let directory = session.directory().to_path_buf();
        let path = preview.path().to_path_buf();
        fs::write(&path, b"preview still closing").unwrap();
        let playback = OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&path)
            .unwrap();
        drop(preview);
        drop(session);
        assert!(directory.join(LOCK_NAME).is_file());
        let second = PreviewSession::new(&root).unwrap();
        assert!(path.is_file());
        assert!(directory.join(LOCK_NAME).is_file());
        drop(playback);
        let third = PreviewSession::new(&root).unwrap();
        assert!(!directory.exists());
        drop(second);
        drop(third);
        fs::remove_dir(root).unwrap();
    }
}
