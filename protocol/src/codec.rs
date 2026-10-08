use crate::*;
use cine_rooms::model::*;
use serde::{
    Deserialize,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

struct Strict(Value);
impl<'de> Deserialize<'de> for Strict {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Strict;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("JSON without duplicate keys")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut m: M) -> Result<Strict, M::Error> {
                let mut value = serde_json::Map::new();
                while let Some(k) = m.next_key::<String>()? {
                    if value.contains_key(&k) {
                        return Err(de::Error::custom("duplicate key"));
                    }
                    value.insert(k, m.next_value::<Strict>()?.0);
                }
                Ok(Strict(Value::Object(value)))
            }
            fn visit_seq<S: SeqAccess<'de>>(self, mut s: S) -> Result<Strict, S::Error> {
                let mut value = vec![];
                while let Some(v) = s.next_element::<Strict>()? {
                    value.push(v.0);
                }
                Ok(Strict(Value::Array(value)))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Strict, E> {
                Ok(Strict(Value::String(v.into())))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Strict, E> {
                Ok(Strict(Value::String(v)))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Strict, E> {
                serde_json::Number::from_f64(v)
                    .map(|n| Strict(Value::Number(n)))
                    .ok_or_else(|| E::custom("invalid number"))
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Strict, E> {
                Ok(Strict(Value::Null))
            }
        }
        d.deserialize_any(V)
    }
}
fn validate_value(v: &Value, key: &str) -> Result<(), ErrorCode> {
    match v {
        Value::Object(m) => {
            for (k, v) in m {
                validate_value(v, k)?;
            }
        }
        Value::Array(a) => {
            for v in a {
                validate_value(v, key)?;
            }
        }
        Value::Number(n) => {
            if n.as_u64().is_some_and(|v| v > MAX_INTEGER)
                || n.as_i64()
                    .is_some_and(|v| v < 0 && (key != "drift_ms" || v.unsigned_abs() > MAX_INTEGER))
                || (!n.is_u64() && !n.is_i64() && !matches!(key, "rate" | "applied_rate"))
            {
                return Err(ErrorCode::InvalidEvent);
            }
        }
        Value::String(s)
            if (key.ends_with("_id") && key != "provider_content_id")
                || key.ends_with("_epoch") =>
        {
            if key == "sender_id" && s == "server" {
                return Ok(());
            }
            let id = Uuid::parse_str(s).map_err(|_| ErrorCode::InvalidEvent)?;
            if id.get_version_num() != 4 || id.get_variant() != uuid::Variant::RFC4122 {
                return Err(ErrorCode::InvalidEvent);
            }
        }
        _ => {}
    }
    Ok(())
}
pub fn decode(text: &str) -> Result<WireMessage, ErrorCode> {
    if text.len() > MAX_MESSAGE_BYTES {
        return Err(ErrorCode::PayloadTooLarge);
    }
    let mut depth = 0u32;
    let mut in_string = false;
    let mut escaped = false;
    for b in text.bytes() {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
        } else if b == b'"' {
            in_string = true;
        } else if b == b'{' || b == b'[' {
            depth += 1;
            if depth > 12 {
                return Err(ErrorCode::InvalidEvent);
            }
        } else if b == b'}' || b == b']' {
            depth = depth.saturating_sub(1);
        }
    }
    let value = serde_json::from_str::<Strict>(text)
        .map_err(|_| ErrorCode::InvalidEvent)?
        .0;
    validate_value(&value, "")?;
    let object = value.as_object().ok_or(ErrorCode::InvalidEvent)?;
    for key in [
        "protocol_version",
        "event_id",
        "type",
        "room_id",
        "room_epoch",
        "sender_id",
        "sequence",
        "sent_at_ms",
        "payload",
    ] {
        if !object.contains_key(key) {
            return Err(ErrorCode::InvalidEvent);
        }
    }
    let message: WireMessage =
        serde_json::from_value(value).map_err(|_| ErrorCode::InvalidEvent)?;
    if message.protocol_version != 1 {
        return Err(ErrorCode::ProtocolVersionUnsupported);
    }
    if !message.payload.is_object() {
        return Err(ErrorCode::InvalidEvent);
    }
    Ok(message)
}
pub fn encode(message: &WireMessage) -> Result<String, ErrorCode> {
    let text = serde_json::to_string(message).map_err(|_| ErrorCode::InternalError)?;
    if text.len() > MAX_MESSAGE_BYTES {
        return Err(ErrorCode::PayloadTooLarge);
    }
    Ok(text)
}
#[derive(Deserialize)]
struct AdminDto {
    expected_sequence: u64,
    authority_revision: u64,
}
impl From<AdminDto> for AdminContext {
    fn from(a: AdminDto) -> Self {
        Self {
            expected_sequence: a.expected_sequence,
            authority_revision: a.authority_revision,
        }
    }
}
#[derive(Deserialize)]
struct ControlDto {
    expected_sequence: u64,
    authority_revision: u64,
    media_revision: u64,
}
impl From<ControlDto> for ControlContext {
    fn from(c: ControlDto) -> Self {
        Self {
            admin: AdminContext {
                expected_sequence: c.expected_sequence,
                authority_revision: c.authority_revision,
            },
            media_revision: c.media_revision,
        }
    }
}
#[derive(Deserialize)]
#[serde(tag = "type", content = "payload")]
enum RequestDto {
    #[serde(rename = "SESSION_HELLO")]
    Hello {
        supported_versions: Vec<u32>,
        client_name: String,
    },
    #[serde(rename = "TIME_PING")]
    Ping { sample_id: Uuid, t1_ms: u64 },
    #[serde(rename = "ROOM_CREATE")]
    Create { display_name: String },
    #[serde(rename = "ROOM_JOIN")]
    Join {
        invite_token: String,
        display_name: String,
    },
    #[serde(rename = "ROOM_RESUME")]
    Resume {
        resume_token: String,
        last_sequence: u64,
    },
    #[serde(rename = "ROOM_LEAVE")]
    Leave {},
    #[serde(rename = "MEDIA_SELECT_REQUEST")]
    Select {
        #[serde(flatten)]
        context: AdminDto,
        previous_media_revision: u64,
        descriptor: MediaDto,
    },
    #[serde(rename = "MEDIA_METADATA")]
    Metadata {
        media_revision: u64,
        identity: IdentityDto,
        duration_ms: u64,
        mime: Option<String>,
        codecs: Vec<String>,
    },
    #[serde(rename = "MEDIA_READY")]
    Ready {
        media_revision: u64,
        clock_uncertainty_ms: u64,
    },
    #[serde(rename = "MEDIA_NOT_READY")]
    NotReady { media_revision: u64, reason: String },
    #[serde(rename = "PLAY_REQUEST")]
    Play {
        #[serde(flatten)]
        context: ControlDto,
        position_ms: u64,
    },
    #[serde(rename = "PAUSE_REQUEST")]
    Pause {
        #[serde(flatten)]
        context: ControlDto,
    },
    #[serde(rename = "SEEK_REQUEST")]
    Seek {
        #[serde(flatten)]
        context: ControlDto,
        position_ms: u64,
    },
    #[serde(rename = "HOST_TRANSFER_REQUEST")]
    Transfer {
        #[serde(flatten)]
        context: AdminDto,
        target_member_id: Uuid,
    },
    #[serde(rename = "CHAT_SEND")]
    Chat { text: String },
    #[serde(rename = "MESSAGE_SEND")]
    RichMessage {
        content: MessageContentDto,
        reply_to_message_id: Option<Uuid>,
    },
    #[serde(rename = "MESSAGE_REACTION_SEND")]
    MessageReact { message_id: Uuid, emoji: String },
    #[serde(rename = "REACTION_SEND")]
    React { emoji: String },
    #[serde(rename = "SYNC_REQUEST")]
    Sync { last_sequence: u64, reason: String },
}
pub enum Incoming {
    Hello {
        supported_versions: Vec<u32>,
        client_name: String,
    },
    Ping {
        sample_id: Uuid,
        t1_ms: u64,
    },
    Room(Box<Request>),
}
fn name(s: &str) -> Result<(), ErrorCode> {
    if s.is_empty() || s.len() > 64 {
        Err(ErrorCode::InvalidEvent)
    } else {
        Ok(())
    }
}
fn token(s: &str) -> Result<(), ErrorCode> {
    if s.len() != 43
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        Err(ErrorCode::InvalidEvent)
    } else {
        Ok(())
    }
}
pub fn incoming(message: &WireMessage) -> Result<Incoming, ErrorCode> {
    if message.sequence.is_some() || message.sender_id.as_deref() == Some("server") {
        return Err(ErrorCode::NotAuthorized);
    }
    if [
        "PLAY",
        "PAUSE",
        "SEEK",
        "ROOM_STATE",
        "MEMBER_JOINED",
        "MEMBER_LEFT",
        "MEMBER_STATUS",
        "MEDIA_SELECTED",
        "MEDIA_VERIFIED",
        "MEDIA_READINESS",
        "HOST_TRANSFERRED",
        "SYNC_STATE",
        "ACK",
        "ERROR",
        "SESSION_ACCEPT",
        "TIME_PONG",
        "ROOM_CLOSED",
        "SOCIAL_STATE",
        "CHAT_MESSAGE",
        "REACTION",
    ]
    .contains(&message.kind.as_str())
    {
        return Err(ErrorCode::NotAuthorized);
    }
    if matches!(message.kind.as_str(), "CHAT_SEND" | "REACTION_SEND")
        && (message.payload.as_object().is_none_or(|p| p.len() != 1)
            || message
                .payload
                .get(if message.kind == "CHAT_SEND" {
                    "text"
                } else {
                    "emoji"
                })
                .is_none())
    {
        return Err(ErrorCode::InvalidEvent);
    }
    if matches!(
        message.kind.as_str(),
        "MESSAGE_SEND" | "MESSAGE_REACTION_SEND"
    ) {
        let allowed = if message.kind == "MESSAGE_SEND" {
            ["content", "reply_to_message_id"]
        } else {
            ["message_id", "emoji"]
        };
        if message
            .payload
            .as_object()
            .is_none_or(|p| p.keys().any(|key| !allowed.contains(&key.as_str())))
        {
            return Err(ErrorCode::InvalidEvent);
        }
    }
    let dto: RequestDto =
        serde_json::from_value(json!({"type":message.kind,"payload":message.payload}))
            .map_err(|_| ErrorCode::InvalidEvent)?;
    let command = match dto {
        RequestDto::Hello {
            supported_versions,
            client_name,
        } => {
            name(&client_name)?;
            if supported_versions.is_empty() || supported_versions.len() > 8 {
                return Err(ErrorCode::InvalidEvent);
            }
            if message.room_id.is_some()
                || message.room_epoch.is_some()
                || message.sender_id.is_some()
            {
                return Err(ErrorCode::InvalidEvent);
            }
            return Ok(Incoming::Hello {
                supported_versions,
                client_name,
            });
        }
        RequestDto::Ping { sample_id, t1_ms } => {
            if message.room_id.is_some() || message.room_epoch.is_some() {
                return Err(ErrorCode::InvalidEvent);
            }
            return Ok(Incoming::Ping { sample_id, t1_ms });
        }
        RequestDto::Create { display_name } => {
            name(&display_name)?;
            if message.room_id.is_some() || message.room_epoch.is_some() {
                return Err(ErrorCode::InvalidEvent);
            }
            Command::Create { display_name }
        }
        RequestDto::Join {
            invite_token,
            display_name,
        } => {
            name(&display_name)?;
            token(&invite_token)?;
            Command::Join {
                invite_token,
                display_name,
            }
        }
        RequestDto::Resume {
            resume_token,
            last_sequence,
        } => {
            token(&resume_token)?;
            Command::Resume {
                resume_token,
                last_sequence,
            }
        }
        RequestDto::Chat { text } => {
            cine_rooms::social::validate_text(&text)?;
            Command::Chat { text }
        }
        RequestDto::RichMessage {
            content,
            reply_to_message_id,
        } => {
            let content = content.domain();
            content.validate()?;
            Command::RichMessage {
                content,
                reply_to_message_id,
            }
        }
        RequestDto::MessageReact { message_id, emoji } => {
            if !cine_rooms::social::REACTIONS.contains(&emoji.as_str()) {
                return Err(ErrorCode::InvalidEvent);
            }
            Command::MessageReact { message_id, emoji }
        }
        RequestDto::React { emoji } => {
            if !cine_rooms::social::REACTIONS.contains(&emoji.as_str()) {
                return Err(ErrorCode::InvalidEvent);
            }
            Command::React { emoji }
        }
        RequestDto::Leave {} => Command::Leave,
        RequestDto::Select {
            context,
            previous_media_revision,
            descriptor,
        } => Command::Select {
            context: context.into(),
            previous_media_revision,
            descriptor: descriptor.validate()?,
        },
        RequestDto::Metadata {
            media_revision,
            identity,
            duration_ms,
            mime,
            codecs,
        } => {
            if duration_ms == 0
                || duration_ms > 604_800_000
                || mime.as_ref().is_some_and(|m| m.len() > 128)
                || codecs.len() > 16
                || codecs.iter().any(|c| c.is_empty() || c.len() > 64)
            {
                return Err(ErrorCode::InvalidMedia);
            }
            Command::Metadata {
                media_revision,
                identity: identity.validate()?,
                duration_ms,
            }
        }
        RequestDto::Ready {
            media_revision,
            clock_uncertainty_ms,
        } => Command::Ready {
            media_revision,
            clock_uncertainty_ms,
        },
        RequestDto::NotReady {
            media_revision,
            reason,
        } => Command::NotReady {
            media_revision,
            reason: match reason.as_str() {
                "loading" => NotReadyReason::Loading,
                "buffering" => NotReadyReason::Buffering,
                "player_error" => NotReadyReason::PlayerError,
                "user" => NotReadyReason::User,
                _ => return Err(ErrorCode::InvalidEvent),
            },
        },
        RequestDto::Play {
            context,
            position_ms,
        } => Command::Play {
            context: context.into(),
            position_ms,
        },
        RequestDto::Pause { context } => Command::Pause {
            context: context.into(),
        },
        RequestDto::Seek {
            context,
            position_ms,
        } => Command::Seek {
            context: context.into(),
            position_ms,
        },
        RequestDto::Transfer {
            context,
            target_member_id,
        } => Command::Transfer {
            context: context.into(),
            target_member_id,
        },
        RequestDto::Sync {
            last_sequence,
            reason,
        } => {
            let _ = last_sequence;
            if !["initial", "gap", "resume", "clock_change", "manual"].contains(&reason.as_str()) {
                return Err(ErrorCode::InvalidEvent);
            }
            Command::Sync
        }
    };
    if !matches!(command, Command::Create { .. })
        && (message.room_id.is_none() || message.room_epoch.is_none())
    {
        return Err(ErrorCode::InvalidEvent);
    }
    Ok(Incoming::Room(Box::new(Request {
        event_id: message.event_id,
        room_id: message.room_id,
        room_epoch: message.room_epoch,
        payload_fingerprint: Some(
            Sha256::digest(
                serde_json::to_vec(&message.payload).map_err(|_| ErrorCode::InvalidEvent)?,
            )
            .into(),
        ),
        command,
    })))
}
pub fn error_message(id: Option<Uuid>, code: ErrorCode, now: u64) -> WireMessage {
    WireMessage::server(
        "ERROR",
        json!({"request_event_id":id,"error":{"code":code.as_str(),"message":"Request rejected by protocol or room rules"}}),
        now,
    )
}
pub fn effect_message(effect: &Effect, clock_epoch: Uuid, now: u64) -> Option<WireMessage> {
    let mut message = match effect {
        Effect::Social {
            room_id,
            room_epoch,
            payload,
        } => return Some(social_message(*room_id, *room_epoch, payload, now)),
        Effect::Close => return None,
        Effect::Error {
            request_event_id,
            code,
        } => return Some(error_message(Some(*request_event_id), *code, now)),
        Effect::Ack {
            request_event_id,
            sequence,
            result,
        } => {
            let result = match result {
                AckResult::Empty => json!({}),
                AckResult::Credentials(c) => {
                    let mut value = json!({"room_id":c.room_id,"room_epoch":c.room_epoch,"member_id":c.member_id,"resume_token":c.resume_token,"lease_ms":c.lease_ms});
                    if let Some(token) = &c.invite_token {
                        value["invite_token"] = json!(token);
                    }
                    value
                }
            };
            WireMessage::server(
                "ACK",
                json!({"request_event_id":request_event_id,"room_sequence":sequence,"result":result}),
                now,
            )
        }
        Effect::Snapshot(state) => {
            let mut m = WireMessage::server(
                "ROOM_STATE",
                json!({"state":StateDto::from(state),"clock_epoch":clock_epoch}),
                now,
            );
            m.room_id = Some(state.room_id);
            m.room_epoch = Some(state.room_epoch);
            m.sequence = Some(state.sequence);
            m
        }
        Effect::Event(e) => {
            let mut payload = json!({"state":StateDto::from(&e.state),"actor_id":e.actor_id});
            if let Some(id) = e.member_id {
                payload["member_id"] = json!(id);
            }
            if let Some(reason) = e.reason {
                payload["reason"] = json!(reason);
            }
            if let Some(id) = e.command_event_id {
                payload["command_event_id"] = json!(id);
            }
            if e.kind == EventKind::MediaSelected
                || e.kind == EventKind::MediaVerified
                || e.kind == EventKind::MediaReadiness
            {
                payload["media_revision"] = json!(e.state.media.as_ref().map(|m| m.media_revision));
            }
            if e.kind == EventKind::HostTransferred {
                payload["previous_host_id"] = json!(e.previous_host_id);
                payload["host_id"] = json!(e.state.host_id);
                payload["authority_revision"] = json!(e.state.authority_revision);
            }
            let mut m = WireMessage::server(e.kind.as_str(), payload, now);
            m.room_id = Some(e.state.room_id);
            m.room_epoch = Some(e.state.room_epoch);
            m.sequence = Some(e.state.sequence);
            m
        }
    };
    message.sender_id = Some("server".into());
    Some(message)
}
pub fn state_from_message(message: &WireMessage) -> Result<Option<RoomState>, ErrorCode> {
    if message.sender_id.as_deref() != Some("server") {
        return Err(ErrorCode::NotAuthorized);
    }
    if ![
        "ROOM_STATE",
        "SYNC_STATE",
        "MEMBER_JOINED",
        "MEMBER_STATUS",
        "MEMBER_LEFT",
        "ROOM_CLOSED",
        "MEDIA_SELECTED",
        "MEDIA_VERIFIED",
        "MEDIA_READINESS",
        "PLAY",
        "PAUSE",
        "SEEK",
        "HOST_TRANSFERRED",
    ]
    .contains(&message.kind.as_str())
    {
        if !["ACK", "ERROR", "SESSION_ACCEPT", "TIME_PONG"].contains(&message.kind.as_str()) {
            return Err(ErrorCode::InvalidEvent);
        }
        return Ok(None);
    }
    let dto: StateDto = serde_json::from_value(
        message
            .payload
            .get("state")
            .cloned()
            .ok_or(ErrorCode::InvalidEvent)?,
    )
    .map_err(|_| ErrorCode::InvalidEvent)?;
    let state = dto.validate()?;
    if message.room_id != Some(state.room_id)
        || message.room_epoch != Some(state.room_epoch)
        || message.sequence != Some(state.sequence)
    {
        return Err(ErrorCode::InvalidEvent);
    }
    Ok(Some(state))
}
