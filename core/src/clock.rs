use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClockEstimate {
    pub offset_ms: f64,
    pub rtt_ms: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct ClockSample {
    pub t1: i64,
    pub t2: i64,
    pub t3: i64,
    pub t4: i64,
}

impl ClockSample {
    pub fn estimate(self) -> Option<ClockEstimate> {
        if self.t4 < self.t1 || self.t3 < self.t2 {
            return None;
        }
        let rtt = (i128::from(self.t4) - i128::from(self.t1))
            - (i128::from(self.t3) - i128::from(self.t2));
        if !(0..=2_000).contains(&rtt) {
            return None;
        }
        let offset = ((i128::from(self.t2) - i128::from(self.t1))
            + (i128::from(self.t3) - i128::from(self.t4))) as f64
            / 2.0;
        Some(ClockEstimate {
            offset_ms: offset,
            rtt_ms: rtt as f64,
        })
    }
}

#[derive(Debug, Default)]
pub struct ClockFilter {
    samples: VecDeque<ClockEstimate>,
}

impl ClockFilter {
    pub fn push(&mut self, sample: ClockSample) -> bool {
        let Some(estimate) = sample.estimate() else {
            return false;
        };
        if self.samples.len() == 8 {
            self.samples.pop_front();
        }
        self.samples.push_back(estimate);
        true
    }

    /// Median offset of the three lowest-RTT samples; no estimate before warmup.
    pub fn estimate(&self) -> Option<ClockEstimate> {
        if self.samples.len() < 3 {
            return None;
        }
        let mut best: Vec<_> = self.samples.iter().copied().collect();
        best.sort_by(|a, b| a.rtt_ms.total_cmp(&b.rtt_ms));
        best.truncate(3);
        let rtt_ms = best[0].rtt_ms;
        best.sort_by(|a, b| a.offset_ms.total_cmp(&b.offset_ms));
        Some(ClockEstimate {
            offset_ms: best[1].offset_ms,
            rtt_ms,
        })
    }
}
