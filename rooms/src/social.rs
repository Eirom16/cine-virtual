//! Bounded, in-memory social state. Never participates in playback authority.
use crate::model::{ErrorCode, Member};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
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
/// Provider identity is stable; URLs are bounded delivery hints, never arbitrary input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GifDescriptor {
    pub provider: String,
    pub provider_content_id: String,
    pub media_url: String,
    pub preview_url: Option<String>,
    pub width: u16,
    pub height: u16,
    pub alt_text: String,
}
impl GifDescriptor {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        let id = &self.provider_content_id;
        if id.is_empty()
            || id.len() > 64
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || !(1..=640).contains(&self.width)
            || !(1..=640).contains(&self.height)
            || self.alt_text.len() > 256
            || self.alt_text.chars().any(char::is_control)
        {
            return Err(ErrorCode::InvalidEvent);
        }
        self.validate_url(&self.media_url)?;
        if let Some(url) = &self.preview_url {
            self.validate_url(url)?;
        }
        Ok(())
    }
    fn validate_url(&self, raw: &str) -> Result<(), ErrorCode> {
        if raw.len() > 1024 || raw.bytes().any(|b| b <= 0x20 || b == b'\\') {
            return Err(ErrorCode::InvalidEvent);
        }
        if !raw.starts_with("https://")
            || raw[8..]
                .split('/')
                .next()
                .is_some_and(|authority| authority.contains(':') || authority.contains('@'))
        {
            return Err(ErrorCode::InvalidEvent);
        }
        let url = url::Url::parse(raw).map_err(|_| ErrorCode::InvalidEvent)?;
        if url.scheme() != "https"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some()
            || url.fragment().is_some()
        {
            return Err(ErrorCode::InvalidEvent);
        }
        let allowed = match self.provider.as_str() {
            "fixture" => {
                self.provider_content_id == "celebrate"
                    && raw == "https://fixtures.cine.invalid/celebrate.gif"
            }
            "giphy" => {
                matches!(
                    url.host_str(),
                    Some(
                        "media.giphy.com"
                            | "media0.giphy.com"
                            | "media1.giphy.com"
                            | "media2.giphy.com"
                            | "media3.giphy.com"
                            | "media4.giphy.com"
                    )
                ) && url
                    .path()
                    .starts_with(&format!("/media/{}/", self.provider_content_id))
                    && !url.path().contains('%')
                    && !url.path().contains("..")
                    && (url.path().ends_with(".gif") || url.path().ends_with(".webp"))
                    && url.query_pairs().all(|(k, v)| {
                        matches!(k.as_ref(), "cid" | "ep" | "rid" | "ct") && v.len() <= 128
                    })
            }
            _ => false,
        };
        if allowed {
            Ok(())
        } else {
            Err(ErrorCode::InvalidEvent)
        }
    }
    pub fn budget(&self) -> usize {
        512 + self.media_url.len() * 2
            + self.preview_url.as_ref().map_or(0, |s| s.len() * 2)
            + self.alt_text.len() * 6
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MessageContent {
    Text(String),
    Gif(GifDescriptor),
    System(String),
}
impl MessageContent {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        match self {
            Self::Text(text) => {
                validate_text(text)?;
                Ok(())
            }
            Self::Gif(gif) => gif.validate(),
            Self::System(_) => Err(ErrorCode::InvalidEvent),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SocialEntry {
    pub message_id: Uuid,
    pub sender_id: Uuid,
    pub display_name: String,
    pub social_sequence: u64,
    pub sent_at_ms: u64,
    /// UTC epoch milliseconds for presentation only; never sequence or sync.
    pub sent_at_utc_ms: Option<u64>,
    pub content: MessageContent,
    pub reply_to_message_id: Option<Uuid>,
    pub message_reactions: BTreeMap<String, BTreeSet<Uuid>>,
}
impl SocialEntry {
    pub fn kind(&self) -> &str {
        match &self.content {
            MessageContent::System(kind) => kind,
            _ => "chat",
        }
    }
    pub fn text(&self) -> &str {
        match &self.content {
            MessageContent::Text(text) => text,
            MessageContent::Gif(_) => "[GIF]",
            MessageContent::System(_) => "",
        }
    }
    pub fn budget(&self) -> usize {
        entry_budget(self.text(), &self.display_name) * 2
            + 128
            + match &self.content {
                MessageContent::Gif(gif) => gif.budget(),
                _ => 0,
            }
            + self
                .message_reactions
                .values()
                .map(|members| 64 + members.len() * 40)
                .sum::<usize>()
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
    message_quotas: HashMap<Uuid, Bucket>,
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
        self.message_quotas.remove(&member);
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
        let content = if kind == "chat" {
            MessageContent::Text(text.into())
        } else {
            MessageContent::System(kind.into())
        };
        self.message(id, member, content, None, now)
    }
    pub fn validate_reply(&self, reply: Option<Uuid>) -> Result<(), ErrorCode> {
        if reply.is_some_and(|id| {
            !self
                .history
                .iter()
                .any(|e| e.message_id == id && e.kind() == "chat")
        }) {
            return Err(ErrorCode::InvalidEvent);
        }
        Ok(())
    }
    pub fn message(
        &mut self,
        id: Uuid,
        member: &Member,
        content: MessageContent,
        reply: Option<Uuid>,
        now: u64,
    ) -> SocialEntry {
        self.sequence += 1;
        let entry = SocialEntry {
            message_id: id,
            sender_id: member.member_id,
            display_name: member.display_name.clone(),
            social_sequence: self.sequence,
            sent_at_ms: now,
            sent_at_utc_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|t| t.as_millis() as u64),
            content,
            reply_to_message_id: reply,
            message_reactions: BTreeMap::new(),
        };
        self.bytes += entry.budget();
        self.history.push_back(entry.clone());
        self.trim();
        entry
    }
    fn trim(&mut self) {
        while self.history.len() > HISTORY_MAX_COUNT || self.bytes > HISTORY_MAX_BYTES {
            self.bytes -= self.history.pop_front().unwrap().budget();
        }
    }
    pub fn toggle(
        &mut self,
        message: Uuid,
        member: Uuid,
        emoji: &str,
        now: u64,
    ) -> Result<(), ErrorCode> {
        if !REACTIONS.contains(&emoji)
            || !self
                .history
                .iter()
                .any(|e| e.message_id == message && e.kind() == "chat")
        {
            return Err(ErrorCode::InvalidEvent);
        }
        let quota = self.message_quotas.entry(member).or_insert(Bucket {
            units: 6 * 1000,
            at: now,
        });
        if !quota.take(now, 6, 1000) {
            return Err(ErrorCode::RateLimited);
        }
        let entry = self
            .history
            .iter_mut()
            .find(|e| e.message_id == message)
            .unwrap();
        let old = entry.budget();
        let members = entry.message_reactions.entry(emoji.into()).or_default();
        if !members.remove(&member) {
            if members.len() >= 16 {
                return Err(ErrorCode::RateLimited);
            }
            members.insert(member);
        }
        if members.is_empty() {
            entry.message_reactions.remove(emoji);
        }
        self.bytes = self.bytes - old + entry.budget();
        self.sequence += 1;
        self.trim();
        Ok(())
    }
}
#[derive(Clone)]
pub enum SocialPayload {
    Snapshot {
        sequence: u64,
        entries: Vec<SocialEntry>,
    },
    MessageReactions {
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
