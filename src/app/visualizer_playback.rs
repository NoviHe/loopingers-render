use loopkaraoke::karaoke::preview::PreviewSession;

use super::{lyrics::win_audio, Event, StudioApp};

impl StudioApp {
    pub(super) fn visualizer_track_duration(&self) -> f64 {
        self.generate
            .tracks
            .get(self.visualizer_selected_track)
            .map_or(0.0, |track| track.duration.max(0.0))
    }

    pub(super) fn sync_visualizer_selection(&mut self) {
        if self.generate.tracks.is_empty() {
            self.stop_visualizer_playback(true);
            self.visualizer_active_track_id = None;
            self.visualizer_selected_track = 0;
            return;
        }
        if let Some(index) = self
            .generate
            .tracks
            .iter()
            .position(|track| Some(track.id) == self.visualizer_active_track_id)
        {
            self.visualizer_selected_track = index;
        } else {
            self.select_visualizer_track(
                self.visualizer_selected_track
                    .min(self.generate.tracks.len() - 1),
                false,
            );
        }
    }

    pub(super) fn select_visualizer_track(&mut self, index: usize, play: bool) {
        let Some(track) = self.generate.tracks.get(index) else {
            return;
        };
        let track_id = track.id;
        if self.visualizer_active_track_id != Some(track_id) {
            self.stop_visualizer_playback(true);
            self.visualizer_active_track_id = Some(track_id);
        }
        self.visualizer_selected_track = index;
        if play {
            self.toggle_visualizer_playback();
        }
    }

    pub(super) fn stop_visualizer_playback(&mut self, reset: bool) {
        if self.visualizer_playing || self.visualizer_loading_preview.is_some() {
            win_audio::stop();
        }
        self.visualizer_playing = false;
        self.visualizer_loading_preview = None;
        self.visualizer_preview_request = self.visualizer_preview_request.wrapping_add(1);
        if reset {
            self.visualizer_time = 0.0;
        }
    }

    pub(super) fn seek_visualizer(&mut self, time: f64) {
        self.visualizer_time = time.clamp(0.0, self.visualizer_track_duration());
        if self.visualizer_playing {
            if let Some(preview) = self
                .visualizer_active_track_id
                .and_then(|id| self.generate.previews.get(&id))
            {
                win_audio::play_from_with_volume(
                    preview.path(),
                    self.visualizer_time,
                    self.visualizer_volume,
                );
            }
        }
    }

    pub(super) fn toggle_visualizer_playback(&mut self) {
        if self.visualizer_playing || self.visualizer_loading_preview.is_some() {
            self.stop_visualizer_playback(false);
            return;
        }
        let Some(track) = self.generate.tracks.get(self.visualizer_selected_track) else {
            return;
        };
        let track_id = track.id;
        let source = track.path.clone();
        if !self.ffmpeg_ready {
            self.warn("Menunggu FFmpeg siap untuk preview Visualizer.");
            return;
        }
        self.visualizer_active_track_id = Some(track_id);
        self.stop_generate_preview();
        self.lrc_studio.stop_playback();
        if self.visualizer_time >= self.visualizer_track_duration() {
            self.visualizer_time = 0.0;
        }
        if let Some(preview) = self.generate.previews.get(&track_id) {
            if preview.path().is_file() {
                win_audio::play_from_with_volume(
                    preview.path(),
                    self.visualizer_time,
                    self.visualizer_volume,
                );
                self.visualizer_playing = true;
                return;
            }
        }
        self.generate.previews.remove(&track_id);
        if self.audio_preview_session.is_none() {
            match PreviewSession::new(&self.paths.temp) {
                Ok(session) => self.audio_preview_session = Some(session),
                Err(error) => {
                    self.error(format!("Gagal menyiapkan preview Visualizer: {error}"));
                    return;
                }
            }
        }
        let preview = self.audio_preview_session.as_ref().unwrap().create_file();
        let tools = self.tools.clone();
        self.visualizer_preview_request = self.visualizer_preview_request.wrapping_add(1);
        let request_id = self.visualizer_preview_request;
        self.visualizer_loading_preview = Some(track_id);
        self.spawn(async move {
            let mut command = tools.ffmpeg();
            command
                .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
                .arg(&source)
                .args(["-vn", "-ac", "2", "-ar", "44100", "-acodec", "pcm_s16le"])
                .arg(preview.path());
            let result =
                match loopkaraoke::ffmpeg::run(&mut command, "FFmpeg (preview Visualizer)").await {
                    Ok(_) if preview.path().is_file() => Ok(preview),
                    Ok(_) => Err(loopkaraoke::error::Error::internal(
                        "File preview Visualizer tidak terbentuk.",
                    )),
                    Err(error) => Err(error),
                };
            Event::VisualizerPreview {
                track_id,
                request_id,
                result,
            }
        });
    }
}
