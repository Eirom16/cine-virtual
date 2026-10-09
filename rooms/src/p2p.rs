//! Independent control-plane state; no binary data and no playback sequence.
use crate::model::{ErrorCode, RoomState};
use cine_transfer_model::{Credential, Grant, Offer, Receiver, TransferIntent, TransferSnapshot};
use std::collections::HashMap;
use uuid::Uuid;
#[derive(Default)]
pub struct Transfers {
    offer: Option<Offer>,
    receivers: HashMap<Uuid, Receiver>,
    grants: HashMap<Uuid, Grant>,
}
impl Transfers {
    fn valid(&self, room: &RoomState) -> bool {
        self.offer.as_ref().is_some_and(|o| {
            room.media.as_ref().is_some_and(|m| {
                o.manifest.room_id == room.room_id
                    && o.manifest.room_epoch == room.room_epoch
                    && o.manifest.host_id == room.host_id
                    && o.manifest.media_revision == m.media_revision
                    && o.manifest.authority_revision == room.authority_revision
                    && o.manifest.media_id.to_string() == m.descriptor.media_id
                    && o.manifest.identity() == m.descriptor.identity
            })
        })
    }
    pub fn reconcile(&mut self, room: &RoomState, now: u64) {
        if !self.valid(room) {
            self.offer = None;
            self.receivers.clear();
            self.grants.clear();
        }
        self.grants.retain(|mid, g| {
            now < g.expires_at_ms
                && room
                    .members
                    .iter()
                    .any(|m| m.member_id == *mid && m.connected)
                && room
                    .members
                    .iter()
                    .any(|m| m.member_id == room.host_id && m.connected)
        });
        for receiver in self.receivers.values_mut() {
            receiver.authorized = self.grants.contains_key(&receiver.receiver_id);
            if !receiver.authorized
                && ["connecting", "transferring", "verifying"].contains(&receiver.state.as_str())
            {
                receiver.state = "reconnecting".into();
            }
        }
        self.receivers
            .retain(|mid, _| room.members.iter().any(|m| m.member_id == *mid));
    }
    pub fn snapshot(&self) -> TransferSnapshot {
        let mut receivers: Vec<_> = self.receivers.values().cloned().collect();
        receivers.sort_by_key(|r| r.receiver_id);
        TransferSnapshot {
            offer: self.offer.clone(),
            receivers,
        }
    }
    pub fn apply(
        &mut self,
        room: &RoomState,
        actor: Uuid,
        intent: &TransferIntent,
        now: u64,
    ) -> Result<Option<Grant>, ErrorCode> {
        intent.validate().map_err(|_| ErrorCode::InvalidEvent)?;
        self.reconcile(room, now);
        let host = room.host_id;
        let is_host = actor == host;
        if !room
            .members
            .iter()
            .any(|m| m.member_id == actor && m.connected)
            || !room
                .members
                .iter()
                .any(|m| m.member_id == host && m.connected)
        {
            return Err(ErrorCode::NotAuthorized);
        }
        if let TransferIntent::Offer {
            manifest,
            address,
            certificate,
        } = intent
        {
            if !is_host {
                return Err(ErrorCode::NotAuthorized);
            }
            let Some(media) = &room.media else {
                return Err(ErrorCode::NoMedia);
            };
            if manifest.room_id != room.room_id
                || manifest.room_epoch != room.room_epoch
                || manifest.host_id != host
                || manifest.authority_revision != room.authority_revision
            {
                return Err(ErrorCode::NotAuthorized);
            }
            if manifest.media_revision != media.media_revision
                || manifest.media_id.to_string() != media.descriptor.media_id
                || manifest.identity() != media.descriptor.identity
                || manifest.duration_ms != media.descriptor.duration_ms
            {
                return Err(ErrorCode::StaleMedia);
            }
            if !self.grants.is_empty() {
                return Err(ErrorCode::InvalidState);
            }
            self.offer = Some(Offer {
                manifest: *manifest.clone(),
                address: *address,
                certificate: certificate.clone(),
            });
            self.receivers.clear();
            return Ok(None);
        }
        let transfer_id = match intent {
            TransferIntent::Request { transfer_id }
            | TransferIntent::Accept { transfer_id, .. }
            | TransferIntent::Reject { transfer_id, .. }
            | TransferIntent::Cancel { transfer_id }
            | TransferIntent::Withdraw { transfer_id }
            | TransferIntent::Status { transfer_id, .. } => *transfer_id,
            _ => unreachable!(),
        };
        let offer = self.offer.as_ref().ok_or(ErrorCode::NoMedia)?;
        if transfer_id != offer.manifest.transfer_id {
            return Err(ErrorCode::StaleMedia);
        }
        match intent {
            TransferIntent::Request { .. } => {
                if is_host {
                    return Err(ErrorCode::InvalidState);
                }
                if self
                    .receivers
                    .get(&actor)
                    .is_some_and(|r| r.state == "completed")
                {
                    return Err(ErrorCode::InvalidState);
                }
                self.grants.remove(&actor);
                self.receivers.insert(
                    actor,
                    Receiver {
                        authorized: false,
                        receiver_id: actor,
                        state: "waiting_for_acceptance".into(),
                        verified_bytes: self.receivers.get(&actor).map_or(0, |r| r.verified_bytes),
                    },
                );
            }
            TransferIntent::Accept { receiver_id, .. } => {
                if !is_host {
                    return Err(ErrorCode::NotAuthorized);
                }
                if !self.grants.is_empty() {
                    return Err(ErrorCode::RateLimited);
                }
                let receiver = self
                    .receivers
                    .get_mut(receiver_id)
                    .ok_or(ErrorCode::InvalidState)?;
                if receiver.state != "waiting_for_acceptance"
                    || !room
                        .members
                        .iter()
                        .any(|m| m.member_id == *receiver_id && m.connected)
                {
                    return Err(ErrorCode::NotAuthorized);
                }
                let credential = Credential::new(&offer.manifest, *receiver_id)
                    .map_err(|_| ErrorCode::InternalError)?;
                let grant = Grant {
                    credential,
                    expires_at_ms: now + 600_000,
                };
                self.grants.insert(*receiver_id, grant.clone());
                receiver.state = "connecting".into();
                receiver.authorized = true;
                return Ok(Some(grant));
            }
            TransferIntent::Reject { receiver_id, .. } => {
                if !is_host {
                    return Err(ErrorCode::NotAuthorized);
                }
                self.grants.remove(receiver_id);
                self.receivers.remove(receiver_id);
            }
            TransferIntent::Cancel { .. } => {
                self.grants.remove(&actor);
                self.receivers.remove(&actor);
            }
            TransferIntent::Withdraw { .. } => {
                if !is_host {
                    return Err(ErrorCode::NotAuthorized);
                }
                self.offer = None;
                self.grants.clear();
                self.receivers.clear();
            }
            TransferIntent::Status {
                state,
                verified_bytes,
                ..
            } => {
                let receiver = self
                    .receivers
                    .get_mut(&actor)
                    .ok_or(ErrorCode::NotAuthorized)?;
                if *verified_bytes > offer.manifest.size_bytes {
                    return Err(ErrorCode::InvalidEvent);
                }
                if state == "transferring" && !self.grants.contains_key(&actor) {
                    return Err(ErrorCode::NotAuthorized);
                }
                receiver.state = state.clone();
                receiver.verified_bytes = *verified_bytes;
                if ["paused", "reconnecting", "completed", "failed"].contains(&state.as_str()) {
                    self.grants.remove(&actor);
                }
            }
            _ => unreachable!(),
        }
        Ok(None)
    }
    pub fn grant(&self, receiver: Uuid) -> Option<&Grant> {
        self.grants.get(&receiver)
    }
}
