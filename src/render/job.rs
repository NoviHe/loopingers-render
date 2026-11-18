use std::{
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Instant,
};

use crate::util::random_id;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Queued,
    Preparing,
    Rendering,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderStage {
    Queued,
    Preparing,
    Audio,
    Video,
    Finalizing,
    Completed,
    Failed,
}

impl RenderStage {
    pub fn label(self) -> &'static str {
        match self {
            Self::Queued => "Menunggu",
            Self::Preparing => "Menyiapkan render",
            Self::Audio => "Menyiapkan audio",
            Self::Video => "Memproses video",
            Self::Finalizing => "Menyimpan hasil",
            Self::Completed => "Selesai",
            Self::Failed => "Gagal",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RenderTimings {
    pub preparation_seconds: f64,
    pub audio_seconds: f64,
    pub video_seconds: f64,
    pub finalizing_seconds: f64,
    pub total_elapsed_seconds: f64,
}

#[derive(Debug, Clone)]
pub struct RenderJob {
    pub status: JobStatus,
    pub progress_percent: f64,
    pub current_time_sec: f64,
    pub total_time_sec: f64,
    pub speed: String,
    pub fps: f64,
    pub eta_seconds: f64,
    pub encoder_name: String,
    pub error_message: String,
    pub output_file: Option<PathBuf>,
    pub metadata_comment: Option<String>,
    pub visual_label: Option<String>,
    pub stage: RenderStage,
    pub timings: RenderTimings,
    pub report_warning: Option<String>,
    started_at: Option<Instant>,
    stage_started_at: Option<Instant>,
}

impl RenderJob {
    fn new() -> Self {
        Self {
            status: JobStatus::Queued,
            progress_percent: 0.0,
            current_time_sec: 0.0,
            total_time_sec: 0.0,
            speed: "0x".to_owned(),
            fps: 0.0,
            eta_seconds: 0.0,
            encoder_name: String::new(),
            error_message: String::new(),
            output_file: None,
            metadata_comment: None,
            visual_label: None,
            stage: RenderStage::Queued,
            timings: RenderTimings::default(),
            report_warning: None,
            started_at: None,
            stage_started_at: None,
        }
    }

    pub fn fail(&mut self, message: String) {
        self.set_stage(RenderStage::Failed);
        self.status = JobStatus::Failed;
        self.error_message = message;
    }

    fn record_elapsed(&mut self, now: Instant) {
        if let Some(started_at) = self.started_at {
            self.timings.total_elapsed_seconds = now.duration_since(started_at).as_secs_f64();
        }
        if let Some(started_at) = self.stage_started_at.replace(now) {
            let elapsed = now.duration_since(started_at).as_secs_f64();
            match self.stage {
                RenderStage::Preparing => self.timings.preparation_seconds += elapsed,
                RenderStage::Audio => self.timings.audio_seconds += elapsed,
                RenderStage::Video => self.timings.video_seconds += elapsed,
                RenderStage::Finalizing => self.timings.finalizing_seconds += elapsed,
                _ => {}
            }
        }
    }

    fn set_stage(&mut self, stage: RenderStage) {
        self.set_stage_at(stage, Instant::now());
    }

    fn set_stage_at(&mut self, stage: RenderStage, now: Instant) {
        self.record_elapsed(now);
        self.stage = stage;
        if matches!(stage, RenderStage::Completed | RenderStage::Failed) {
            self.started_at = None;
            self.stage_started_at = None;
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(
            self.status,
            JobStatus::Queued | JobStatus::Preparing | JobStatus::Rendering
        )
    }
}

/// Shared view of one render: the worker updates it, the UI takes snapshots.
#[derive(Clone)]
pub struct JobHandle {
    id: String,
    job: Arc<Mutex<RenderJob>>,
}

impl JobHandle {
    pub fn new() -> Self {
        Self {
            id: random_id(10),
            job: Arc::new(Mutex::new(RenderJob::new())),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    fn lock(&self) -> MutexGuard<'_, RenderJob> {
        self.job.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn update(&self, change: impl FnOnce(&mut RenderJob)) {
        change(&mut self.lock());
    }

    pub fn snapshot(&self) -> RenderJob {
        let mut snapshot = self.lock().clone();
        if snapshot.started_at.is_some() {
            snapshot.record_elapsed(Instant::now());
        }
        snapshot
    }

    pub(crate) fn start_timing(&self) {
        self.update(|job| {
            let now = Instant::now();
            job.timings = RenderTimings::default();
            job.started_at = Some(now);
            job.stage_started_at = Some(now);
            job.stage = RenderStage::Preparing;
        });
    }

    pub(crate) fn set_stage(&self, stage: RenderStage) {
        self.update(|job| job.set_stage(stage));
    }
}

impl Default for JobHandle {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_timings_include_preparation_and_freeze_on_failure() {
        use std::time::Duration;
        let start = Instant::now();
        let mut job = RenderJob::new();
        job.started_at = Some(start);
        job.stage_started_at = Some(start);
        job.stage = RenderStage::Preparing;
        job.set_stage_at(RenderStage::Audio, start + Duration::from_secs(1));
        job.set_stage_at(RenderStage::Video, start + Duration::from_secs(4));
        job.set_stage_at(RenderStage::Failed, start + Duration::from_secs(9));
        assert_eq!(job.timings.preparation_seconds, 1.0);
        assert_eq!(job.timings.audio_seconds, 3.0);
        assert_eq!(job.timings.video_seconds, 5.0);
        assert_eq!(job.timings.total_elapsed_seconds, 9.0);
        assert!(job.started_at.is_none());
        assert!(job.stage_started_at.is_none());
        assert_eq!(job.stage.label(), "Gagal");
    }

    #[test]
    fn updates_are_visible_in_snapshots() {
        let handle = JobHandle::new();
        assert!(handle.snapshot().is_running());
        handle.update(|job| {
            job.status = JobStatus::Completed;
            job.progress_percent = 100.0;
        });
        let snapshot = handle.snapshot();
        assert_eq!(snapshot.status, JobStatus::Completed);
        assert!(!snapshot.is_running());
        assert_eq!(handle.id().len(), 10);
    }
}
