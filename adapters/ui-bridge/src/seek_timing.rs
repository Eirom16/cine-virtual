//! Adapter-local model of time lost while the SDK seeks. No Core thresholds.
use std::collections::VecDeque;

#[derive(Default)]
pub(crate) struct SeekTiming {
    losses: VecDeque<u64>,
}
impl SeekTiming {
    pub fn observe(&mut self, loss_ms: u64) {
        if self.losses.len() == 3 {
            self.losses.pop_front();
        }
        self.losses.push_back(loss_ms);
    }
    pub fn estimate(&self) -> u64 {
        let mut values: Vec<_> = self.losses.iter().copied().collect();
        values.sort_unstable();
        values.get(values.len() / 2).copied().unwrap_or(0)
    }
}

pub(crate) fn project_target(
    position: u64,
    sampled_at: u64,
    dispatch_at: u64,
    loss_ms: u64,
    playing: bool,
    duration: u64,
) -> u64 {
    position
        .saturating_add(if playing {
            dispatch_at
                .saturating_sub(sampled_at)
                .saturating_add(loss_ms)
        } else {
            0
        })
        .min(duration)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cine_core::sync::{Correction, Observation, SyncConfig, SyncEngine};

    #[test]
    fn slow_seek_compensates_stopped_timeline_without_changing_sync_policy() {
        let mut timing = SeekTiming::default();
        timing.observe(400);
        let mut old = SyncEngine::new(SyncConfig::default()).unwrap();
        let mut corrected = SyncEngine::new(SyncConfig::default()).unwrap();
        let mut requested = 0;
        for now in [2000, 2500, 3000] {
            let sample = Observation {
                target_ms: 10000 + now,
                actual_ms: 10000 + now - 400,
                now_ms: now,
                playing: true,
                buffering: false,
                clock_trusted: true,
                supports_rate: true,
                nominal_rate: 1.0,
            };
            let before = old.observe(sample);
            let after = corrected.observe(sample);
            assert_eq!(after, before);
            if now == 3000 {
                let Correction::Seek(target) = before else {
                    panic!("initial real drift must still request a hard seek")
                };
                requested = target;
            }
        }
        // Dispatch is 200 ms late; SDK then stops advancing for 400 ms.
        let compensated = project_target(requested, 3000, 3200, timing.estimate(), true, 30000);
        assert_eq!(compensated, 13600);
        for now in [4000, 4500, 5000] {
            let sample = Observation {
                target_ms: 10000 + now,
                actual_ms: requested + now - 3600,
                now_ms: now,
                playing: true,
                buffering: false,
                clock_trusted: true,
                supports_rate: true,
                nominal_rate: 1.0,
            };
            let before = old.observe(sample);
            assert_eq!(
                corrected.observe(Observation {
                    actual_ms: compensated + now - 3600,
                    ..sample
                }),
                Correction::None
            );
            if now == 5000 {
                assert!(matches!(before, Correction::Seek(_)));
            }
        }
    }
    #[test]
    fn seek_loss_uses_recent_observations_and_reset_has_no_assumed_delay() {
        let mut timing = SeekTiming::default();
        assert_eq!(timing.estimate(), 0);
        for loss in [400, 420, 900, 380] {
            timing.observe(loss);
        }
        assert_eq!(timing.estimate(), 420);
        assert_eq!(SeekTiming::default().estimate(), 0);
    }
    #[test]
    fn sample_projection_accounts_for_queue_age_and_keeps_paused_target_exact() {
        assert_eq!(project_target(10000, 1000, 1300, 400, true, 30000), 10700);
        assert_eq!(project_target(10000, 1000, 1300, 400, false, 30000), 10000);
        assert_eq!(project_target(29900, 1000, 1300, 400, true, 30000), 30000);
    }
}
