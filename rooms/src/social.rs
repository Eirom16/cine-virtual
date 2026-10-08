//! Bounded, in-memory social state. Never participates in playback authority.
use crate::model::{ErrorCode, Member};
use std::collections::{HashMap, VecDeque};
use uuid::Uuid;

pub const CHAT_MAX_BYTES: usize = 2048;
pub const HISTORY_MAX_COUNT: usize = 100;
// Conservative JSON budget (including escaping), below the 64 KiB wire boundary.
pub const HISTORY_MAX_BYTES: usize = 48 * 1024;
pub const REACTIONS: [&str; 6] = ["❤️", "😂", "😮", "😢", "🔥", "👏"];

pub fn validate_text(text: &str) -> Result<&str, ErrorCode> {
    if text.len() > CHAT_MAX_BYTES {
        return Err(ErrorCode::PayloadTooLarge);
    }
    let text = text.trim();
    if text.is_empty()
        || text.chars().filter(|c| *c == '\n').count() > 8
        || text
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(ErrorCode::InvalidEvent);
    }
    Ok(text)
}
/// Fixed metadata allowance plus the actual upper bound for JSON string escaping.
/// Count UTF-8 normally: multiplying all Unicode by six would evict useful history.
pub fn entry_budget(text: &str, display_name: &str) -> usize {
    fn escaped_bytes(text: &str) -> usize {
        text.chars()
            .map(|c| match c {
                '"' | '\\' | '\n' | '\r' | '\t' | '\u{8}' | '\u{c}' => 2,
                c if c < '\u{20}' => 6,
                c => c.len_utf8(),
            })
            .sum()
    }
    384 + escaped_bytes(text) + escaped_bytes(display_name)
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SocialEntry {
    pub message_id: Uuid,
    pub sender_id: Uuid,
    pub display_name: String,
    pub social_sequence: u64,
    pub sent_at_ms: u64,
    /// chat | joined | left | resumed. System entries contain no user text.
    pub kind: String,
    pub text: String,
}
impl SocialEntry {
    pub fn budget(&self) -> usize {
        entry_budget(&self.text, &self.display_name)
    }
}
struct Bucket {
    units: u64,
    at: u64,
}
impl Bucket {
    fn take(&mut self, now: u64, burst: u64, interval: u64) -> bool {
        self.units = (self.units + now.saturating_sub(self.at)).min(burst * interval);
        self.at = now;
        if self.units < interval {
            return false;
        }
        self.units -= interval;
        true
    }
}
#[derive(Default)]
pub struct SocialState {
    pub sequence: u64,
    pub history: VecDeque<SocialEntry>,
    pub bytes: usize,
    quotas: HashMap<Uuid, (Bucket, Bucket)>,
    last_resumed: HashMap<Uuid, u64>,
}
impl SocialState {
    pub fn allow(&mut self, member: Uuid, reaction: bool, now: u64) -> bool {
        let (chat, reactions) = self.quotas.entry(member).or_insert_with(|| {
            (
                Bucket {
                    units: 5 * 2000,
                    at: now,
                },
                Bucket {
                    units: 8 * 500,
                    at: now,
                },
            )
        });
        if reaction {
            reactions.take(now, 8, 500)
        } else {
            chat.take(now, 5, 2000)
        }
    }
    pub fn forget(&mut self, member: Uuid) {
        self.quotas.remove(&member);
        self.last_resumed.remove(&member);
    }
    pub fn presence(&mut self, member: &Member, kind: &str, now: u64) -> Option<SocialEntry> {
        if kind == "resumed" {
            if self
                .last_resumed
                .get(&member.member_id)
                .is_some_and(|last| now.saturating_sub(*last) < 30_000)
            {
                return None;
            }
            self.last_resumed.insert(member.member_id, now);
        }
        Some(self.entry(Uuid::new_v4(), member, kind, "", now))
    }
    pub fn entry(
        &mut self,
        id: Uuid,
        member: &Member,
        kind: &str,
        text: &str,
        now: u64,
    ) -> SocialEntry {
        self.sequence += 1;
        let entry = SocialEntry {
            message_id: id,
            sender_id: member.member_id,
            display_name: member.display_name.clone(),
            social_sequence: self.sequence,
            sent_at_ms: now,
            kind: kind.into(),
            text: text.into(),
        };
        self.bytes += entry.budget();
        self.history.push_back(entry.clone());
        while self.history.len() > HISTORY_MAX_COUNT || self.bytes > HISTORY_MAX_BYTES {
            self.bytes -= self.history.pop_front().unwrap().budget();
        }
        entry
    }
}
#[derive(Clone)]
pub enum SocialPayload {
    Snapshot {
        sequence: u64,
        entries: Vec<SocialEntry>,
    },
    Message(SocialEntry),
    Reaction {
        reaction_id: Uuid,
        sender_id: Uuid,
        emoji: String,
        sent_at_ms: u64,
    },
}
