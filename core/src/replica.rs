#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    Apply,
    Ignore,
    NeedSnapshot,
    WrongEpoch,
}

#[derive(Debug)]
pub struct SequenceGate {
    epoch: String,
    last: Option<u64>,
    recovering: bool,
}

impl SequenceGate {
    pub fn new(epoch: String) -> Self {
        Self {
            epoch,
            last: None,
            recovering: true,
        }
    }

    /// Only call after the application has authenticated and validated a full snapshot.
    pub fn snapshot(&mut self, epoch: &str, sequence: u64) -> Delivery {
        if epoch != self.epoch {
            return Delivery::WrongEpoch;
        }
        if self.last.is_some_and(|last| sequence < last) {
            return Delivery::Ignore;
        }
        self.last = Some(sequence);
        self.recovering = false;
        Delivery::Apply
    }

    pub fn event(&mut self, epoch: &str, sequence: u64) -> Delivery {
        if epoch != self.epoch {
            return Delivery::WrongEpoch;
        }
        if self.last.is_some_and(|last| sequence <= last) {
            return Delivery::Ignore;
        }
        if !self.recovering && self.last.and_then(|v| v.checked_add(1)) == Some(sequence) {
            self.last = Some(sequence);
            return Delivery::Apply;
        }
        self.recovering = true;
        Delivery::NeedSnapshot
    }

    pub fn disconnected(&mut self) {
        self.recovering = true;
    }
    pub fn last_sequence(&self) -> Option<u64> {
        self.last
    }
}
