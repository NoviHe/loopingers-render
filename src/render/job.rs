use std::{
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
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
        }
    }

    pub fn fail(&mut self, message: String) {
        self.status = JobStatus::Failed;
        self.error_message = message;
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
        self.lock().clone()
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
