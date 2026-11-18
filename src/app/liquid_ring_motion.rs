//! Audio-driven, damped motion for the continuous liquid ring profile.
use loopkaraoke::spectrum::BAND_COUNT;

pub const PROFILE_LEN: usize = 128;

pub struct LiquidRingMotion {
    pub profile: [f32; PROFILE_LEN],
    velocity: [f32; PROFILE_LEN],
    envelope: f32,
}

impl Default for LiquidRingMotion {
    fn default() -> Self {
        Self {
            profile: [0.0; PROFILE_LEN],
            velocity: [0.0; PROFILE_LEN],
            envelope: 0.35,
        }
    }
}

impl LiquidRingMotion {
    pub fn advance(
        &mut self,
        levels: &[f32; BAND_COUNT],
        sensitivity_pct: u32,
        smoothing_pct: u32,
        dt: f32,
        reset: bool,
    ) -> bool {
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.1)
        } else {
            0.0
        };
        let clean = levels.map(|v| {
            if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.0
            }
        });
        let peak = clean.iter().copied().fold(0.0_f32, f32::max);
        if reset {
            self.envelope = peak.max(0.35);
            self.velocity.fill(0.0);
        } else {
            // Bounded gain: quiet music remains visible without amplifying silence.
            let tau = if peak > self.envelope { 0.06 } else { 1.2 };
            self.envelope += (peak.max(0.35) - self.envelope) * (1.0 - (-dt / tau).exp());
        }
        let gain = (0.8 / self.envelope.max(0.35)).clamp(1.0, 2.0)
            * sensitivity_pct.clamp(25, 300) as f32
            / 100.0;
        let bands = clean.map(|v| (((v - 0.035).max(0.0) * gain).min(1.0)).powf(1.35));
        let bass = bands[..10].iter().sum::<f32>() / 10.0;
        let target: [f32; PROFILE_LEN] = std::array::from_fn(|index| {
            let angle = index as f32 / PROFILE_LEN as f32 * std::f32::consts::TAU;
            // A cosine mapping joins both ends without a sharp frequency seam.
            let position = (1.0 - angle.cos()) * 0.5 * (BAND_COUNT - 1) as f32;
            let mut amplitude = 0.0;
            let mut weight_sum = 0.0;
            for offset in -2_i32..=2 {
                let p = (position + offset as f32).clamp(0.0, (BAND_COUNT - 1) as f32);
                let lo = p.floor() as usize;
                let hi = (lo + 1).min(BAND_COUNT - 1);
                let weight = (3 - offset.abs()) as f32;
                amplitude += (bands[lo] * (1.0 - p.fract()) + bands[hi] * p.fract()) * weight;
                weight_sum += weight;
            }
            bass * 0.018 + amplitude / weight_sum * 0.145
        });
        if reset {
            self.profile = target;
            return false;
        }
        let smoothing = smoothing_pct.min(100) as f32 / 100.0;
        let omega = 42.0 - smoothing * 22.0;
        let damping = 0.82 + smoothing * 0.1;
        // Small bounded substeps keep the spring stable even during slow UI frames.
        let steps = (dt * 240.0).ceil().max(1.0) as usize;
        let h = dt / steps as f32;
        for _ in 0..steps {
            let previous = self.profile;
            for index in 0..PROFILE_LEN {
                let neighbors = (previous[(index + PROFILE_LEN - 1) % PROFILE_LEN]
                    + previous[(index + 1) % PROFILE_LEN])
                    * 0.5;
                let target_neighbors = (target[(index + PROFILE_LEN - 1) % PROFILE_LEN]
                    + target[(index + 1) % PROFILE_LEN])
                    * 0.5;
                let acceleration = omega * omega * (target[index] - previous[index])
                    + 100.0 * ((neighbors - previous[index]) - (target_neighbors - target[index]))
                    - 2.0 * damping * omega * self.velocity[index];
                self.velocity[index] += acceleration * h;
                self.profile[index] = (previous[index] + self.velocity[index] * h).clamp(0.0, 0.19);
            }
        }
        let moving = self
            .profile
            .iter()
            .zip(target)
            .zip(self.velocity)
            .any(|((p, t), v)| (*p - t).abs() > 0.0001 || v.abs() > 0.001);
        if !moving {
            self.profile = target;
            self.velocity.fill(0.0);
        }
        moving
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liquid_motion_is_frame_rate_independent_and_settles_after_audio() {
        let mut slow = LiquidRingMotion::default();
        let mut fast = LiquidRingMotion::default();
        for _ in 0..30 {
            slow.advance(&[0.7; BAND_COUNT], 100, 65, 1.0 / 30.0, false);
        }
        for _ in 0..60 {
            fast.advance(&[0.7; BAND_COUNT], 100, 65, 1.0 / 60.0, false);
        }
        for (a, b) in slow.profile.iter().zip(fast.profile) {
            assert!((*a - b).abs() < 0.002);
        }
        assert!(slow.profile.iter().any(|p| *p > 0.08));
        for _ in 0..180 {
            slow.advance(&[0.0; BAND_COUNT], 100, 65, 1.0 / 60.0, false);
        }
        assert!(slow.profile.iter().all(|p| *p == 0.0));
    }

    #[test]
    fn liquid_motion_stays_bounded_and_reset_discards_previous_song() {
        let mut motion = LiquidRingMotion::default();
        let mut levels = [f32::NAN; BAND_COUNT];
        levels[8] = 1.0;
        for _ in 0..200 {
            motion.advance(&levels, 300, 0, 0.1, false);
        }
        assert!(motion
            .profile
            .iter()
            .all(|v| v.is_finite() && (0.0..=0.19).contains(v)));
        let maximum_step = motion
            .profile
            .iter()
            .enumerate()
            .map(|(i, v)| (*v - motion.profile[(i + 1) % PROFILE_LEN]).abs())
            .fold(0.0_f32, f32::max);
        assert!(maximum_step < 0.06);
        motion.advance(&[0.0; BAND_COUNT], 100, 65, 0.016, true);
        assert_eq!(motion.profile, [0.0; PROFILE_LEN]);
        assert_eq!(motion.velocity, [0.0; PROFILE_LEN]);
    }

    #[test]
    fn paused_audio_settles_without_perpetual_repaints() {
        let mut motion = LiquidRingMotion::default();
        let levels = std::array::from_fn(|i| if i < 12 { 0.8 } else { 0.1 });
        let mut moving = true;
        for _ in 0..240 {
            moving = motion.advance(&levels, 100, 65, 1.0 / 60.0, false);
        }
        assert!(!moving);
    }
}
