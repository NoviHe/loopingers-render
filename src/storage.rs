use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use chrono::{DateTime, Local};

use crate::{config::AppPaths, util::round2};

const BYTES_PER_MB: f64 = 1024.0 * 1024.0;

fn megabytes(bytes: u64) -> f64 {
    round2(bytes as f64 / BYTES_PER_MB)
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub size_mb: f64,
    pub modified_str: String,
}

#[derive(Debug, Clone, Default)]
pub struct DirStats {
    pub size_bytes: u64,
    pub size_mb: f64,
    pub count: usize,
    /// Newest first.
    pub files: Vec<FileEntry>,
}

#[derive(Debug, Clone, Default)]
pub struct StorageStats {
    pub temp: DirStats,
    pub outputs: DirStats,
    pub models: DirStats,
    pub total_app_mb: f64,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct CleanupCounts {
    pub temp: usize,
    pub outputs: usize,
}

#[derive(Debug, Default)]
pub struct CleanupOptions {
    pub clean_temp: bool,
    pub clean_outputs: bool,
    /// Only files at least this old are removed.
    pub min_age: Option<Duration>,
}

fn dir_stats(dir: &Path) -> DirStats {
    let mut files: Vec<(SystemTime, FileEntry)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let metadata = entry.metadata().ok().filter(fs::Metadata::is_file)?;
            let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
            Some((
                modified,
                FileEntry {
                    name: entry.file_name().to_string_lossy().into_owned(),
                    path: entry.path(),
                    size_bytes: metadata.len(),
                    size_mb: megabytes(metadata.len()),
                    modified_str: DateTime::<Local>::from(modified)
                        .format("%Y-%m-%d %H:%M:%S")
                        .to_string(),
                },
            ))
        })
        .collect();
    files.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));

    let size_bytes = files.iter().map(|(_, file)| file.size_bytes).sum();
    DirStats {
        size_bytes,
        size_mb: megabytes(size_bytes),
        count: files.len(),
        files: files.into_iter().map(|(_, file)| file).collect(),
    }
}

pub fn stats(paths: &AppPaths) -> StorageStats {
    let temp = dir_stats(&paths.temp);
    let outputs = dir_stats(&paths.outputs);
    let total_app_mb = megabytes(temp.size_bytes + outputs.size_bytes);
    StorageStats {
        temp,
        outputs,
        models: dir_stats(&paths.models),
        total_app_mb,
    }
}

fn is_old_enough(path: &Path, min_age: Option<Duration>) -> bool {
    let Some(min_age) = min_age else {
        return true;
    };
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age >= min_age)
}

fn remove_files(dir: &Path, min_age: Option<Duration>) -> usize {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_old_enough(path, min_age))
        .filter(|path| fs::remove_file(path).is_ok())
        .count()
}

/// Deletes intermediate files and/or finished videos, optionally only the older ones.
pub fn cleanup(paths: &AppPaths, options: &CleanupOptions) -> CleanupCounts {
    CleanupCounts {
        temp: if options.clean_temp {
            remove_files(&paths.temp, options.min_age)
        } else {
            0
        },
        outputs: if options.clean_outputs {
            remove_files(&paths.outputs, options.min_age)
        } else {
            0
        },
    }
}

/// Deletes one file, but only when it lives directly in the outputs or temp folder.
pub fn delete_file(paths: &AppPaths, file: &Path) -> bool {
    let inside = file
        .parent()
        .is_some_and(|parent| parent == paths.outputs || parent == paths.temp);
    inside && file.is_file() && fs::remove_file(file).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_paths(label: &str) -> AppPaths {
        let root =
            std::env::temp_dir().join(format!("loopkaraoke-test-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        AppPaths::new(&root, root.join("effects")).unwrap()
    }

    #[test]
    fn cleanup_only_touches_requested_folders() {
        let paths = temp_paths("cleanup");
        fs::write(paths.temp.join("a.ass"), b"x").unwrap();
        fs::write(paths.outputs.join("video.mp4"), b"x").unwrap();

        let options = CleanupOptions {
            clean_temp: true,
            ..CleanupOptions::default()
        };
        assert_eq!(
            cleanup(&paths, &options),
            CleanupCounts {
                temp: 1,
                outputs: 0
            }
        );
        assert!(paths.outputs.join("video.mp4").exists());

        let options = CleanupOptions {
            clean_outputs: true,
            ..CleanupOptions::default()
        };
        assert_eq!(
            cleanup(&paths, &options),
            CleanupCounts {
                temp: 0,
                outputs: 1
            }
        );
        let _ = fs::remove_dir_all(&paths.data_dir);
    }

    #[test]
    fn min_age_protects_fresh_files() {
        let paths = temp_paths("age");
        fs::write(paths.temp.join("fresh.ass"), b"x").unwrap();
        let options = CleanupOptions {
            clean_temp: true,
            min_age: Some(Duration::from_secs(3600)),
            ..CleanupOptions::default()
        };
        assert_eq!(cleanup(&paths, &options).temp, 0);
        assert!(paths.temp.join("fresh.ass").exists());
        let _ = fs::remove_dir_all(&paths.data_dir);
    }

    #[test]
    fn delete_file_is_confined_to_app_folders() {
        let paths = temp_paths("delete");
        let inside = paths.outputs.join("a.mp4");
        let outside = paths.data_dir.join("keep.txt");
        fs::write(&inside, b"x").unwrap();
        fs::write(&outside, b"x").unwrap();
        assert!(!delete_file(&paths, &outside));
        assert!(delete_file(&paths, &inside));
        let _ = fs::remove_dir_all(&paths.data_dir);
    }

    #[test]
    fn stats_count_files_and_bytes() {
        let paths = temp_paths("stats");
        fs::write(paths.outputs.join("a.mp4"), vec![0u8; 2048]).unwrap();
        let stats = stats(&paths);
        assert_eq!(stats.outputs.count, 1);
        assert_eq!(stats.outputs.size_bytes, 2048);
        let _ = fs::remove_dir_all(&paths.data_dir);
    }
}
