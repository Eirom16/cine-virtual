//! Social projection owned by the same Client session; no player effects.
use cine_protocol::{
    ReactionDto, SocialEntryDto, SocialSnapshotDto, WireMessage, validate_reaction,
};
use cine_rooms::social::{HISTORY_MAX_BYTES, HISTORY_MAX_COUNT};
use serde_json::{Value, json};
use std::collections::VecDeque;
use uuid::Uuid;

#[derive(Default)]
pub struct SocialReplica {
    pub scope: Option<(Uuid, Uuid)>,
    pub sequence: u64,
    history: VecDeque<SocialEntryDto>,
    reactions: VecDeque<(ReactionDto, u64)>,
    pub dropped_reactions: u64,
    pub supported: bool,
    pub rich_supported: bool,
}
impl SocialReplica {
    pub fn install(&mut self, message: &WireMessage, now: u64) -> Result<bool, &'static str> {
        let scope = (
            message.room_id.ok_or("INVALID_EVENT")?,
            message.room_epoch.ok_or("INVALID_EVENT")?,
        );
        if self.scope != Some(scope) {
            if message.kind != "SOCIAL_STATE" {
                return Err("INVALID_EVENT");
            }
            self.scope = Some(scope);
            self.sequence = 0;
            self.history.clear();
            self.reactions.clear();
        }
        match message.kind.as_str() {
            "SOCIAL_STATE" => {
                let snapshot: SocialSnapshotDto =
                    serde_json::from_value(message.payload.clone()).map_err(|_| "INVALID_EVENT")?;
                snapshot.validate().map_err(|_| "INVALID_EVENT")?;
                if snapshot.social_sequence >= self.sequence {
                    self.sequence = snapshot.social_sequence;
                    self.history = snapshot.entries.into();
                }
                // Ephemeral animations never survive resume/snapshot.
                if !snapshot.live_update {
                    self.reactions.clear();
                }
            }
            "CHAT_MESSAGE" => {
                let entry: SocialEntryDto =
                    serde_json::from_value(message.payload.clone()).map_err(|_| "INVALID_EVENT")?;
                entry.validate().map_err(|_| "INVALID_EVENT")?;
                if entry.social_sequence <= self.sequence {
                    return Ok(false);
                }
                if entry.social_sequence != self.sequence + 1 {
                    return Ok(true);
                }
                self.sequence = entry.social_sequence;
                self.history.push_back(entry);
                while self.history.len() > HISTORY_MAX_COUNT
                    || self.history.iter().map(|e| e.budget()).sum::<usize>() > HISTORY_MAX_BYTES
                {
                    self.history.pop_front();
                }
            }
            "REACTION" => {
                let r: ReactionDto =
                    serde_json::from_value(message.payload.clone()).map_err(|_| "INVALID_EVENT")?;
                validate_reaction(&r).map_err(|_| "INVALID_EVENT")?;
                self.reactions.retain(|(_, until)| now < *until);
                if message.sent_at_ms.saturating_sub(r.sent_at_ms) > 3000
                    || self
                        .reactions
                        .iter()
                        .any(|(old, _)| old.reaction_id == r.reaction_id)
                {
                    return Ok(false);
                }
                if self.reactions.len() == 32 {
                    self.reactions.pop_front();
                    self.dropped_reactions += 1;
                }
                self.reactions.push_back((r, now + 3000));
            }
            _ => return Err("INVALID_EVENT"),
        }
        Ok(false)
    }
    pub fn summary(&self, now: u64) -> Value {
        json!({"supported":self.supported,"rich_supported":self.rich_supported,"social_sequence":self.sequence,"entries":self.history,
            "reactions":self.reactions.iter().filter(|(_,until)|now<*until).map(|(r,_)|r).collect::<Vec<_>>(),
            "buffer_count":self.history.len(),"buffer_bytes":self.history.iter().map(|e|e.budget()).sum::<usize>(),"dropped_reactions":self.dropped_reactions})
    }
    pub fn disconnected(&mut self) {
        self.reactions.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cine_protocol::social_message;
    use cine_rooms::{
        model::{Member, MemberStatus, Role},
        social::{SocialPayload, SocialState},
    };
    #[test]
    fn replay_dedup_gap_and_ephemeral_queue_remain_bounded() {
        let room = Uuid::new_v4();
        let epoch = Uuid::new_v4();
        let member = Member {
            member_id: Uuid::new_v4(),
            display_name: "Alex".into(),
            role: Role::Host,
            connected: true,
            ready: false,
            verified_media_revision: None,
            status: MemberStatus::Idle,
            joined_at_ms: 0,
            lease_expires_at_ms: None,
        };
        let mut state = SocialState::default();
        let mut replica = SocialReplica::default();
        let snapshot = social_message(
            room,
            epoch,
            &SocialPayload::Snapshot {
                sequence: 0,
                entries: vec![],
            },
            0,
        );
        replica.install(&snapshot, 0).unwrap();
        for i in 1..=500 {
            let entry = state.entry(Uuid::new_v4(), &member, "chat", "hello", i);
            let message = social_message(room, epoch, &SocialPayload::Message(entry), i);
            assert!(!replica.install(&message, i).unwrap());
            assert!(!replica.install(&message, i).unwrap());
        }
        assert!(replica.history.len() <= HISTORY_MAX_COUNT);
        assert_eq!(replica.sequence, 500);
        let snapshot = social_message(
            room,
            epoch,
            &SocialPayload::Snapshot {
                sequence: state.sequence,
                entries: state.history.iter().cloned().collect(),
            },
            500,
        );
        replica.install(&snapshot, 500).unwrap();
        assert_eq!(replica.history.len(), state.history.len());
        let mut entry = state.entry(Uuid::new_v4(), &member, "chat", "gap", 501);
        entry.social_sequence += 1;
        assert!(
            replica
                .install(
                    &social_message(room, epoch, &SocialPayload::Message(entry), 501),
                    501
                )
                .unwrap()
        );
        assert_eq!(replica.sequence, 500);
        for _ in 0..1000 {
            let m = social_message(
                room,
                epoch,
                &SocialPayload::Reaction {
                    reaction_id: Uuid::new_v4(),
                    sender_id: member.member_id,
                    emoji: "😂".into(),
                    sent_at_ms: 600,
                },
                600,
            );
            replica.install(&m, 600).unwrap();
            replica.install(&m, 600).unwrap();
        }
        assert_eq!(replica.reactions.len(), 32);
        assert_eq!(replica.dropped_reactions, 968);
        assert_eq!(replica.summary(3600)["reactions"], json!([]));
        replica.install(&snapshot, 700).unwrap();
        assert!(replica.reactions.is_empty());
        assert_eq!(replica.history.len(), state.history.len());
        replica.disconnected();
        assert!(replica.reactions.is_empty());
    }
}
