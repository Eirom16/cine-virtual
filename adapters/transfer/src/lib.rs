//! Bounded blocking transfer workers. Never run these functions on UI/control threads.
pub mod storage;
pub mod tls;
pub mod transport;
pub use cine_transfer_model::*;
use serde::Serialize;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
use subtle::ConstantTimeEq;

pub struct Authorization {
    credential: Credential,
    deadline: Instant,
    consumed: AtomicBool,
    pub revoked: Arc<AtomicBool>,
}
impl Authorization {
    pub fn new(credential: Credential, ttl: std::time::Duration) -> Self {
        Self {
            credential,
            deadline: Instant::now() + ttl,
            consumed: AtomicBool::new(false),
            revoked: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn check(&self) -> Result<(), Error> {
        if self.revoked.load(Ordering::Acquire) {
            return Err(Error::Unauthorized);
        }
        if Instant::now() >= self.deadline {
            return Err(Error::Expired);
        }
        Ok(())
    }
    pub fn consume(&self, candidate: &Credential) -> Result<(), Error> {
        self.check()?;
        if candidate.grant_id != self.credential.grant_id
            || candidate.transfer_id != self.credential.transfer_id
            || candidate.room_id != self.credential.room_id
            || candidate.room_epoch != self.credential.room_epoch
            || candidate.receiver_id != self.credential.receiver_id
            || candidate.media_revision != self.credential.media_revision
            || candidate.authority_revision != self.credential.authority_revision
            || candidate.manifest_hash != self.credential.manifest_hash
        {
            return Err(Error::Unauthorized);
        }
        if candidate.secret.ct_eq(&self.credential.secret).unwrap_u8() != 1 {
            return Err(Error::Unauthorized);
        }
        if self.consumed.swap(true, Ordering::AcqRel) {
            return Err(Error::Replay);
        }
        Ok(())
    }
}
#[derive(Clone, Default)]
pub struct Control {
    pub deadline: Arc<std::sync::Mutex<Option<Instant>>>,
    pub pause: Arc<AtomicBool>,
    pub cancel: Arc<AtomicBool>,
}
impl Control {
    pub fn check(&self) -> Result<(), Error> {
        if self.cancel.load(Ordering::Acquire) {
            Err(Error::Cancelled)
        } else if self.pause.load(Ordering::Acquire) {
            Err(Error::Paused)
        } else if self
            .deadline
            .lock()
            .unwrap()
            .is_some_and(|d| Instant::now() >= d)
        {
            Err(Error::Expired)
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Debug, Serialize, Default)]
pub struct Progress {
    pub state: String,
    pub received_bytes: u64,
    pub verified_bytes: u64,
    pub total_bytes: u64,
    pub bytes_per_second: Option<f64>,
    pub eta_seconds: Option<f64>,
    pub error: Option<String>,
}
