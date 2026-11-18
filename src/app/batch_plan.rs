//! Batch planning is independent of the single-video render pipeline.
use std::{
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

use rand::{
    seq::{IteratorRandom, SliceRandom},
    Rng,
};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct BatchSettings {
    pub count: String,
    pub block_name: String,
    pub range_start: String,
    pub range_end: String,
    pub manual: String,
}

impl Default for BatchSettings {
    fn default() -> Self {
        Self {
            count: String::new(),
            block_name: "Acakaja".into(),
            range_start: String::new(),
            range_end: String::new(),
            manual: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct PlanTrack {
    pub path: PathBuf,
    pub title: String,
    pub duration: f64,
}

#[derive(Debug)]
pub(super) struct PlannedVideo {
    pub output_code: String,
    pub visual: PathBuf,
    pub thumbnail: Option<PathBuf>,
    pub duration: u64,
    pub requested_duration: Option<u64>,
    pub order: Vec<usize>,
}

#[derive(Debug)]
pub(super) struct BatchPlan {
    pub block_name: String,
    pub crossfade_secs: f64,
    pub tracks: Vec<PlanTrack>,
    pub videos: Vec<PlannedVideo>,
}

const MAX_DURATION: u64 = 7 * 24 * 3600;

/// Accept M:SS or H:MM:SS, without silently normalizing malformed components.
fn parse_duration(text: &str) -> Result<u64, String> {
    let parts: Vec<_> = text.trim().split(':').collect();
    if !(2..=3).contains(&parts.len())
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.bytes().all(|c| c.is_ascii_digit()))
    {
        return Err(format!(
            "Durasi '{text}' harus berformat MM:SS atau HH:MM:SS."
        ));
    }
    let mut total = 0_u64;
    for (index, part) in parts.iter().enumerate() {
        let value: u64 = part
            .parse()
            .map_err(|_| "Durasi terlalu besar.".to_string())?;
        if index > 0 && value >= 60 {
            return Err(format!(
                "Komponen menit/detik pada '{text}' harus kurang dari 60."
            ));
        }
        total = total
            .checked_mul(60)
            .and_then(|n| n.checked_add(value))
            .ok_or("Durasi terlalu besar.")?;
    }
    if total == 0 || total > MAX_DURATION {
        return Err("Durasi harus lebih dari nol dan maksimal 7 hari.".into());
    }
    Ok(total)
}

pub(super) fn build_plan(
    settings: &BatchSettings,
    visuals: &[PathBuf],
    thumbnails: &[PathBuf],
    tracks: &[PlanTrack],
    rng: &mut impl Rng,
    crossfade_secs: f64,
) -> Result<BatchPlan, String> {
    if visuals.is_empty() {
        return Err("Pilih folder berisi video visual terlebih dahulu.".into());
    }
    if tracks.is_empty() {
        return Err("Tambahkan lagu di Generate terlebih dahulu.".into());
    }
    if tracks
        .iter()
        .any(|t| !t.duration.is_finite() || t.duration < 1.0)
    {
        return Err("Setiap lagu harus memiliki durasi valid minimal satu detik.".into());
    }
    let count = if settings.count.trim().is_empty() {
        visuals.len()
    } else {
        let value = settings.count.trim();
        if !value.bytes().all(|c| c.is_ascii_digit()) {
            return Err("Jumlah video harus bilangan bulat positif.".into());
        }
        value
            .parse::<usize>()
            .map_err(|_| "Jumlah video terlalu besar.".to_string())?
    };
    if count == 0 || count > 10_000 {
        return Err("Jumlah video harus antara 1 dan 10000.".into());
    }
    let name = settings.block_name.trim();
    if name.is_empty() || name.chars().any(char::is_control) {
        return Err("Nama blok daftar lagu wajib diisi tanpa karakter kontrol.".into());
    }
    let manual: Vec<_> = settings
        .manual
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let manual_durations = if manual.is_empty() {
        Vec::new()
    } else {
        if manual.len() != count {
            return Err(format!(
                "Isi tepat {count} durasi manual, ditemukan {}.",
                manual.len()
            ));
        }
        manual
            .iter()
            .map(|line| parse_duration(line))
            .collect::<Result<Vec<_>, _>>()?
    };
    let use_range = manual_durations.is_empty();
    let range = if use_range
        && !(settings.range_start.trim().is_empty() && settings.range_end.trim().is_empty())
    {
        if settings.range_start.trim().is_empty() || settings.range_end.trim().is_empty() {
            return Err("Isi minimum dan maksimum, atau kosongkan keduanya.".into());
        }
        let start = parse_duration(&settings.range_start)?;
        let end = parse_duration(&settings.range_end)?;
        if start > end {
            return Err("Durasi minimum tidak boleh melebihi maksimum.".into());
        }
        Some((start as f64, end as f64))
    } else {
        None
    };
    if tracks.len() < 2 {
        return Err("Tambahkan minimal dua lagu untuk membuat rencana.".into());
    }
    let mut reductions = Vec::new();
    let mut range_fallback = None;
    let mut videos = Vec::with_capacity(count);
    let mut visual_order: Vec<_> = visuals.iter().take(count).collect();
    while visual_order.len() < count {
        let mut round: Vec<_> = visuals.iter().collect();
        round.shuffle(rng);
        if round.len() > 1 && round.first() == visual_order.last() {
            round.swap(0, 1);
        }
        visual_order.extend(round.into_iter().take(count - visual_order.len()));
    }
    for (index, visual) in visual_order.into_iter().enumerate() {
        let mut order: Vec<_> = (0..tracks.len()).collect();
        order.shuffle(rng);
        let requested_duration = manual_durations.get(index).copied();
        let duration = if let Some(target) = requested_duration {
            let min = target.saturating_sub(30) as f64;
            let max = (target + 30).min(MAX_DURATION) as f64;
            let mut nearest = None;
            let (selection, total) = ranged_order(
                tracks, crossfade_secs, min, max, rng, &mut nearest,
            )
            .ok_or_else(|| {
                let suggestion = nearest.map_or_else(String::new, |duration| {
                    format!(" Durasi terdekat yang ditemukan: {}.", timestamp(duration.round() as u64))
                });
                format!(
                    "Video {}: belum ditemukan susunan lagu utuh dalam toleransi ±30 detik dari {}.{suggestion}",
                    index + 1, timestamp(target)
                )
            })?;
            order = selection;
            total.ceil() as u64
        } else if let Some((min, max)) = range {
            let result = ranged_order(tracks, crossfade_secs, min, max, rng, &mut None)
                .or_else(|| range_fallback.clone())
                .ok_or_else(|| {
                    "Belum ditemukan susunan lagu utuh dalam rentang ini. Coba perluas rentang."
                        .to_string()
                })?;
            if range_fallback.is_none() {
                range_fallback = Some(result.clone());
            }
            order = result.0;
            result.1.ceil() as u64
        } else {
            if reductions.is_empty() {
                let limit = match tracks.len() {
                    2..=5 => 2,
                    6..=10 => 4,
                    11..=20 => 6,
                    _ => 8,
                }
                .min(tracks.len() - 2);
                reductions.extend(if limit == 0 { 0..=0 } else { 1..=limit });
                reductions.shuffle(rng);
            }
            order.truncate(tracks.len() - reductions.pop().unwrap_or(0));
            let total = order.iter().fold(0.0, |elapsed, &i| {
                append_duration(elapsed, tracks[i].duration, crossfade_secs)
            });
            if total > MAX_DURATION as f64 {
                return Err("Durasi playlist melebihi batas 7 hari.".into());
            }
            total.ceil() as u64
        };
        let thumbnail = thumbnails
            .iter()
            .find(|thumb| super::normalized_stem(thumb) == super::normalized_stem(visual))
            .cloned();
        videos.push(PlannedVideo {
            output_code: loopkaraoke::util::random_code(6),
            visual: visual.clone(),
            thumbnail,
            duration,
            requested_duration,
            order,
        });
    }
    Ok(BatchPlan {
        block_name: name.into(),
        crossfade_secs,
        tracks: tracks.to_vec(),
        videos,
    })
}

fn append_duration(elapsed: f64, track: f64, crossfade: f64) -> f64 {
    let overlap = crossfade.min(elapsed * 0.45).min(track * 0.45);
    elapsed + track
        - if overlap >= 0.05 {
            (overlap * 1000.0).round() / 1000.0
        } else {
            0.0
        }
}

fn ranged_order(
    tracks: &[PlanTrack],
    crossfade: f64,
    min: f64,
    max: f64,
    rng: &mut impl Rng,
    nearest: &mut Option<f64>,
) -> Option<(Vec<usize>, f64)> {
    let mut seen = std::collections::HashSet::new();
    let mut candidates: Vec<_> = tracks
        .iter()
        .enumerate()
        .filter_map(|(index, track)| seen.insert(&track.path).then_some(index))
        .collect();
    if candidates.len() < 2 {
        return None;
    }
    let target = (min + max) / 2.0;
    let mut consider = |duration: f64| {
        if nearest.is_none_or(|best| (duration - target).abs() < (best - target).abs()) {
            *nearest = Some(duration);
        }
    };
    // Try shuffled selections of whole tracks. Extend a full playlist only
    // when its crossfaded duration falls short of the minimum.
    for _ in 0..64 {
        candidates.shuffle(rng);
        let mut order = Vec::new();
        let mut elapsed = 0.0;
        let full_duration = candidates.iter().fold(0.0, |total, &i| {
            append_duration(total, tracks[i].duration, crossfade)
        });
        for &i in &candidates {
            let next = append_duration(elapsed, tracks[i].duration, crossfade);
            if !order.is_empty() {
                consider(next);
            }
            if next <= max {
                order.push(i);
                elapsed = next;
                if order.len() >= 2 && elapsed >= min {
                    return Some((order, elapsed));
                }
            }
        }
        if full_duration < min {
            while elapsed < min {
                let next = candidates
                    .iter()
                    .copied()
                    .filter(|i| order.last() != Some(i))
                    .filter(|&i| {
                        let next = append_duration(elapsed, tracks[i].duration, crossfade);
                        if !order.is_empty() {
                            consider(next);
                        }
                        next <= max
                    })
                    .choose(rng);
                let Some(i) = next else { break };
                order.push(i);
                elapsed = append_duration(elapsed, tracks[i].duration, crossfade);
            }
            if order.len() >= 2 && elapsed >= min {
                return Some((order, elapsed));
            }
        }
    }
    None
}

impl BatchPlan {
    pub fn filename(&self) -> String {
        format!(
            "acakan_1-{}_{}_lagu.txt",
            self.videos.len(),
            self.tracks.len()
        )
    }

    pub fn text(&self) -> String {
        let mut output = String::new();
        for (index, video) in self.videos.iter().enumerate() {
            let _ = writeln!(
                output,
                "{} {}\nVisual: {}\nThumbnail: {}\nDurasi: {}\nUrutan lagu:",
                self.block_name,
                index + 1,
                video.visual.display(),
                video
                    .thumbnail
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "Belum tersedia".into()),
                timestamp(video.duration)
            );
            let mut elapsed = 0.0;
            for (number, track_index) in video.order.iter().enumerate() {
                let track = &self.tracks[*track_index];
                let end = append_duration(elapsed, track.duration, self.crossfade_secs);
                let start = end - track.duration;
                let title: String = track
                    .title
                    .chars()
                    .map(|c| if c.is_control() { ' ' } else { c })
                    .collect();
                let _ = writeln!(
                    output,
                    "{}. [{} – {}] {}",
                    number + 1,
                    timestamp(start.max(0.0).floor() as u64),
                    timestamp(end.ceil() as u64),
                    title
                );
                elapsed = end;
            }
            output.push('\n');
        }
        output
    }
}

fn timestamp(seconds: u64) -> String {
    if seconds >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

pub(super) fn export_plan(plan: &BatchPlan, path: &Path) -> std::io::Result<()> {
    fs::write(path, plan.text())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};

    fn fixture() -> (BatchSettings, Vec<PathBuf>, Vec<PlanTrack>) {
        (
            BatchSettings {
                range_start: "1:00".into(),
                range_end: "2:00".into(),
                ..Default::default()
            },
            vec!["p-1.mp4".into(), "p-2.mp4".into()],
            vec![
                PlanTrack {
                    path: "a.mp3".into(),
                    title: "A".into(),
                    duration: 30.0,
                },
                PlanTrack {
                    path: "b.mp3".into(),
                    title: "B".into(),
                    duration: 40.0,
                },
            ],
        )
    }

    #[test]
    fn empty_durations_follow_playlist_and_partial_range_is_rejected() {
        let (_, visuals, mut tracks) = fixture();
        tracks[0].duration = 30.5;
        let mut settings = BatchSettings::default();
        let mut rng = StdRng::seed_from_u64(91);
        let plan = build_plan(&settings, &visuals, &[], &tracks, &mut rng, 0.0).unwrap();
        assert!(plan
            .videos
            .iter()
            .all(|video| video.requested_duration.is_none() && video.duration == 71));
        assert_eq!(plan.text().matches(". [").count(), 4);
        settings.range_start = "1:00".into();
        assert!(build_plan(&settings, &visuals, &[], &tracks, &mut rng, 0.0).is_err());
        settings.range_end = "2:00".into();
        let plan = build_plan(&settings, &visuals, &[], &tracks, &mut rng, 0.0).unwrap();
        assert!(plan
            .videos
            .iter()
            .all(|video| video.requested_duration.is_none()));
    }

    #[test]
    fn validates_duration_components_and_overflow() {
        assert_eq!(parse_duration("60:00").unwrap(), 3600);
        assert_eq!(parse_duration("1:02:03").unwrap(), 3723);
        for invalid in [
            "0:00",
            "1:60",
            "1:60:00",
            "-1:00",
            "40",
            "1.5:00",
            "999999999999999999999:00",
            "200:00:00",
        ] {
            assert!(parse_duration(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn manual_overrides_invalid_range_but_must_match_video_count() {
        let (mut settings, visuals, tracks) = fixture();
        settings.range_start = "invalid".into();
        settings.manual = "40:00\n\n45:30\n".into();
        let plan = build_plan(
            &settings,
            &visuals,
            &[],
            &tracks,
            &mut StdRng::seed_from_u64(5),
            0.0,
        )
        .unwrap();
        assert_eq!(
            plan.videos
                .iter()
                .map(|v| v.requested_duration)
                .collect::<Vec<_>>(),
            vec![Some(2400), Some(2730)]
        );
        settings.manual = "40:00".into();
        assert!(build_plan(
            &settings,
            &visuals,
            &[],
            &tracks,
            &mut StdRng::seed_from_u64(5),
            0.0
        )
        .is_err());
    }

    #[test]
    fn validates_counts_range_and_playlist() {
        let (mut settings, visuals, tracks) = fixture();
        for count in ["0", "-1", "10001", "1.5", "9999999999999999999999999"] {
            settings.count = count.into();
            assert!(build_plan(
                &settings,
                &visuals,
                &[],
                &tracks,
                &mut StdRng::seed_from_u64(1),
                0.0
            )
            .is_err());
        }
        settings.count = "1".into();
        settings.range_start = "3:00".into();
        assert!(build_plan(
            &settings,
            &visuals,
            &[],
            &tracks,
            &mut StdRng::seed_from_u64(1),
            0.0
        )
        .is_err());
        settings.range_start = "1:00".into();
        assert!(build_plan(
            &settings,
            &visuals,
            &[],
            &[],
            &mut StdRng::seed_from_u64(1),
            0.0
        )
        .is_err());
    }

    #[test]
    fn explicit_count_limits_media_and_accepts_equal_range_endpoints() {
        let (mut settings, visuals, tracks) = fixture();
        settings.count = " 1 ".into();
        settings.block_name = " Pilihan ".into();
        settings.range_start = "1:10".into();
        settings.range_end = "1:10".into();
        let plan = build_plan(
            &settings,
            &visuals,
            &[],
            &tracks,
            &mut StdRng::seed_from_u64(1),
            0.0,
        )
        .unwrap();
        assert_eq!(plan.videos.len(), 1);
        assert_eq!(plan.videos[0].visual, visuals[0]);
        assert_eq!(plan.videos[0].duration, 70);
        assert!(plan.text().starts_with("Pilihan 1\n"));
        assert_eq!(plan.filename(), "acakan_1-1_2_lagu.txt");

        settings.manual = "40:00\n45:30".into();
        assert!(build_plan(
            &settings,
            &visuals,
            &[],
            &tracks,
            &mut StdRng::seed_from_u64(1),
            0.0,
        )
        .is_err());
    }

    #[test]
    fn rejects_invalid_manual_durations_and_unusable_track_durations() {
        let (mut settings, visuals, mut tracks) = fixture();
        settings.manual = "40:00\n45:60".into();
        assert!(build_plan(
            &settings,
            &visuals,
            &[],
            &tracks,
            &mut StdRng::seed_from_u64(1),
            0.0,
        )
        .is_err());
        settings.manual.clear();
        for duration in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            tracks[0].duration = duration;
            assert!(build_plan(
                &settings,
                &visuals,
                &[],
                &tracks,
                &mut StdRng::seed_from_u64(1),
                0.0,
            )
            .is_err());
        }
    }

    #[test]
    fn shuffles_complete_playlist_matches_media_and_exports_repeatable_text() {
        let (settings, visuals, tracks) = fixture();
        let plan = build_plan(
            &settings,
            &visuals,
            &["P-1.PNG".into()],
            &tracks,
            &mut StdRng::seed_from_u64(4),
            0.0,
        )
        .unwrap();
        assert_eq!(plan.videos.len(), 2);
        assert!(plan.videos[0].thumbnail.is_some());
        assert!(plan.videos[1].thumbnail.is_none());
        for video in &plan.videos {
            assert!((60..=120).contains(&video.duration));
            let mut order = video.order.clone();
            order.sort();
            assert_eq!(order, vec![0, 1]);
        }
        assert_eq!(plan.filename(), "acakan_1-2_2_lagu.txt");
        let path = std::env::temp_dir().join(format!("batch-plan-{}.txt", uuid::Uuid::new_v4()));
        export_plan(&plan, &path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(text, plan.text());
        assert!(text.contains("Acakaja 1\nVisual: p-1.mp4"));
        assert!(text.contains("Acakaja 2\nVisual: p-2.mp4"));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn manual_duration_exports_whole_tracks() {
        let (mut settings, visuals, tracks) = fixture();
        settings.count = "1".into();
        settings.manual = "1:10".into();
        let plan = build_plan(
            &settings,
            &visuals,
            &[],
            &tracks,
            &mut StdRng::seed_from_u64(1),
            0.0,
        )
        .unwrap();
        let text = plan.text();
        assert_eq!(plan.videos[0].duration, 70);
        assert_eq!(plan.videos[0].requested_duration, Some(70));
        assert!(text.contains("2. ["));
        assert!(!text.contains("3. ["));
    }
}
