//! Serial Generate jobs with immutable requests and retained per-video results.
use std::{
    fmt::Write as _,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use super::{
    job::{JobHandle, JobStatus},
    RenderContext, SimpleRenderRequest,
};

pub struct BatchItem {
    pub request: SimpleRenderRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchStatus {
    Queued,
    Running,
    Completed,
    Stopped,
}

#[derive(Debug, Clone)]
pub struct BatchResult {
    pub output: Option<PathBuf>,
    pub error: Option<String>,
    pub warning: Option<String>,
    pub duration_seconds: f64,
}

#[derive(Clone)]
pub struct BatchSnapshot {
    pub status: BatchStatus,
    pub current: usize,
    pub total: usize,
    pub finished: Vec<BatchResult>,
    pub active_job: Option<JobHandle>,
    pub directory: Option<PathBuf>,
    pub warning: Option<String>,
}

#[derive(Clone)]
pub struct BatchHandle {
    state: Arc<Mutex<BatchSnapshot>>,
    stop: Arc<AtomicBool>,
}

impl BatchHandle {
    pub fn new(total: usize) -> Self {
        Self {
            state: Arc::new(Mutex::new(BatchSnapshot {
                status: BatchStatus::Queued,
                current: 0,
                total,
                finished: Vec::new(),
                active_job: None,
                directory: None,
                warning: None,
            })),
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn snapshot(&self) -> BatchSnapshot {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn is_running(&self) -> bool {
        matches!(
            self.snapshot().status,
            BatchStatus::Queued | BatchStatus::Running
        )
    }

    pub fn stop_after_current(&self) {
        self.stop.store(true, Ordering::Release);
    }

    pub fn stop_requested(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }

    fn update(&self, update: impl FnOnce(&mut BatchSnapshot)) {
        update(&mut self.state.lock().unwrap_or_else(|e| e.into_inner()));
    }
}

pub async fn execute(
    context: RenderContext,
    handle: BatchHandle,
    items: Vec<BatchItem>,
    repaint: impl Fn() + Send + Sync,
) {
    handle.update(|state| {
        state.status = BatchStatus::Running;
        state.total = items.len();
    });
    let Some(first) = items.first() else {
        handle.update(|state| state.status = BatchStatus::Completed);
        repaint();
        return;
    };
    let directory = match create_batch_directory(&first.request.output_dir).await {
        Ok(directory) => directory,
        Err(error) => {
            handle.update(|state| {
                state.status = BatchStatus::Stopped;
                state.warning = Some(format!("Folder batch gagal dibuat: {error}"));
            });
            repaint();
            return;
        }
    };
    handle.update(|state| state.directory = Some(directory.clone()));
    write_summary(&handle).await;
    for (index, mut item) in items.into_iter().enumerate() {
        if handle.stop.load(Ordering::Acquire) {
            break;
        }
        let job = JobHandle::new();
        handle.update(|state| {
            state.current = index;
            state.active_job = Some(job.clone());
        });
        repaint();
        item.request.output_dir = directory.clone();
        super::execute_simple(&context, &job, item.request).await;
        let result = job.snapshot();
        let mut warning = result.report_warning;
        let output = match result.output_file {
            Some(output) => match organize_video_folder(&directory, &output, index + 1).await {
                Ok(output) => Some(output),
                Err(error) => {
                    let note =
                        format!("Folder video gagal dirapikan; hasil tetap tersimpan: {error}");
                    if let Some(warning) = &mut warning {
                        warning.push('\n');
                        warning.push_str(&note);
                    } else {
                        warning = Some(note);
                    }
                    Some(output)
                }
            },
            None => None,
        };
        handle.update(|state| {
            state.finished.push(BatchResult {
                output,
                error: (result.status == JobStatus::Failed).then_some(result.error_message),
                warning,
                duration_seconds: result.total_time_sec,
            });
            state.active_job = None;
        });
        write_summary(&handle).await;
        repaint();
    }
    handle.update(|state| {
        state.active_job = None;
        state.status = if state.finished.len() < state.total {
            BatchStatus::Stopped
        } else {
            BatchStatus::Completed
        };
    });
    write_summary(&handle).await;
    repaint();
}

async fn create_batch_directory(base: &Path) -> std::io::Result<PathBuf> {
    tokio::fs::create_dir_all(base).await?;
    let date = chrono::Local::now().format("%Y-%m-%d");
    for sequence in 1_u64.. {
        let directory = base.join(format!("Batch_{date}_{sequence:03}"));
        match tokio::fs::create_dir(&directory).await {
            Ok(()) => return Ok(directory),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    unreachable!("batch sequence exhausted")
}

async fn organize_video_folder(
    directory: &Path,
    output: &Path,
    number: usize,
) -> std::io::Result<PathBuf> {
    let invalid = || std::io::Error::other("Lokasi hasil video di luar folder batch.");
    let source = output.parent().ok_or_else(invalid)?;
    if source.parent() != Some(directory)
        || tokio::fs::canonicalize(source).await?.parent()
            != Some(tokio::fs::canonicalize(directory).await?.as_path())
    {
        return Err(invalid());
    }
    let code = output
        .file_stem()
        .and_then(|s| s.to_str())
        .and_then(|s| s.strip_prefix("video_"))
        .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric()))
        .ok_or_else(|| std::io::Error::other("Kode video tidak valid."))?;
    let destination = directory.join(format!("{number:02}_{code}"));
    if destination.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "Folder tujuan sudah ada.",
        ));
    }
    tokio::fs::rename(source, &destination).await?;
    Ok(destination.join(output.file_name().ok_or_else(invalid)?))
}

fn format_summary(snapshot: &BatchSnapshot) -> String {
    let status = match snapshot.status {
        BatchStatus::Queued => "Menunggu",
        BatchStatus::Running => "Berjalan",
        BatchStatus::Completed => "Selesai",
        BatchStatus::Stopped => "Dihentikan",
    };
    let failed = snapshot
        .finished
        .iter()
        .filter(|item| item.error.is_some())
        .count();
    let mut text = format!("Ringkasan Batch Render\nStatus: {status}\nTotal video: {}\nBerhasil: {}\nGagal: {failed}\nBelum diproses: {}\n",
        snapshot.total, snapshot.finished.len() - failed, snapshot.total.saturating_sub(snapshot.finished.len()));
    if let Some(directory) = &snapshot.directory {
        let _ = writeln!(text, "Folder batch: {}", directory.display());
    }
    if let Some(warning) = &snapshot.warning {
        let _ = writeln!(text, "Peringatan: {warning}");
    }
    for (index, item) in snapshot.finished.iter().enumerate() {
        let status = if item.error.is_some() {
            "Gagal"
        } else {
            "Selesai"
        };
        let _ = writeln!(
            text,
            "\nVideo {:02}: {status}\nDurasi: {:.3} detik",
            index + 1,
            item.duration_seconds
        );
        if let Some(output) = &item.output {
            let _ = writeln!(text, "Hasil: {}", output.display());
        }
        if let Some(error) = &item.error {
            let _ = writeln!(text, "Error: {error}");
        }
        if let Some(warning) = &item.warning {
            let _ = writeln!(text, "Peringatan: {warning}");
        }
    }
    text
}

async fn write_summary(handle: &BatchHandle) {
    let snapshot = handle.snapshot();
    let Some(directory) = &snapshot.directory else {
        return;
    };
    if let Err(error) = tokio::fs::write(
        directory.join("ringkasan_batch.txt"),
        format_summary(&snapshot),
    )
    .await
    {
        handle.update(|state| {
            state.warning = Some(format!("Ringkasan batch gagal disimpan: {error}"))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn concurrent_batches_get_separate_directories_without_overwriting() {
        let root = std::env::temp_dir().join(format!("batch-folders-{}", uuid::Uuid::new_v4()));
        let (first, second) =
            tokio::join!(create_batch_directory(&root), create_batch_directory(&root));
        let (first, second) = (first.unwrap(), second.unwrap());
        assert_ne!(first, second);
        assert_eq!(first.parent(), Some(root.as_path()));
        assert!(first
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("Batch_"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn organizing_moves_the_whole_bundle_and_rejects_external_sources() {
        let root = std::env::temp_dir().join(format!("batch-bundle-{}", uuid::Uuid::new_v4()));
        let batch = create_batch_directory(&root).await.unwrap();
        let source = batch.join("generated-original");
        tokio::fs::create_dir(&source).await.unwrap();
        let video = source.join("video_abc123.mp4");
        tokio::fs::write(&video, b"video").await.unwrap();
        tokio::fs::write(source.join("thumbnail_abc123.png"), b"thumbnail")
            .await
            .unwrap();
        let output = organize_video_folder(&batch, &video, 2).await.unwrap();
        assert_eq!(output, batch.join("02_abc123/video_abc123.mp4"));
        assert!(batch.join("02_abc123/thumbnail_abc123.png").is_file());
        assert!(!source.exists());
        let external = root.join("video_external.mp4");
        tokio::fs::write(&external, b"preserve").await.unwrap();
        assert!(organize_video_folder(&batch, &external, 3).await.is_err());
        assert_eq!(tokio::fs::read(external).await.unwrap(), b"preserve");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn summary_distinguishes_failed_and_unprocessed_items_after_stop() {
        let handle = BatchHandle::new(4);
        handle.update(|state| {
            state.status = BatchStatus::Stopped;
            state.finished = vec![
                BatchResult {
                    output: Some("01_abc/video_abc.mp4".into()),
                    error: None,
                    warning: Some("thumbnail warning".into()),
                    duration_seconds: 42.5,
                },
                BatchResult {
                    output: None,
                    error: Some("source missing".into()),
                    warning: None,
                    duration_seconds: 0.0,
                },
            ];
        });
        let summary = format_summary(&handle.snapshot());
        for expected in [
            "Status: Dihentikan",
            "Berhasil: 1",
            "Gagal: 1",
            "Belum diproses: 2",
            "42.500 detik",
            "source missing",
            "thumbnail warning",
            "video_abc.mp4",
        ] {
            assert!(summary.contains(expected), "missing {expected}");
        }
    }
}
