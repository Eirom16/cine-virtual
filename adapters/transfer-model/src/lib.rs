//! Pure, bounded file-transfer control types; no sockets, filesystem, Player or runtime.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;
pub const CHUNK_SIZE: u32 = 1_048_576;
pub const MAX_SIZE: u64 = 16 * 1024 * 1024 * 1024;
pub const MAX_CONTROL: usize = 4096;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Space,
    Manifest,
    Bounds,
    Corrupt,
    Identity,
    Unauthorized,
    Expired,
    Replay,
    Cancelled,
    Paused,
    Io,
    Tls,
    Storage,
    Modified,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TRANSFER_{:?}", self)
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(_: std::io::Error) -> Self {
        Self::Io
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: u32,
    pub transfer_id: Uuid,
    pub room_id: Uuid,
    pub room_epoch: Uuid,
    pub host_id: Uuid,
    pub authority_revision: u64,
    pub media_revision: u64,
    pub media_id: Uuid,
    pub display_name: String,
    pub size_bytes: u64,
    pub chunk_size: u32,
    pub chunk_count: u32,
    pub sha256: [u8; 32],
    pub duration_ms: u64,
    pub block_hash: String,
}
impl Manifest {
    pub fn validate(&self) -> Result<(), Error> {
        if self.version != 1
            || [
                self.transfer_id,
                self.room_id,
                self.room_epoch,
                self.host_id,
                self.media_id,
            ]
            .iter()
            .any(|id| id.get_version_num() != 4 || id.get_variant() != uuid::Variant::RFC4122)
            || self.authority_revision == 0
            || self.media_revision == 0
            || self.size_bytes == 0
            || self.size_bytes > MAX_SIZE
            || self.chunk_size != CHUNK_SIZE
            || u64::from(self.chunk_count) != self.size_bytes.div_ceil(u64::from(self.chunk_size))
            || self.display_name.is_empty()
            || self.display_name.len() > 128
            || self
                .display_name
                .chars()
                .any(|c| c.is_control() || "/\\:".contains(c))
            || self.display_name == "."
            || self.display_name == ".."
            || self.duration_ms == 0
            || self.duration_ms > 604_800_000
            || self.block_hash != "sha256"
        {
            return Err(Error::Manifest);
        }
        Ok(())
    }
    pub fn chunk(&self, index: u32) -> Result<(u64, usize), Error> {
        self.validate()?;
        if index >= self.chunk_count {
            return Err(Error::Bounds);
        }
        let offset = u64::from(index) * u64::from(self.chunk_size);
        Ok((
            offset,
            (self.size_bytes - offset).min(u64::from(self.chunk_size)) as usize,
        ))
    }
    pub fn identity(&self) -> cine_core::media::ContentIdentity {
        cine_core::media::ContentIdentity {
            size_bytes: self.size_bytes,
            sha256: self.sha256,
        }
    }
    pub fn fingerprint(&self) -> [u8; 32] {
        Sha256::digest(serde_json::to_vec(self).expect("manifest serializes")).into()
    }
}
// Secrets deliberately do not implement Debug.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Credential {
    pub grant_id: Uuid,
    pub transfer_id: Uuid,
    pub room_id: Uuid,
    pub room_epoch: Uuid,
    pub receiver_id: Uuid,
    pub media_revision: u64,
    pub authority_revision: u64,
    pub manifest_hash: [u8; 32],
    pub secret: [u8; 32],
}
impl Credential {
    pub fn new(manifest: &Manifest, receiver_id: Uuid) -> Result<Self, Error> {
        manifest.validate()?;
        let mut secret = [0; 32];
        getrandom::fill(&mut secret).map_err(|_| Error::Unauthorized)?;
        Ok(Self {
            grant_id: Uuid::new_v4(),
            transfer_id: manifest.transfer_id,
            room_id: manifest.room_id,
            room_epoch: manifest.room_epoch,
            receiver_id,
            media_revision: manifest.media_revision,
            authority_revision: manifest.authority_revision,
            manifest_hash: manifest.fingerprint(),
            secret,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum TransferIntent {
    Offer {
        manifest: Box<Manifest>,
        address: std::net::SocketAddr,
        certificate: Vec<u8>,
    },
    Request {
        transfer_id: Uuid,
    },
    Accept {
        transfer_id: Uuid,
        receiver_id: Uuid,
    },
    Reject {
        transfer_id: Uuid,
        receiver_id: Uuid,
    },
    Cancel {
        transfer_id: Uuid,
    },
    Withdraw {
        transfer_id: Uuid,
    },
    Status {
        transfer_id: Uuid,
        state: String,
        verified_bytes: u64,
    },
}
impl TransferIntent {
    pub fn validate(&self) -> Result<(), Error> {
        match self {
            Self::Offer {
                manifest,
                address,
                certificate,
            } => {
                manifest.validate()?;
                if certificate.is_empty()
                    || certificate.len() > 2048
                    || address.port() == 0
                    || !matches!(address.ip(),std::net::IpAddr::V4(ip) if ip.is_private() || ip.is_loopback())
                {
                    return Err(Error::Manifest);
                }
            }
            Self::Status { state, .. }
                if ![
                    "paused",
                    "reconnecting",
                    "transferring",
                    "verifying",
                    "completed",
                    "failed",
                ]
                .contains(&state.as_str()) =>
            {
                return Err(Error::Manifest);
            }
            _ => {}
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Offer {
    pub manifest: Manifest,
    pub address: std::net::SocketAddr,
    pub certificate: Vec<u8>,
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Receiver {
    pub authorized: bool,
    pub receiver_id: Uuid,
    pub state: String,
    pub verified_bytes: u64,
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TransferSnapshot {
    pub offer: Option<Offer>,
    pub receivers: Vec<Receiver>,
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub credential: Credential,
    pub expires_at_ms: u64,
}
// Public pin encoding; never use it for token logging or credential encoding.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn unhex(text: &str) -> Result<Vec<u8>, Error> {
    if text.len() > 4096 || !text.len().is_multiple_of(2) {
        return Err(Error::Tls);
    }
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| {
            let s = std::str::from_utf8(c).map_err(|_| Error::Tls)?;
            u8::from_str_radix(s, 16).map_err(|_| Error::Tls)
        })
        .collect()
}
