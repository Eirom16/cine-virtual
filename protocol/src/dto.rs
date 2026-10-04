use cine_core::{
    media::{ContentIdentity, MediaDescriptor, SourceType},
    playback::{PlaybackStatus, Timeline},
};
use cine_rooms::model::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MAX_MESSAGE_BYTES: usize = 65_536;
pub const MAX_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Serialize, Deserialize)]
pub struct WireMessage {
    pub protocol_version: u32,
    pub event_id: Uuid,
    #[serde(rename = "type")]
    pub kind: String,
    pub room_id: Option<Uuid>,
    pub room_epoch: Option<Uuid>,
    pub sender_id: Option<String>,
    pub sequence: Option<u64>,
    pub sent_at_ms: u64,
    pub payload: serde_json::Value,
}
impl WireMessage {
    pub fn server(kind: &str, payload: serde_json::Value, now: u64) -> Self {
        Self {
            protocol_version: 1,
            event_id: Uuid::new_v4(),
            kind: kind.into(),
            room_id: None,
            room_epoch: None,
            sender_id: Some("server".into()),
            sequence: None,
            sent_at_ms: now,
            payload,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct IdentityDto {
    pub algorithm: String,
    pub digest: String,
    pub size_bytes: u64,
}
impl IdentityDto {
    pub fn validate(&self) -> Result<ContentIdentity, ErrorCode> {
        if self.algorithm != "sha256"
            || self.digest.len() != 64
            || !self
                .digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || self.size_bytes == 0
            || self.size_bytes > 1_099_511_627_776
        {
            return Err(ErrorCode::InvalidMedia);
        }
        let mut bytes = [0; 32];
        for (i, value) in bytes.iter_mut().enumerate() {
            *value = u8::from_str_radix(&self.digest[i * 2..i * 2 + 2], 16)
                .map_err(|_| ErrorCode::InvalidMedia)?;
        }
        Ok(ContentIdentity {
            size_bytes: self.size_bytes,
            sha256: bytes,
        })
    }
}
impl From<&ContentIdentity> for IdentityDto {
    fn from(i: &ContentIdentity) -> Self {
        Self {
            algorithm: "sha256".into(),
            digest: i.sha256.iter().map(|b| format!("{b:02x}")).collect(),
            size_bytes: i.size_bytes,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaDto {
    pub media_id: Uuid,
    pub source_type: String,
    pub title: Option<String>,
    pub duration_ms: u64,
    pub identity: IdentityDto,
    pub mime: Option<String>,
    pub codecs: Vec<String>,
}
impl MediaDto {
    pub fn validate(&self) -> Result<MediaDescriptor, ErrorCode> {
        if self.source_type != "local_file" {
            return Err(ErrorCode::UnsupportedSource);
        }
        let descriptor = MediaDescriptor {
            media_id: self.media_id.to_string(),
            source_type: SourceType::LocalFile,
            title: self.title.clone(),
            duration_ms: self.duration_ms,
            identity: self.identity.validate()?,
            mime: self.mime.clone(),
            codecs: self.codecs.clone(),
        };
        if !descriptor.valid_local() {
            return Err(ErrorCode::InvalidMedia);
        }
        Ok(descriptor)
    }
}
impl From<&MediaDescriptor> for MediaDto {
    fn from(m: &MediaDescriptor) -> Self {
        Self {
            media_id: Uuid::parse_str(&m.media_id).expect("validated domain media ID"),
            source_type: "local_file".into(),
            title: m.title.clone(),
            duration_ms: m.duration_ms,
            identity: (&m.identity).into(),
            mime: m.mime.clone(),
            codecs: m.codecs.clone(),
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusDto {
    Playing,
    Paused,
    Stopped,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct TimelineDto {
    pub status: StatusDto,
    pub position_ms: u64,
    pub anchor_time_ms: u64,
    pub rate: f64,
    pub duration_ms: u64,
}
impl TimelineDto {
    pub fn validate(&self) -> Result<Timeline, ErrorCode> {
        let timeline = Timeline {
            status: match self.status {
                StatusDto::Playing => PlaybackStatus::Playing,
                StatusDto::Paused => PlaybackStatus::Paused,
                StatusDto::Stopped => PlaybackStatus::Stopped,
            },
            position_ms: self.position_ms,
            anchor_time_ms: self.anchor_time_ms as i64,
            rate: self.rate,
            duration_ms: self.duration_ms,
        };
        if !timeline.valid() || self.rate != 1.0 || self.duration_ms > 604_800_000 {
            return Err(ErrorCode::InvalidEvent);
        }
        Ok(timeline)
    }
}
impl From<Timeline> for TimelineDto {
    fn from(t: Timeline) -> Self {
        Self {
            status: match t.status {
                PlaybackStatus::Playing => StatusDto::Playing,
                PlaybackStatus::Paused => StatusDto::Paused,
                PlaybackStatus::Stopped => StatusDto::Stopped,
            },
            position_ms: t.position_ms,
            anchor_time_ms: t.anchor_time_ms as u64,
            rate: t.rate,
            duration_ms: t.duration_ms,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoleDto {
    Host,
    Participant,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberStatusDto {
    Idle,
    Loading,
    Ready,
    Buffering,
    Error,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct MemberDto {
    pub member_id: Uuid,
    pub display_name: String,
    pub role: RoleDto,
    pub connected: bool,
    pub ready: bool,
    pub verified_media_revision: Option<u64>,
    pub status: MemberStatusDto,
    pub joined_at_ms: u64,
    pub lease_expires_at_ms: Option<u64>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PermissionsDto {
    pub profile: String,
    pub host_permissions: Vec<String>,
    pub participant_permissions: Vec<String>,
}
pub const HOST_PERMISSIONS: [&str; 6] = [
    "play",
    "pause",
    "seek",
    "change_media",
    "kick_member",
    "manage_permissions",
];
#[derive(Clone, Serialize, Deserialize)]
pub struct SelectionDto {
    pub media_revision: u64,
    pub descriptor: MediaDto,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PendingDto {
    pub command_event_id: Option<Uuid>,
    pub sequence: u64,
    pub authority_revision: u64,
    pub media_revision: u64,
    pub execute_at_ms: u64,
    pub timeline_after: TimelineDto,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PlaybackDto {
    pub current: TimelineDto,
    pub pending: Option<PendingDto>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct StateDto {
    pub room_id: Uuid,
    pub room_epoch: Uuid,
    pub sequence: u64,
    pub updated_at_ms: u64,
    pub host_id: Uuid,
    pub authority_revision: u64,
    pub permissions: PermissionsDto,
    pub members: Vec<MemberDto>,
    pub media: Option<SelectionDto>,
    pub playback: Option<PlaybackDto>,
}
impl From<&RoomState> for StateDto {
    fn from(s: &RoomState) -> Self {
        Self {
            room_id: s.room_id,
            room_epoch: s.room_epoch,
            sequence: s.sequence,
            updated_at_ms: s.updated_at_ms,
            host_id: s.host_id,
            authority_revision: s.authority_revision,
            permissions: PermissionsDto {
                profile: "host_only_v1".into(),
                host_permissions: HOST_PERMISSIONS.iter().map(|s| s.to_string()).collect(),
                participant_permissions: vec![],
            },
            members: s
                .members
                .iter()
                .map(|m| MemberDto {
                    member_id: m.member_id,
                    display_name: m.display_name.clone(),
                    role: if m.role == Role::Host {
                        RoleDto::Host
                    } else {
                        RoleDto::Participant
                    },
                    connected: m.connected,
                    ready: m.ready,
                    verified_media_revision: m.verified_media_revision,
                    status: match m.status {
                        MemberStatus::Idle => MemberStatusDto::Idle,
                        MemberStatus::Loading => MemberStatusDto::Loading,
                        MemberStatus::Ready => MemberStatusDto::Ready,
                        MemberStatus::Buffering => MemberStatusDto::Buffering,
                        MemberStatus::Error => MemberStatusDto::Error,
                    },
                    joined_at_ms: m.joined_at_ms,
                    lease_expires_at_ms: m.lease_expires_at_ms,
                })
                .collect(),
            media: s.media.as_ref().map(|m| SelectionDto {
                media_revision: m.media_revision,
                descriptor: (&m.descriptor).into(),
            }),
            playback: s.playback.as_ref().map(|p| PlaybackDto {
                current: p.current.into(),
                pending: p.pending.as_ref().map(|t| PendingDto {
                    command_event_id: t.command_event_id,
                    sequence: t.sequence,
                    authority_revision: t.authority_revision,
                    media_revision: t.media_revision,
                    execute_at_ms: t.execute_at_ms,
                    timeline_after: t.timeline_after.into(),
                }),
            }),
        }
    }
}
impl StateDto {
    pub fn validate(&self) -> Result<RoomState, ErrorCode> {
        let invalid = ErrorCode::InvalidEvent;
        if self.sequence == 0
            || self.authority_revision == 0
            || self.members.is_empty()
            || self.members.len() > 16
            || self.permissions.profile != "host_only_v1"
            || !self.permissions.participant_permissions.is_empty()
            || self
                .permissions
                .host_permissions
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                != HOST_PERMISSIONS
        {
            return Err(invalid);
        }
        let media = self
            .media
            .as_ref()
            .map(|m| {
                Ok(MediaSelection {
                    media_revision: m.media_revision,
                    descriptor: m.descriptor.validate()?,
                })
            })
            .transpose()?;
        if media.as_ref().is_some_and(|m| m.media_revision == 0)
            || self.media.is_some() != self.playback.is_some()
        {
            return Err(invalid);
        }
        let mut seen = std::collections::HashSet::new();
        let mut members = vec![];
        for m in &self.members {
            if m.display_name.is_empty()
                || m.display_name.len() > 64
                || !seen.insert(m.member_id)
                || (matches!(m.role, RoleDto::Host) != (m.member_id == self.host_id))
                || (!m.connected && m.ready)
                || (m.connected && m.lease_expires_at_ms.is_some())
                || m.verified_media_revision
                    .is_some_and(|r| media.as_ref().is_none_or(|sel| sel.media_revision != r))
                || (m.ready
                    && (!matches!(m.status, MemberStatusDto::Ready)
                        || media.as_ref().is_none_or(|sel| {
                            m.verified_media_revision != Some(sel.media_revision)
                        })))
            {
                return Err(invalid);
            }
            members.push(Member {
                member_id: m.member_id,
                display_name: m.display_name.clone(),
                role: if matches!(m.role, RoleDto::Host) {
                    Role::Host
                } else {
                    Role::Participant
                },
                connected: m.connected,
                ready: m.ready,
                verified_media_revision: m.verified_media_revision,
                status: match m.status {
                    MemberStatusDto::Idle => MemberStatus::Idle,
                    MemberStatusDto::Loading => MemberStatus::Loading,
                    MemberStatusDto::Ready => MemberStatus::Ready,
                    MemberStatusDto::Buffering => MemberStatus::Buffering,
                    MemberStatusDto::Error => MemberStatus::Error,
                },
                joined_at_ms: m.joined_at_ms,
                lease_expires_at_ms: m.lease_expires_at_ms,
            });
        }
        if !seen.contains(&self.host_id) {
            return Err(invalid);
        }
        let playback = self
            .playback
            .as_ref()
            .map(|p| {
                let current = p.current.validate()?;
                if media
                    .as_ref()
                    .is_none_or(|m| m.descriptor.duration_ms != current.duration_ms)
                {
                    return Err(invalid);
                }
                let pending = p
                    .pending
                    .as_ref()
                    .map(|t| {
                        let timeline = t.timeline_after.validate()?;
                        if t.execute_at_ms != t.timeline_after.anchor_time_ms
                            || t.sequence > self.sequence
                            || t.sequence == 0
                            || t.authority_revision != self.authority_revision
                            || t.media_revision != media.as_ref().unwrap().media_revision
                            || timeline.duration_ms != current.duration_ms
                        {
                            return Err(invalid);
                        }
                        Ok(ScheduledTransition {
                            command_event_id: t.command_event_id,
                            sequence: t.sequence,
                            authority_revision: t.authority_revision,
                            media_revision: t.media_revision,
                            execute_at_ms: t.execute_at_ms,
                            timeline_after: timeline,
                        })
                    })
                    .transpose()?;
                Ok(Playback { current, pending })
            })
            .transpose()?;
        Ok(RoomState {
            room_id: self.room_id,
            room_epoch: self.room_epoch,
            sequence: self.sequence,
            updated_at_ms: self.updated_at_ms,
            host_id: self.host_id,
            authority_revision: self.authority_revision,
            members,
            media,
            playback,
        })
    }
}
