use cine_core::{
    media::{ContentIdentity, MediaDescriptor},
    playback::Timeline,
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Host,
    Participant,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberStatus {
    Idle,
    Loading,
    Ready,
    Buffering,
    Error,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    pub member_id: Uuid,
    pub display_name: String,
    pub role: Role,
    pub connected: bool,
    pub ready: bool,
    pub verified_media_revision: Option<u64>,
    pub status: MemberStatus,
    pub joined_at_ms: u64,
    pub lease_expires_at_ms: Option<u64>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct MediaSelection {
    pub media_revision: u64,
    pub descriptor: MediaDescriptor,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ScheduledTransition {
    pub command_event_id: Option<Uuid>,
    pub sequence: u64,
    pub authority_revision: u64,
    pub media_revision: u64,
    pub execute_at_ms: u64,
    pub timeline_after: Timeline,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Playback {
    pub current: Timeline,
    pub pending: Option<ScheduledTransition>,
}
impl Playback {
    pub fn timeline_at(&self, now: u64) -> Timeline {
        self.pending
            .as_ref()
            .filter(|p| now >= p.execute_at_ms)
            .map_or(self.current, |p| p.timeline_after)
    }
    pub fn normalize(&mut self, now: u64) {
        if self
            .pending
            .as_ref()
            .is_some_and(|p| now >= p.execute_at_ms)
        {
            self.current = self.pending.take().unwrap().timeline_after;
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct RoomState {
    pub room_id: Uuid,
    pub room_epoch: Uuid,
    pub sequence: u64,
    pub updated_at_ms: u64,
    pub host_id: Uuid,
    pub authority_revision: u64,
    pub members: Vec<Member>,
    pub media: Option<MediaSelection>,
    pub playback: Option<Playback>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdminContext {
    pub expected_sequence: u64,
    pub authority_revision: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlContext {
    pub admin: AdminContext,
    pub media_revision: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotReadyReason {
    Loading,
    Buffering,
    PlayerError,
    User,
}
impl NotReadyReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Loading => "loading",
            Self::Buffering => "buffering",
            Self::PlayerError => "player_error",
            Self::User => "user",
        }
    }
}
#[derive(Clone, PartialEq)]
pub enum Command {
    Create {
        display_name: String,
    },
    Join {
        invite_token: String,
        display_name: String,
    },
    Resume {
        resume_token: String,
        last_sequence: u64,
    },
    Leave,
    Select {
        context: AdminContext,
        previous_media_revision: u64,
        descriptor: MediaDescriptor,
    },
    Metadata {
        media_revision: u64,
        identity: ContentIdentity,
        duration_ms: u64,
    },
    Ready {
        media_revision: u64,
        clock_uncertainty_ms: u64,
    },
    NotReady {
        media_revision: u64,
        reason: NotReadyReason,
    },
    Play {
        context: ControlContext,
        position_ms: u64,
    },
    Pause {
        context: ControlContext,
    },
    Seek {
        context: ControlContext,
        position_ms: u64,
    },
    Transfer {
        context: AdminContext,
        target_member_id: Uuid,
    },
    Chat {
        text: String,
    },
    RichMessage {
        content: crate::social::MessageContent,
        reply_to_message_id: Option<Uuid>,
    },
    MessageReact {
        message_id: Uuid,
        emoji: String,
    },
    React {
        emoji: String,
    },
    P2p(cine_transfer_model::TransferIntent),
    Sync,
}
#[derive(Clone, PartialEq)]
pub struct Request {
    pub event_id: Uuid,
    pub room_id: Option<Uuid>,
    pub room_epoch: Option<Uuid>,
    /// Opaque digest of the validated canonical wire payload; absent for domain-only callers.
    pub payload_fingerprint: Option<[u8; 32]>,
    pub command: Command,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorCode {
    RoomNotFound,
    RoomFull,
    RoomLimitReached,
    NotAuthorized,
    InviteInvalid,
    ResumeExpired,
    MediaMismatch,
    MediaNotReady,
    MediaNotVerified,
    StaleMedia,
    StaleAuthority,
    NoMedia,
    InvalidMedia,
    UnsupportedSource,
    ProtocolVersionUnsupported,
    InvalidEvent,
    OutOfSequence,
    PositionOutOfRange,
    InvalidState,
    ControlPending,
    ClockUncertain,
    MemberNotFound,
    RateLimited,
    PayloadTooLarge,
    FeatureNotSupported,
    InternalError,
}
impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RoomNotFound => "ROOM_NOT_FOUND",
            Self::RoomFull => "ROOM_FULL",
            Self::RoomLimitReached => "ROOM_LIMIT_REACHED",
            Self::NotAuthorized => "NOT_AUTHORIZED",
            Self::InviteInvalid => "INVITE_INVALID",
            Self::ResumeExpired => "RESUME_EXPIRED",
            Self::MediaMismatch => "MEDIA_MISMATCH",
            Self::MediaNotReady => "MEDIA_NOT_READY",
            Self::MediaNotVerified => "MEDIA_NOT_VERIFIED",
            Self::StaleMedia => "STALE_MEDIA",
            Self::StaleAuthority => "STALE_AUTHORITY",
            Self::NoMedia => "NO_MEDIA",
            Self::InvalidMedia => "INVALID_MEDIA",
            Self::UnsupportedSource => "UNSUPPORTED_SOURCE",
            Self::ProtocolVersionUnsupported => "PROTOCOL_VERSION_UNSUPPORTED",
            Self::InvalidEvent => "INVALID_EVENT",
            Self::OutOfSequence => "OUT_OF_SEQUENCE",
            Self::PositionOutOfRange => "POSITION_OUT_OF_RANGE",
            Self::InvalidState => "INVALID_STATE",
            Self::ControlPending => "CONTROL_PENDING",
            Self::ClockUncertain => "CLOCK_UNCERTAIN",
            Self::MemberNotFound => "MEMBER_NOT_FOUND",
            Self::RateLimited => "RATE_LIMITED",
            Self::PayloadTooLarge => "PAYLOAD_TOO_LARGE",
            Self::FeatureNotSupported => "FEATURE_NOT_SUPPORTED",
            Self::InternalError => "INTERNAL_ERROR",
        }
    }
}
#[derive(Clone)]
pub struct Credentials {
    pub room_id: Uuid,
    pub room_epoch: Uuid,
    pub member_id: Uuid,
    pub invite_token: Option<String>,
    pub resume_token: String,
    pub lease_ms: u64,
}
#[derive(Clone)]
pub enum AckResult {
    Empty,
    Credentials(Credentials),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    MemberJoined,
    MemberStatus,
    MemberLeft,
    RoomClosed,
    MediaSelected,
    MediaVerified,
    MediaReadiness,
    Play,
    Pause,
    Seek,
    HostTransferred,
    SyncState,
}
impl EventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MemberJoined => "MEMBER_JOINED",
            Self::MemberStatus => "MEMBER_STATUS",
            Self::MemberLeft => "MEMBER_LEFT",
            Self::RoomClosed => "ROOM_CLOSED",
            Self::MediaSelected => "MEDIA_SELECTED",
            Self::MediaVerified => "MEDIA_VERIFIED",
            Self::MediaReadiness => "MEDIA_READINESS",
            Self::Play => "PLAY",
            Self::Pause => "PAUSE",
            Self::Seek => "SEEK",
            Self::HostTransferred => "HOST_TRANSFERRED",
            Self::SyncState => "SYNC_STATE",
        }
    }
}
#[derive(Clone)]
pub struct RoomEvent {
    pub kind: EventKind,
    pub state: RoomState,
    pub actor_id: Option<Uuid>,
    pub member_id: Option<Uuid>,
    pub reason: Option<&'static str>,
    pub command_event_id: Option<Uuid>,
    pub previous_host_id: Option<Uuid>,
}
#[derive(Clone)]
pub enum Effect {
    Social {
        room_id: Uuid,
        room_epoch: Uuid,
        payload: crate::social::SocialPayload,
    },
    Ack {
        request_event_id: Uuid,
        sequence: Option<u64>,
        result: AckResult,
    },
    Error {
        request_event_id: Uuid,
        code: ErrorCode,
    },
    P2p {
        room_id: Uuid,
        room_epoch: Uuid,
        snapshot: cine_transfer_model::TransferSnapshot,
        grant: Option<cine_transfer_model::Grant>,
    },
    Snapshot(RoomState),
    Event(RoomEvent),
    Close,
}
#[derive(Clone)]
pub struct Delivery {
    pub recipients: Vec<Uuid>,
    pub effect: Effect,
}
