use crate::WireMessage;
use cine_rooms::{
    model::ErrorCode,
    social::{
        HISTORY_MAX_BYTES, HISTORY_MAX_COUNT, REACTIONS, SocialEntry, SocialPayload, validate_text,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::json;
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
}
impl From<&SocialEntry> for SocialEntryDto {
    fn from(e: &SocialEntry) -> Self {
        Self {
            message_id: e.message_id,
            sender_id: e.sender_id,
            display_name: e.display_name.clone(),
            social_sequence: e.social_sequence,
            sent_at_ms: e.sent_at_ms,
            kind: e.kind.clone(),
            text: e.text.clone(),
        }
    }
}
impl SocialEntryDto {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.display_name.is_empty() || self.display_name.len() > 64 || self.social_sequence == 0
        {
            return Err(ErrorCode::InvalidEvent);
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
        cine_rooms::social::entry_budget(&self.text, &self.display_name)
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
