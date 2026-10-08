use crate::model::*;
use crate::social::{REACTIONS, SocialPayload, SocialState, validate_text};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use cine_core::playback::{PlaybackStatus, Timeline};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub struct ServiceConfig {
    pub lead_ms: u64,
    pub lease_ms: u64,
    pub max_rooms: usize,
}
impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            lead_ms: 500,
            lease_ms: 30_000,
            max_rooms: 128,
        }
    }
}
struct Room {
    social: SocialState,
    state: RoomState,
    invite: [u8; 32],
    connections: HashMap<Uuid, Uuid>,
    resume: HashMap<Uuid, [u8; 32]>,
}
#[derive(Default)]
pub struct RoomStore {
    rooms: HashMap<Uuid, Room>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Scope {
    Connection(Uuid),
    Member(Uuid, Uuid, Uuid),
}
struct Cached {
    request: Request,
    result: Effect,
    expires: u64,
}
pub struct RoomService {
    store: RoomStore,
    bindings: HashMap<Uuid, (Uuid, Uuid)>,
    cache: HashMap<(Scope, Uuid), Cached>,
    config: ServiceConfig,
}
fn hash_token(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}
fn new_token() -> Result<(String, [u8; 32]), ErrorCode> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes).map_err(|_| ErrorCode::InternalError)?;
    let token = URL_SAFE_NO_PAD.encode(bytes);
    let verifier = hash_token(&token);
    Ok((token, verifier))
}
fn private(connection: Uuid, effect: Effect) -> Delivery {
    Delivery {
        recipients: vec![connection],
        effect,
    }
}
fn member(id: Uuid, name: String, role: Role, now: u64) -> Member {
    Member {
        member_id: id,
        display_name: name,
        role,
        connected: true,
        ready: false,
        verified_media_revision: None,
        status: MemberStatus::Idle,
        joined_at_ms: now,
        lease_expires_at_ms: None,
    }
}
impl Room {
    fn recipients(&self) -> Vec<Uuid> {
        self.connections.values().copied().collect()
    }
    fn event(
        &self,
        kind: EventKind,
        actor: Option<Uuid>,
        member: Option<Uuid>,
        reason: Option<&'static str>,
        command: Option<Uuid>,
        old_host: Option<Uuid>,
    ) -> Delivery {
        Delivery {
            recipients: self.recipients(),
            effect: Effect::Event(RoomEvent {
                kind,
                state: self.state.clone(),
                actor_id: actor,
                member_id: member,
                reason,
                command_event_id: command,
                previous_host_id: old_host,
            }),
        }
    }
    fn social_delivery(&self, recipients: Vec<Uuid>, payload: SocialPayload) -> Delivery {
        Delivery {
            recipients,
            effect: Effect::Social {
                room_id: self.state.room_id,
                room_epoch: self.state.room_epoch,
                payload,
            },
        }
    }
    fn changed(&mut self, now: u64) {
        self.state.sequence += 1;
        self.state.updated_at_ms = now;
    }
}
impl Default for RoomService {
    fn default() -> Self {
        Self::new(ServiceConfig::default())
    }
}
impl RoomService {
    pub fn new(config: ServiceConfig) -> Self {
        Self {
            store: RoomStore::default(),
            bindings: HashMap::new(),
            cache: HashMap::new(),
            config,
        }
    }
    pub fn state(&self, room: Uuid) -> Option<&RoomState> {
        self.store.rooms.get(&room).map(|r| &r.state)
    }
    pub fn binding(&self, connection: Uuid) -> Option<(Uuid, Uuid)> {
        self.bindings.get(&connection).copied()
    }
    pub fn execute(&mut self, connection: Uuid, request: Request, now: u64) -> Vec<Delivery> {
        self.cache.retain(|_, v| now < v.expires);
        let preentry = matches!(
            request.command,
            Command::Create { .. } | Command::Join { .. } | Command::Resume { .. }
        );
        let scope = if preentry || matches!(request.command, Command::Leave) {
            Scope::Connection(connection)
        } else {
            match self.bindings.get(&connection) {
                Some((id, member)) => {
                    let room = &self.store.rooms[id];
                    Scope::Member(*id, room.state.room_epoch, *member)
                }
                None => {
                    return vec![private(
                        connection,
                        Effect::Error {
                            request_event_id: request.event_id,
                            code: ErrorCode::NotAuthorized,
                        },
                    )];
                }
            }
        };
        if let Some(cached) = self.cache.get(&(scope, request.event_id)) {
            if cached.request != request {
                return vec![private(
                    connection,
                    Effect::Error {
                        request_event_id: request.event_id,
                        code: ErrorCode::InvalidEvent,
                    },
                )];
            }
            let mut deliveries = vec![private(connection, cached.result.clone())];
            if let Some((id, _)) = self.binding(connection) {
                let room = self.store.rooms.get_mut(&id).unwrap();
                if let Some(p) = &mut room.state.playback {
                    p.normalize(now);
                }
                deliveries.push(private(connection, Effect::Snapshot(room.state.clone())));
            }
            self.decorate_social(&mut deliveries, None, now);
            return deliveries;
        }
        if self.cache.len() >= 65_536
            || self.cache.keys().filter(|(s, _)| *s == scope).count() >= 256
        {
            return vec![private(
                connection,
                Effect::Error {
                    request_event_id: request.event_id,
                    code: ErrorCode::RateLimited,
                },
            )];
        }
        let previous = self.binding(connection).and_then(|(id, mid)| {
            self.store.rooms[&id]
                .state
                .members
                .iter()
                .find(|m| m.member_id == mid)
                .cloned()
        });
        let result = self.apply(connection, &request, now);
        let mut deliveries = match result {
            Ok(d) => d,
            Err(code) => vec![private(
                connection,
                Effect::Error {
                    request_event_id: request.event_id,
                    code,
                },
            )],
        };
        if !matches!(request.command, Command::Sync) {
            let outcome = deliveries
                .iter()
                .find(|d| matches!(d.effect, Effect::Ack { .. } | Effect::Error { .. }))
                .unwrap()
                .effect
                .clone();
            self.cache.insert(
                (scope, request.event_id),
                Cached {
                    request,
                    result: outcome,
                    expires: now + 120_000,
                },
            );
        }
        self.decorate_social(&mut deliveries, previous.as_ref(), now);
        // Requests consume no transport resources; only returned effects leave the service.
        deliveries.shrink_to_fit();
        deliveries
    }
    fn apply(
        &mut self,
        connection: Uuid,
        req: &Request,
        now: u64,
    ) -> Result<Vec<Delivery>, ErrorCode> {
        use ErrorCode::*;
        if let Command::Create { display_name } = &req.command {
            if self.binding(connection).is_some() {
                return Err(InvalidState);
            }
            if self.store.rooms.len() >= self.config.max_rooms {
                return Err(RoomLimitReached);
            }
            let (invite_token, invite) = new_token()?;
            let (resume_token, verifier) = new_token()?;
            let id = Uuid::new_v4();
            let epoch = Uuid::new_v4();
            let host = Uuid::new_v4();
            let state = RoomState {
                room_id: id,
                room_epoch: epoch,
                sequence: 1,
                updated_at_ms: now,
                host_id: host,
                authority_revision: 1,
                members: vec![member(host, display_name.clone(), Role::Host, now)],
                media: None,
                playback: None,
            };
            self.store.rooms.insert(
                id,
                Room {
                    social: SocialState::default(),
                    state: state.clone(),
                    invite,
                    connections: HashMap::from([(host, connection)]),
                    resume: HashMap::from([(host, verifier)]),
                },
            );
            self.bindings.insert(connection, (id, host));
            return Ok(vec![
                private(
                    connection,
                    Effect::Ack {
                        request_event_id: req.event_id,
                        sequence: Some(1),
                        result: AckResult::Credentials(Credentials {
                            room_id: id,
                            room_epoch: epoch,
                            member_id: host,
                            invite_token: Some(invite_token),
                            resume_token,
                            lease_ms: self.config.lease_ms,
                        }),
                    },
                ),
                private(connection, Effect::Snapshot(state)),
            ]);
        }
        let id = req.room_id.ok_or(InvalidEvent)?;
        let room = self.store.rooms.get_mut(&id).ok_or(RoomNotFound)?;
        if req.room_epoch != Some(room.state.room_epoch) {
            return Err(InvalidEvent);
        }
        if let Some(p) = &mut room.state.playback {
            p.normalize(now);
        }
        if let Command::Join {
            invite_token,
            display_name,
        } = &req.command
        {
            if self.bindings.contains_key(&connection) {
                return Err(InvalidState);
            }
            if room.invite != hash_token(invite_token) {
                return Err(InviteInvalid);
            }
            if room.state.members.len() >= 16 {
                return Err(RoomFull);
            }
            let (resume_token, verifier) = new_token()?;
            let mid = Uuid::new_v4();
            room.state
                .members
                .push(member(mid, display_name.clone(), Role::Participant, now));
            room.resume.insert(mid, verifier);
            room.connections.insert(mid, connection);
            self.bindings.insert(connection, (id, mid));
            room.changed(now);
            return Ok(vec![
                private(
                    connection,
                    Effect::Ack {
                        request_event_id: req.event_id,
                        sequence: Some(room.state.sequence),
                        result: AckResult::Credentials(Credentials {
                            room_id: id,
                            room_epoch: room.state.room_epoch,
                            member_id: mid,
                            invite_token: None,
                            resume_token,
                            lease_ms: self.config.lease_ms,
                        }),
                    },
                ),
                private(connection, Effect::Snapshot(room.state.clone())),
                room.event(
                    EventKind::MemberJoined,
                    Some(mid),
                    Some(mid),
                    None,
                    None,
                    None,
                ),
            ]);
        }
        if let Command::Resume { resume_token, .. } = &req.command {
            if self.bindings.contains_key(&connection) {
                return Err(InvalidState);
            }
            let verifier = hash_token(resume_token);
            let mid = room
                .resume
                .iter()
                .find(|(_, v)| **v == verifier)
                .map(|(id, _)| *id)
                .ok_or(ResumeExpired)?;
            let m = room
                .state
                .members
                .iter_mut()
                .find(|m| m.member_id == mid)
                .ok_or(ResumeExpired)?;
            if m.lease_expires_at_ms
                .is_some_and(|deadline| now >= deadline)
            {
                return Err(ResumeExpired);
            }
            let (token, new_verifier) = new_token()?;
            let old = room.connections.insert(mid, connection);
            m.connected = true;
            m.ready = false;
            m.status = MemberStatus::Idle;
            m.lease_expires_at_ms = None;
            room.resume.insert(mid, new_verifier);
            self.bindings.insert(connection, (id, mid));
            room.changed(now);
            let mut effects = vec![
                private(
                    connection,
                    Effect::Ack {
                        request_event_id: req.event_id,
                        sequence: Some(room.state.sequence),
                        result: AckResult::Credentials(Credentials {
                            room_id: id,
                            room_epoch: room.state.room_epoch,
                            member_id: mid,
                            invite_token: None,
                            resume_token: token,
                            lease_ms: self.config.lease_ms,
                        }),
                    },
                ),
                private(connection, Effect::Snapshot(room.state.clone())),
                room.event(
                    EventKind::MemberStatus,
                    Some(mid),
                    Some(mid),
                    Some("resumed"),
                    None,
                    None,
                ),
            ];
            if let Some(old) = old {
                self.bindings.remove(&old);
                effects.push(private(old, Effect::Close));
            }
            return Ok(effects);
        }
        let (bound_id, mid) = self
            .bindings
            .get(&connection)
            .copied()
            .ok_or(NotAuthorized)?;
        if bound_id != id || room.connections.get(&mid) != Some(&connection) {
            return Err(NotAuthorized);
        }
        let host_command = matches!(
            req.command,
            Command::Select { .. }
                | Command::Play { .. }
                | Command::Pause { .. }
                | Command::Seek { .. }
                | Command::Transfer { .. }
        );
        if host_command && mid != room.state.host_id {
            return Err(NotAuthorized);
        }
        let admin = match &req.command {
            Command::Select { context, .. } | Command::Transfer { context, .. } => Some(*context),
            Command::Play { context, .. }
            | Command::Pause { context }
            | Command::Seek { context, .. } => Some(context.admin),
            _ => None,
        };
        if let Some(ctx) = admin {
            if ctx.authority_revision != room.state.authority_revision {
                return Err(StaleAuthority);
            }
            if ctx.expected_sequence != room.state.sequence {
                return Err(OutOfSequence);
            }
        }
        let mut kind = None;
        let mut reason = None;
        let mut old_host = None;
        match &req.command {
            Command::Chat { text } => {
                let text = validate_text(text)?;
                if !room.social.allow(mid, false, now) {
                    return Err(RateLimited);
                }
                let member = room
                    .state
                    .members
                    .iter()
                    .find(|m| m.member_id == mid)
                    .unwrap();
                let entry = room.social.entry(Uuid::new_v4(), member, "chat", text, now);
                return Ok(vec![
                    private(
                        connection,
                        Effect::Ack {
                            request_event_id: req.event_id,
                            sequence: None,
                            result: AckResult::Empty,
                        },
                    ),
                    room.social_delivery(room.recipients(), SocialPayload::Message(entry)),
                ]);
            }
            Command::React { emoji } => {
                if !REACTIONS.contains(&emoji.as_str()) {
                    return Err(InvalidEvent);
                }
                if !room.social.allow(mid, true, now) {
                    return Err(RateLimited);
                }
                return Ok(vec![
                    private(
                        connection,
                        Effect::Ack {
                            request_event_id: req.event_id,
                            sequence: None,
                            result: AckResult::Empty,
                        },
                    ),
                    room.social_delivery(
                        room.recipients(),
                        SocialPayload::Reaction {
                            reaction_id: Uuid::new_v4(),
                            sender_id: mid,
                            emoji: emoji.clone(),
                            sent_at_ms: now,
                        },
                    ),
                ]);
            }
            Command::Sync => {
                return Ok(vec![private(
                    connection,
                    Effect::Snapshot(room.state.clone()),
                )]);
            }
            Command::Leave => {
                let mut effects = vec![private(
                    connection,
                    Effect::Ack {
                        request_event_id: req.event_id,
                        sequence: Some(room.state.sequence + 1),
                        result: AckResult::Empty,
                    },
                )];
                room.changed(now);
                if mid == room.state.host_id {
                    effects.push(room.event(
                        EventKind::RoomClosed,
                        Some(mid),
                        None,
                        Some("host_left"),
                        None,
                        None,
                    ));
                    let recipients = room.recipients();
                    for c in &recipients {
                        self.bindings.remove(c);
                    }
                    effects.extend(recipients.into_iter().map(|c| private(c, Effect::Close)));
                    self.store.rooms.remove(&id);
                } else {
                    effects.push(room.event(
                        EventKind::MemberLeft,
                        Some(mid),
                        Some(mid),
                        Some("left"),
                        None,
                        None,
                    ));
                    room.state.members.retain(|m| m.member_id != mid);
                    // The broadcast must describe the state after removal.
                    if let Effect::Event(event) = &mut effects[1].effect {
                        event.state = room.state.clone();
                    }
                    room.connections.remove(&mid);
                    room.resume.remove(&mid);
                    self.bindings.remove(&connection);
                }
                return Ok(effects);
            }
            Command::Select {
                previous_media_revision,
                descriptor,
                ..
            } => {
                if *previous_media_revision
                    != room.state.media.as_ref().map_or(0, |m| m.media_revision)
                {
                    return Err(StaleMedia);
                }
                if !descriptor.valid_local() {
                    return Err(InvalidMedia);
                }
                if let Some(p) = &room.state.playback {
                    if p.pending.is_some() {
                        return Err(ControlPending);
                    }
                    if p.current.status == PlaybackStatus::Playing {
                        return Err(InvalidState);
                    }
                }
                let revision = previous_media_revision + 1;
                room.state.media = Some(MediaSelection {
                    media_revision: revision,
                    descriptor: descriptor.clone(),
                });
                room.state.playback = Some(Playback {
                    current: Timeline {
                        status: PlaybackStatus::Paused,
                        position_ms: 0,
                        anchor_time_ms: now as i64,
                        rate: 1.0,
                        duration_ms: descriptor.duration_ms,
                    },
                    pending: None,
                });
                for m in &mut room.state.members {
                    m.ready = false;
                    m.verified_media_revision = None;
                    m.status = MemberStatus::Loading;
                }
                kind = Some(EventKind::MediaSelected);
            }
            Command::Metadata {
                media_revision,
                identity,
                duration_ms,
            } => {
                let selected = room.state.media.as_ref().ok_or(NoMedia)?;
                if *media_revision != selected.media_revision {
                    return Err(StaleMedia);
                }
                if !identity.matches(&selected.descriptor.identity)
                    || duration_ms.abs_diff(selected.descriptor.duration_ms) > 1_000
                {
                    return Err(MediaMismatch);
                }
                let m = room
                    .state
                    .members
                    .iter_mut()
                    .find(|m| m.member_id == mid)
                    .unwrap();
                if m.verified_media_revision != Some(*media_revision) {
                    m.verified_media_revision = Some(*media_revision);
                    kind = Some(EventKind::MediaVerified);
                }
            }
            Command::Ready {
                media_revision,
                clock_uncertainty_ms,
            } => {
                if room.state.media.as_ref().ok_or(NoMedia)?.media_revision != *media_revision {
                    return Err(StaleMedia);
                }
                if *clock_uncertainty_ms > 100 {
                    return Err(ClockUncertain);
                }
                let m = room
                    .state
                    .members
                    .iter_mut()
                    .find(|m| m.member_id == mid)
                    .unwrap();
                if m.verified_media_revision != Some(*media_revision) {
                    return Err(MediaNotVerified);
                }
                if !m.ready {
                    m.ready = true;
                    m.status = MemberStatus::Ready;
                    kind = Some(EventKind::MediaReadiness);
                    reason = Some("ready");
                }
            }
            Command::NotReady {
                media_revision,
                reason: r,
            } => {
                if room.state.media.as_ref().ok_or(NoMedia)?.media_revision != *media_revision {
                    return Err(StaleMedia);
                }
                let status = match r {
                    NotReadyReason::Buffering => MemberStatus::Buffering,
                    NotReadyReason::PlayerError => MemberStatus::Error,
                    _ => MemberStatus::Loading,
                };
                let m = room
                    .state
                    .members
                    .iter_mut()
                    .find(|m| m.member_id == mid)
                    .unwrap();
                if m.ready || m.status != status {
                    m.ready = false;
                    m.status = status;
                    kind = Some(EventKind::MediaReadiness);
                    reason = Some(r.as_str());
                }
            }
            Command::Play { context, .. }
            | Command::Pause { context }
            | Command::Seek { context, .. } => {
                let selected = room.state.media.as_ref().ok_or(NoMedia)?;
                if context.media_revision != selected.media_revision {
                    return Err(StaleMedia);
                }
                let p = room.state.playback.as_mut().ok_or(NoMedia)?;
                if p.pending.is_some() {
                    return Err(ControlPending);
                }
                let execute_at = now + self.config.lead_ms;
                let mut next = p.current;
                next.anchor_time_ms = execute_at as i64;
                match &req.command {
                    Command::Play { position_ms, .. } => {
                        if room.state.members.iter().any(|m| {
                            m.connected
                                && (!m.ready
                                    || m.verified_media_revision != Some(selected.media_revision))
                        }) {
                            return Err(MediaNotReady);
                        }
                        next.status = PlaybackStatus::Playing;
                        next.position_ms = *position_ms;
                        kind = Some(EventKind::Play);
                    }
                    Command::Pause { .. } => {
                        next.status = PlaybackStatus::Paused;
                        next.position_ms = p.current.position_at(execute_at as i64);
                        kind = Some(EventKind::Pause);
                    }
                    Command::Seek { position_ms, .. } => {
                        next.position_ms = *position_ms;
                        kind = Some(EventKind::Seek);
                    }
                    _ => unreachable!(),
                }
                if next.position_ms > next.duration_ms {
                    return Err(PositionOutOfRange);
                }
                p.pending = Some(ScheduledTransition {
                    command_event_id: Some(req.event_id),
                    sequence: room.state.sequence + 1,
                    authority_revision: room.state.authority_revision,
                    media_revision: selected.media_revision,
                    execute_at_ms: execute_at,
                    timeline_after: next,
                });
            }
            Command::Transfer {
                target_member_id, ..
            } => {
                if let Some(p) = &room.state.playback {
                    if p.pending.is_some() {
                        return Err(ControlPending);
                    }
                    if p.current.status == PlaybackStatus::Playing {
                        return Err(InvalidState);
                    }
                }
                let target = room
                    .state
                    .members
                    .iter()
                    .find(|m| m.member_id == *target_member_id && m.connected)
                    .ok_or(MemberNotFound)?;
                if room.state.media.is_some() && !target.ready {
                    return Err(MediaNotReady);
                }
                if *target_member_id != mid {
                    old_host = Some(mid);
                    room.state.host_id = *target_member_id;
                    room.state.authority_revision += 1;
                    for m in &mut room.state.members {
                        m.role = if m.member_id == *target_member_id {
                            Role::Host
                        } else {
                            Role::Participant
                        };
                    }
                    kind = Some(EventKind::HostTransferred);
                }
            }
            _ => return Err(InvalidEvent),
        }
        if kind.is_some() {
            room.changed(now);
        }
        let mut effects = vec![private(
            connection,
            Effect::Ack {
                request_event_id: req.event_id,
                sequence: Some(room.state.sequence),
                result: AckResult::Empty,
            },
        )];
        if let Some(kind) = kind {
            let command = if matches!(kind, EventKind::Play | EventKind::Pause | EventKind::Seek) {
                Some(req.event_id)
            } else {
                None
            };
            effects.push(room.event(kind, Some(mid), Some(mid), reason, command, old_host));
        }
        Ok(effects)
    }
    pub fn disconnect(&mut self, connection: Uuid, now: u64) -> Vec<Delivery> {
        let Some((id, mid)) = self.bindings.remove(&connection) else {
            return vec![];
        };
        let Some(room) = self.store.rooms.get_mut(&id) else {
            return vec![];
        };
        if room.connections.remove(&mid) != Some(connection) {
            return vec![];
        }
        if let Some(p) = &mut room.state.playback {
            p.normalize(now);
        }
        let m = room
            .state
            .members
            .iter_mut()
            .find(|m| m.member_id == mid)
            .unwrap();
        m.connected = false;
        m.ready = false;
        m.status = MemberStatus::Idle;
        m.lease_expires_at_ms = Some(now + self.config.lease_ms);
        room.changed(now);
        if mid == room.state.host_id
            && let (Some(media), Some(p)) = (&room.state.media, &mut room.state.playback)
        {
            let execute_at = now + self.config.lead_ms;
            let mut next = p.current;
            next.status = PlaybackStatus::Paused;
            next.position_ms = p.current.position_at(execute_at as i64);
            next.anchor_time_ms = execute_at as i64;
            p.pending = Some(ScheduledTransition {
                command_event_id: None,
                sequence: room.state.sequence,
                authority_revision: room.state.authority_revision,
                media_revision: media.media_revision,
                execute_at_ms: execute_at,
                timeline_after: next,
            });
        }
        vec![room.event(
            EventKind::MemberStatus,
            None,
            Some(mid),
            Some("disconnected"),
            None,
            None,
        )]
    }
    fn decorate_social(
        &mut self,
        deliveries: &mut Vec<Delivery>,
        previous: Option<&Member>,
        now: u64,
    ) {
        let mut extra = vec![];
        for d in deliveries.iter() {
            if let Effect::Event(e) = &d.effect {
                let kind = match e.kind {
                    EventKind::MemberJoined => Some("joined"),
                    EventKind::MemberLeft => Some("left"),
                    EventKind::MemberStatus if e.reason == Some("resumed") => Some("resumed"),
                    _ => None,
                };
                if let (Some(kind), Some(mid), Some(room)) = (
                    kind,
                    e.member_id,
                    self.store.rooms.get_mut(&e.state.room_id),
                ) {
                    if let Some(member) = e
                        .state
                        .members
                        .iter()
                        .find(|m| m.member_id == mid)
                        .or(previous.filter(|m| m.member_id == mid))
                        && let Some(entry) = room.social.presence(member, kind, now)
                    {
                        extra.push(
                            room.social_delivery(
                                d.recipients.clone(),
                                SocialPayload::Message(entry),
                            ),
                        );
                    }
                    if kind == "left" {
                        room.social.forget(mid);
                    }
                }
            }
        }
        for d in deliveries.iter() {
            if let Effect::Snapshot(st) = &d.effect
                && let Some(room) = self.store.rooms.get(&st.room_id)
            {
                extra.insert(
                    0,
                    room.social_delivery(
                        d.recipients.clone(),
                        SocialPayload::Snapshot {
                            sequence: room.social.sequence,
                            entries: room.social.history.iter().cloned().collect(),
                        },
                    ),
                );
            }
        }
        deliveries.extend(extra);
    }
    pub fn tick(&mut self, now: u64) -> Vec<Delivery> {
        self.cache.retain(|_, v| now < v.expires);
        let mut deliveries = vec![];
        let ids: Vec<_> = self.store.rooms.keys().copied().collect();
        for id in ids {
            let room = self.store.rooms.get_mut(&id).unwrap();
            if let Some(p) = &mut room.state.playback {
                p.normalize(now);
            }
            let expired: Vec<_> = room
                .state
                .members
                .iter()
                .filter(|m| m.lease_expires_at_ms.is_some_and(|d| now >= d))
                .map(|m| m.member_id)
                .collect();
            if expired.contains(&room.state.host_id) {
                room.changed(now);
                deliveries.push(room.event(
                    EventKind::RoomClosed,
                    None,
                    None,
                    Some("host_expired"),
                    None,
                    None,
                ));
                let recipients = room.recipients();
                for c in recipients {
                    self.bindings.remove(&c);
                    deliveries.push(private(c, Effect::Close));
                }
                self.store.rooms.remove(&id);
                continue;
            }
            for mid in expired {
                if let Some(member) = room.state.members.iter().find(|m| m.member_id == mid) {
                    let entry = room.social.entry(Uuid::new_v4(), member, "left", "", now);
                    deliveries.push(
                        room.social_delivery(room.recipients(), SocialPayload::Message(entry)),
                    );
                }
                room.social.forget(mid);
                room.state.members.retain(|m| m.member_id != mid);
                room.resume.remove(&mid);
                room.changed(now);
                deliveries.push(room.event(
                    EventKind::MemberLeft,
                    None,
                    Some(mid),
                    Some("expired"),
                    None,
                    None,
                ));
            }
        }
        deliveries
    }
    pub fn sync_states(&mut self, now: u64) -> Vec<Delivery> {
        self.store
            .rooms
            .values_mut()
            .map(|room| {
                if let Some(p) = &mut room.state.playback {
                    p.normalize(now);
                }
                room.event(EventKind::SyncState, None, None, None, None, None)
            })
            .collect()
    }
}
