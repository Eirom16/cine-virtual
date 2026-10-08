use crate::WireMessage;
use cine_rooms::social::{GifDescriptor, MessageContent};
use cine_rooms::{
    model::ErrorCode,
    social::{
        HISTORY_MAX_BYTES, HISTORY_MAX_COUNT, REACTIONS, SocialEntry, SocialPayload, validate_text,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GifDto {
    pub provider: String,
    pub provider_content_id: String,
    pub media_url: String,
    pub preview_url: Option<String>,
    pub width: u16,
    pub height: u16,
    pub alt_text: String,
}
impl From<&GifDescriptor> for GifDto {
    fn from(g: &GifDescriptor) -> Self {
        Self {
            provider: g.provider.clone(),
            provider_content_id: g.provider_content_id.clone(),
            media_url: g.media_url.clone(),
            preview_url: g.preview_url.clone(),
            width: g.width,
            height: g.height,
            alt_text: g.alt_text.clone(),
        }
    }
}
impl From<GifDto> for GifDescriptor {
    fn from(g: GifDto) -> Self {
        Self {
            provider: g.provider,
            provider_content_id: g.provider_content_id,
            media_url: g.media_url,
            preview_url: g.preview_url,
            width: g.width,
            height: g.height,
            alt_text: g.alt_text,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum MessageContentDto {
    Text { text: String },
    Gif { gif: GifDto },
}
impl MessageContentDto {
    pub fn domain(&self) -> MessageContent {
        match self {
            Self::Text { text } => MessageContent::Text(text.clone()),
            Self::Gif { gif } => MessageContent::Gif(gif.clone().into()),
        }
    }
}

use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SocialEntryDto {
    pub message_id: Uuid,
    pub sender_id: Uuid,
    pub display_name: String,
    pub social_sequence: u64,
    pub sent_at_ms: u64,
    pub kind: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<MessageContentDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to_message_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sent_at_utc_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub message_reactions: BTreeMap<String, BTreeSet<Uuid>>,
}
impl From<&SocialEntry> for SocialEntryDto {
    fn from(e: &SocialEntry) -> Self {
        Self {
            message_id: e.message_id,
            sender_id: e.sender_id,
            display_name: e.display_name.clone(),
            social_sequence: e.social_sequence,
            sent_at_ms: e.sent_at_ms,
            kind: e.kind().into(),
            text: e.text().into(),
            content: match &e.content {
                MessageContent::Text(text) => Some(MessageContentDto::Text { text: text.clone() }),
                MessageContent::Gif(gif) => Some(MessageContentDto::Gif { gif: gif.into() }),
                MessageContent::System(_) => None,
            },
            reply_to_message_id: e.reply_to_message_id,
            sent_at_utc_ms: e.sent_at_utc_ms,
            message_reactions: e.message_reactions.clone(),
        }
    }
}
impl SocialEntryDto {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.display_name.is_empty() || self.display_name.len() > 64 || self.social_sequence == 0
        {
            return Err(ErrorCode::InvalidEvent);
        }
        if self.message_reactions.len() > 6
            || self.message_reactions.iter().any(|(emoji, ids)| {
                !REACTIONS.contains(&emoji.as_str())
                    || ids.is_empty()
                    || ids.len() > 16
                    || ids.iter().any(|id| id.get_version_num() != 4)
            })
            || self.sent_at_utc_ms.is_some_and(|t| t > crate::MAX_INTEGER)
            || (self.kind != "chat"
                && (self.content.is_some()
                    || self.reply_to_message_id.is_some()
                    || !self.message_reactions.is_empty()))
        {
            return Err(ErrorCode::InvalidEvent);
        }
        if let Some(content) = &self.content {
            content.domain().validate()?;
            match content {
                MessageContentDto::Text { text } if text != &self.text => {
                    return Err(ErrorCode::InvalidEvent);
                }
                MessageContentDto::Gif { .. } if self.text != "[GIF]" => {
                    return Err(ErrorCode::InvalidEvent);
                }
                _ => {}
            }
        }
        match self.kind.as_str() {
            "chat" => {
                if validate_text(&self.text)? != self.text {
                    return Err(ErrorCode::InvalidEvent);
                }
            }
            "joined" | "left" | "resumed" if self.text.is_empty() => {}
            _ => return Err(ErrorCode::InvalidEvent),
        }
        Ok(())
    }
    pub fn budget(&self) -> usize {
        let legacy = cine_rooms::social::entry_budget(&self.text, &self.display_name);
        if self.content.is_none()
            && self.reply_to_message_id.is_none()
            && self.message_reactions.is_empty()
        {
            return legacy;
        }
        legacy * 2
            + 128
            + match &self.content {
                Some(MessageContentDto::Gif { gif }) => GifDescriptor::from(gif.clone()).budget(),
                _ => 0,
            }
            + self
                .message_reactions
                .values()
                .map(|ids| 64 + ids.len() * 40)
                .sum::<usize>()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReactionDto {
    pub reaction_id: Uuid,
    pub sender_id: Uuid,
    pub emoji: String,
    pub sent_at_ms: u64,
}
#[derive(Deserialize)]
pub struct SocialSnapshotDto {
    pub social_sequence: u64,
    pub entries: Vec<SocialEntryDto>,
    #[serde(default)]
    pub live_update: bool,
}
impl SocialSnapshotDto {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.entries.len() > HISTORY_MAX_COUNT
            || self.entries.iter().map(|e| e.budget()).sum::<usize>() > HISTORY_MAX_BYTES
        {
            return Err(ErrorCode::InvalidEvent);
        }
        let mut last = 0;
        let mut ids = std::collections::HashSet::new();
        for entry in &self.entries {
            entry.validate()?;
            if entry.social_sequence <= last
                || entry.social_sequence > self.social_sequence
                || !ids.insert(entry.message_id)
            {
                return Err(ErrorCode::InvalidEvent);
            }
            last = entry.social_sequence;
        }
        Ok(())
    }
}
pub fn social_message(
    room_id: Uuid,
    room_epoch: Uuid,
    payload: &SocialPayload,
    now: u64,
) -> WireMessage {
    let (kind, value) = match payload {
        SocialPayload::Snapshot { sequence, entries } => (
            "SOCIAL_STATE",
            json!({"social_sequence":sequence,"entries":entries.iter().map(SocialEntryDto::from).collect::<Vec<_>>()}),
        ),
        SocialPayload::MessageReactions { sequence, entries } => (
            "SOCIAL_STATE",
            json!({"social_sequence":sequence,"entries":entries.iter().map(SocialEntryDto::from).collect::<Vec<_>>(),"live_update":true}),
        ),
        SocialPayload::Message(e) => ("CHAT_MESSAGE", json!(SocialEntryDto::from(e))),
        SocialPayload::Reaction {
            reaction_id,
            sender_id,
            emoji,
            sent_at_ms,
        } => (
            "REACTION",
            json!(ReactionDto {
                reaction_id: *reaction_id,
                sender_id: *sender_id,
                emoji: emoji.clone(),
                sent_at_ms: *sent_at_ms
            }),
        ),
    };
    let mut m = WireMessage::server(kind, value, now);
    m.room_id = Some(room_id);
    m.room_epoch = Some(room_epoch);
    m
}
pub fn validate_reaction(r: &ReactionDto) -> Result<(), ErrorCode> {
    if REACTIONS.contains(&r.emoji.as_str()) {
        Ok(())
    } else {
        Err(ErrorCode::InvalidEvent)
    }
}

/// Old social_v1 receives exactly its original representation and no rich metadata.
pub fn legacy_social(message: &mut WireMessage) {
    fn strip(value: &mut serde_json::Value) {
        if let Some(entry) = value.as_object_mut() {
            for key in [
                "content",
                "reply_to_message_id",
                "sent_at_utc_ms",
                "message_reactions",
            ] {
                entry.remove(key);
            }
        }
    }
    match message.kind.as_str() {
        "CHAT_MESSAGE" => strip(&mut message.payload),
        "SOCIAL_STATE" => {
            if let Some(entries) = message.payload["entries"].as_array_mut() {
                for entry in entries {
                    strip(entry);
                }
            }
        }
        _ => {}
    }
}
