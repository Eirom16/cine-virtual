use cine_core::{
    media::{ContentIdentity, MediaDescriptor, SourceType},
    playback::PlaybackStatus,
};
use cine_rooms::{RoomService, model::*};
use uuid::Uuid;

struct Fixture {
    service: RoomService,
    host: Uuid,
    room: Uuid,
    epoch: Uuid,
    invite: String,
}
fn credentials(d: &[Delivery]) -> Credentials {
    d.iter()
        .find_map(|d| match &d.effect {
            Effect::Ack {
                result: AckResult::Credentials(c),
                ..
            } => Some(c.clone()),
            _ => None,
        })
        .expect("credentials")
}
fn error(d: &[Delivery]) -> Option<ErrorCode> {
    d.iter().find_map(|d| {
        if let Effect::Error { code, .. } = d.effect {
            Some(code)
        } else {
            None
        }
    })
}
fn descriptor() -> MediaDescriptor {
    MediaDescriptor {
        media_id: Uuid::new_v4().to_string(),
        source_type: SourceType::LocalFile,
        title: None,
        duration_ms: 300_000,
        identity: ContentIdentity {
            size_bytes: 1024,
            sha256: [0xaa; 32],
        },
        mime: None,
        codecs: vec![],
    }
}
impl Fixture {
    fn new() -> Self {
        let mut service = RoomService::default();
        let host = Uuid::new_v4();
        let d = service.execute(
            host,
            Request {
                event_id: Uuid::new_v4(),
                room_id: None,
                room_epoch: None,
                payload_fingerprint: None,
                command: Command::Create {
                    display_name: "Host".into(),
                },
            },
            0,
        );
        let c = credentials(&d);
        Self {
            service,
            host,
            room: c.room_id,
            epoch: c.room_epoch,
            invite: c.invite_token.unwrap(),
        }
    }
    fn state(&self) -> &RoomState {
        self.service.state(self.room).unwrap()
    }
    fn req(&self, command: Command) -> Request {
        Request {
            event_id: Uuid::new_v4(),
            room_id: Some(self.room),
            room_epoch: Some(self.epoch),
            payload_fingerprint: None,
            command,
        }
    }
    fn send(&mut self, connection: Uuid, command: Command, now: u64) -> Vec<Delivery> {
        let req = self.req(command);
        self.service.execute(connection, req, now)
    }
    fn admin(&self) -> AdminContext {
        AdminContext {
            expected_sequence: self.state().sequence,
            authority_revision: self.state().authority_revision,
        }
    }
    fn control(&self) -> ControlContext {
        ControlContext {
            admin: self.admin(),
            media_revision: self.state().media.as_ref().unwrap().media_revision,
        }
    }
    fn join(&mut self) -> (Uuid, Credentials) {
        let connection = Uuid::new_v4();
        let d = self.send(
            connection,
            Command::Join {
                invite_token: self.invite.clone(),
                display_name: "Participant".into(),
            },
            1,
        );
        (connection, credentials(&d))
    }
    fn select(&mut self) {
        let command = Command::Select {
            context: self.admin(),
            previous_media_revision: 0,
            descriptor: descriptor(),
        };
        assert_eq!(error(&self.send(self.host, command, 10)), None);
    }
    fn ready(&mut self, connection: Uuid) {
        let m = self.state().media.as_ref().unwrap().clone();
        assert_eq!(
            error(&self.send(
                connection,
                Command::Metadata {
                    media_revision: m.media_revision,
                    identity: m.descriptor.identity,
                    duration_ms: m.descriptor.duration_ms
                },
                20
            )),
            None
        );
        assert_eq!(
            error(&self.send(
                connection,
                Command::Ready {
                    media_revision: m.media_revision,
                    clock_uncertainty_ms: 1
                },
                30
            )),
            None
        );
    }
}
#[test]
fn create_join_snapshot_and_roles() {
    let mut f = Fixture::new();
    assert_eq!(f.state().sequence, 1);
    assert_eq!(f.state().members[0].role, Role::Host);
    let (_, c) = f.join();
    assert_eq!(f.state().sequence, 2);
    assert_eq!(f.state().members.len(), 2);
    assert_eq!(f.state().members[1].member_id, c.member_id);
    assert_eq!(f.state().members[1].role, Role::Participant);
}
#[test]
fn room_capacity_counts_disconnected_leases() {
    let mut f = Fixture::new();
    let mut last = f.host;
    for _ in 0..15 {
        last = f.join().0;
    }
    f.service.disconnect(last, 10);
    let sequence = f.state().sequence;
    let command = Command::Join {
        invite_token: f.invite.clone(),
        display_name: "Overflow".into(),
    };
    assert_eq!(
        error(&f.send(Uuid::new_v4(), command, 100)),
        Some(ErrorCode::RoomFull)
    );
    assert_eq!(f.state().sequence, sequence);
}
#[test]
fn participant_cannot_control_and_errors_do_not_increment_sequence() {
    let mut f = Fixture::new();
    let (b, _) = f.join();
    f.select();
    f.ready(f.host);
    f.ready(b);
    let sequence = f.state().sequence;
    let command = Command::Play {
        context: f.control(),
        position_ms: 100_000,
    };
    assert_eq!(
        error(&f.send(b, command, 1000)),
        Some(ErrorCode::NotAuthorized)
    );
    assert_eq!(f.state().sequence, sequence);
}
#[test]
fn readiness_requires_matching_metadata_and_clock() {
    let mut f = Fixture::new();
    f.select();
    assert_eq!(
        error(&f.send(
            f.host,
            Command::Ready {
                media_revision: 1,
                clock_uncertainty_ms: 1
            },
            20
        )),
        Some(ErrorCode::MediaNotVerified)
    );
    let mut identity = f
        .state()
        .media
        .as_ref()
        .unwrap()
        .descriptor
        .identity
        .clone();
    identity.sha256[0] = 0;
    assert_eq!(
        error(&f.send(
            f.host,
            Command::Metadata {
                media_revision: 1,
                identity,
                duration_ms: 300_000
            },
            20
        )),
        Some(ErrorCode::MediaMismatch)
    );
    let identity = f
        .state()
        .media
        .as_ref()
        .unwrap()
        .descriptor
        .identity
        .clone();
    f.send(
        f.host,
        Command::Metadata {
            media_revision: 1,
            identity,
            duration_ms: 300_000,
        },
        20,
    );
    assert_eq!(
        error(&f.send(
            f.host,
            Command::Ready {
                media_revision: 1,
                clock_uncertainty_ms: 101
            },
            20
        )),
        Some(ErrorCode::ClockUncertain)
    );
    f.ready(f.host);
    assert!(f.state().members[0].ready);
}
#[test]
fn play_requires_all_connected_ready() {
    let mut f = Fixture::new();
    f.join();
    f.select();
    f.ready(f.host);
    let sequence = f.state().sequence;
    assert_eq!(
        error(&f.send(
            f.host,
            Command::Play {
                context: f.control(),
                position_ms: 0
            },
            1000
        )),
        Some(ErrorCode::MediaNotReady)
    );
    assert_eq!(f.state().sequence, sequence);
}
#[test]
fn play_pause_seek_are_scheduled_and_normalization_is_not_mutation() {
    let mut f = Fixture::new();
    f.select();
    f.ready(f.host);
    let seq = f.state().sequence;
    f.send(
        f.host,
        Command::Play {
            context: f.control(),
            position_ms: 100_000,
        },
        1000,
    );
    let p = f.state().playback.as_ref().unwrap();
    assert_eq!(p.current.status, PlaybackStatus::Paused);
    assert_eq!(p.pending.as_ref().unwrap().execute_at_ms, 1500);
    assert_eq!(p.timeline_at(1499).status, PlaybackStatus::Paused);
    assert_eq!(p.timeline_at(1500).status, PlaybackStatus::Playing);
    assert_eq!(f.state().sequence, seq + 1);
    f.service.tick(1500);
    assert_eq!(f.state().sequence, seq + 1);
    f.send(
        f.host,
        Command::Pause {
            context: f.control(),
        },
        2000,
    );
    let p = f.state().playback.as_ref().unwrap();
    assert_eq!(
        p.pending.as_ref().unwrap().timeline_after.position_ms,
        101_000
    );
    assert_eq!(p.timeline_at(2499).status, PlaybackStatus::Playing);
    assert_eq!(p.timeline_at(2500).status, PlaybackStatus::Paused);
    f.service.tick(2500);
    f.send(
        f.host,
        Command::Seek {
            context: f.control(),
            position_ms: 120_000,
        },
        3000,
    );
    let p = f.state().playback.as_ref().unwrap();
    assert_eq!(p.timeline_at(3499).position_ms, 101_000);
    assert_eq!(p.timeline_at(3500).position_ms, 120_000);
    assert_eq!(p.timeline_at(3500).status, PlaybackStatus::Paused);
}
#[test]
fn duplicate_control_returns_ack_and_snapshot_without_reexecution() {
    let mut f = Fixture::new();
    f.select();
    f.ready(f.host);
    let req = f.req(Command::Play {
        context: f.control(),
        position_ms: 100_000,
    });
    f.service.execute(f.host, req.clone(), 1000);
    let seq = f.state().sequence;
    let effects = f.service.execute(f.host, req, 1100);
    assert_eq!(f.state().sequence, seq);
    assert!(
        effects
            .iter()
            .any(|d| matches!(d.effect, Effect::Snapshot(_)))
    );
    assert!(!effects.iter().any(|d| matches!(d.effect, Effect::Event(_))));
}
#[test]
fn reused_event_with_different_payload_is_invalid() {
    let mut f = Fixture::new();
    f.select();
    f.ready(f.host);
    let mut req = f.req(Command::Play {
        context: f.control(),
        position_ms: 100_000,
    });
    f.service.execute(f.host, req.clone(), 1000);
    let seq = f.state().sequence;
    if let Command::Play { position_ms, .. } = &mut req.command {
        *position_ms = 120_000;
    }
    assert_eq!(
        error(&f.service.execute(f.host, req, 1100)),
        Some(ErrorCode::InvalidEvent)
    );
    assert_eq!(f.state().sequence, seq);
}
#[test]
fn create_retry_uses_preentry_cache() {
    let mut service = RoomService::default();
    let conn = Uuid::new_v4();
    let req = Request {
        event_id: Uuid::new_v4(),
        room_id: None,
        room_epoch: None,
        payload_fingerprint: None,
        command: Command::Create {
            display_name: "Host".into(),
        },
    };
    let first = credentials(&service.execute(conn, req.clone(), 0));
    let second = credentials(&service.execute(conn, req, 10));
    assert_eq!(first.room_id, second.room_id);
    assert_eq!(first.resume_token, second.resume_token);
}
#[test]
fn stale_sequence_authority_media_and_wrong_epoch_are_rejected() {
    let mut f = Fixture::new();
    f.select();
    f.ready(f.host);
    let seq = f.state().sequence;
    let mut context = f.control();
    context.admin.expected_sequence -= 1;
    assert_eq!(
        error(&f.send(
            f.host,
            Command::Play {
                context,
                position_ms: 0
            },
            1000
        )),
        Some(ErrorCode::OutOfSequence)
    );
    context = f.control();
    context.admin.authority_revision = 0;
    assert_eq!(
        error(&f.send(
            f.host,
            Command::Play {
                context,
                position_ms: 0
            },
            1000
        )),
        Some(ErrorCode::StaleAuthority)
    );
    context = f.control();
    context.media_revision = 0;
    assert_eq!(
        error(&f.send(
            f.host,
            Command::Play {
                context,
                position_ms: 0
            },
            1000
        )),
        Some(ErrorCode::StaleMedia)
    );
    let mut req = f.req(Command::Sync);
    req.room_epoch = Some(Uuid::new_v4());
    assert_eq!(
        error(&f.service.execute(f.host, req, 1000)),
        Some(ErrorCode::InvalidEvent)
    );
    assert_eq!(f.state().sequence, seq);
}
#[test]
fn snapshot_does_not_increment_sequence() {
    let mut f = Fixture::new();
    let seq = f.state().sequence;
    let d = f.send(f.host, Command::Sync, 1000);
    assert_eq!(f.state().sequence, seq);
    assert!(matches!(d[0].effect, Effect::Snapshot(_)));
}
#[test]
fn disconnect_resume_rotates_token_resets_ready_and_recovers_snapshot() {
    let mut f = Fixture::new();
    let (b, original) = f.join();
    f.select();
    f.ready(f.host);
    f.ready(b);
    let seq = f.state().sequence;
    f.service.disconnect(b, 1000);
    assert_eq!(f.state().sequence, seq + 1);
    assert!(!f.state().members[1].connected);
    assert!(!f.state().members[1].ready);
    assert_eq!(f.state().members[1].lease_expires_at_ms, Some(31_000));
    let c = Uuid::new_v4();
    let d = f.send(
        c,
        Command::Resume {
            resume_token: original.resume_token.clone(),
            last_sequence: seq,
        },
        1500,
    );
    let resumed = credentials(&d);
    assert_eq!(resumed.member_id, original.member_id);
    assert_ne!(resumed.resume_token, original.resume_token);
    assert!(d.iter().any(|d| matches!(d.effect, Effect::Snapshot(_))));
    assert_eq!(f.service.binding(b), None);
    assert!(f.state().members[1].connected);
    assert!(!f.state().members[1].ready);
    assert_eq!(
        error(&f.send(
            Uuid::new_v4(),
            Command::Resume {
                resume_token: original.resume_token,
                last_sequence: seq
            },
            1600
        )),
        Some(ErrorCode::ResumeExpired)
    );
}
#[test]
fn resume_revokes_a_still_live_previous_socket() {
    let mut f = Fixture::new();
    let (b, c) = f.join();
    let new = Uuid::new_v4();
    f.send(
        new,
        Command::Resume {
            resume_token: c.resume_token,
            last_sequence: 2,
        },
        100,
    );
    assert_eq!(f.service.binding(b), None);
    assert!(f.service.binding(new).is_some());
    assert!(f.service.disconnect(b, 200).is_empty());
    assert!(f.state().members[1].connected);
}
#[test]
fn lease_expiry_removes_member_and_host_expiry_closes_room() {
    let mut f = Fixture::new();
    let (b, c) = f.join();
    f.service.disconnect(b, 1000);
    f.service.tick(31_000);
    assert_eq!(f.state().members.len(), 1);
    assert_eq!(
        error(&f.send(
            Uuid::new_v4(),
            Command::Resume {
                resume_token: c.resume_token,
                last_sequence: 2
            },
            31_001
        )),
        Some(ErrorCode::ResumeExpired)
    );
    f.service.disconnect(f.host, 32_000);
    f.service.tick(62_000);
    assert!(f.service.state(f.room).is_none());
}
#[test]
fn host_disconnect_replaces_pending_play_with_safe_pause() {
    let mut f = Fixture::new();
    f.select();
    f.ready(f.host);
    f.send(
        f.host,
        Command::Play {
            context: f.control(),
            position_ms: 100_000,
        },
        1000,
    );
    f.service.disconnect(f.host, 1100);
    let p = f
        .state()
        .playback
        .as_ref()
        .unwrap()
        .pending
        .as_ref()
        .unwrap();
    assert_eq!(p.command_event_id, None);
    assert_eq!(p.timeline_after.status, PlaybackStatus::Paused);
    assert_eq!(p.execute_at_ms, 1600);
}
#[test]
fn transfer_changes_authority_and_rejects_former_host() {
    let mut f = Fixture::new();
    let (b, c) = f.join();
    f.select();
    f.ready(f.host);
    f.ready(b);
    f.send(
        f.host,
        Command::Transfer {
            context: f.admin(),
            target_member_id: c.member_id,
        },
        1000,
    );
    assert_eq!(f.state().authority_revision, 2);
    assert_eq!(f.state().host_id, c.member_id);
    let mut ctx = f.control();
    ctx.admin.authority_revision = 1;
    assert_eq!(
        error(&f.send(
            b,
            Command::Play {
                context: ctx,
                position_ms: 0
            },
            1100
        )),
        Some(ErrorCode::StaleAuthority)
    );
    assert_eq!(
        error(&f.send(
            f.host,
            Command::Play {
                context: f.control(),
                position_ms: 0
            },
            1100
        )),
        Some(ErrorCode::NotAuthorized)
    );
}
#[test]
fn leave_removes_participant_and_host_closes_room() {
    let mut f = Fixture::new();
    let (b, _) = f.join();
    f.send(b, Command::Leave, 100);
    assert_eq!(f.state().members.len(), 1);
    assert_eq!(f.state().sequence, 3);
    f.send(f.host, Command::Leave, 200);
    assert!(f.service.state(f.room).is_none());
}
#[test]
fn new_selection_invalidates_ready_and_claims() {
    let mut f = Fixture::new();
    f.select();
    f.ready(f.host);
    f.send(
        f.host,
        Command::Select {
            context: f.admin(),
            previous_media_revision: 1,
            descriptor: descriptor(),
        },
        1000,
    );
    assert_eq!(f.state().media.as_ref().unwrap().media_revision, 2);
    assert!(!f.state().members[0].ready);
    assert_eq!(f.state().members[0].verified_media_revision, None);
}

#[test]
fn leave_retry_returns_cached_ack_without_second_mutation() {
    let mut f = Fixture::new();
    let (b, _) = f.join();
    let req = f.req(Command::Leave);
    f.service.execute(b, req.clone(), 100);
    let seq = f.state().sequence;
    let deliveries = f.service.execute(b, req, 200);
    assert!(matches!(deliveries[0].effect, Effect::Ack { .. }));
    assert_eq!(f.state().sequence, seq);
}

#[test]
fn canonical_wire_fingerprint_detects_ignored_field_changes() {
    let mut f = Fixture::new();
    f.select();
    f.ready(f.host);
    let mut req = f.req(Command::Ready {
        media_revision: 1,
        clock_uncertainty_ms: 1,
    });
    req.payload_fingerprint = Some([1; 32]);
    f.service.execute(f.host, req.clone(), 100);
    let seq = f.state().sequence;
    req.payload_fingerprint = Some([2; 32]);
    assert_eq!(
        error(&f.service.execute(f.host, req, 200)),
        Some(ErrorCode::InvalidEvent)
    );
    assert_eq!(f.state().sequence, seq);
}

#[test]
fn dedup_window_is_bounded_and_expiry_releases_capacity() {
    let mut f = Fixture::new();
    // Pre-entry creation has its own connection scope, member requests another.
    for _ in 0..256 {
        assert_eq!(
            error(&f.send(
                f.host,
                Command::Ready {
                    media_revision: 1,
                    clock_uncertainty_ms: 1
                },
                100
            )),
            Some(ErrorCode::NoMedia)
        );
    }
    assert_eq!(
        error(&f.send(
            f.host,
            Command::Ready {
                media_revision: 1,
                clock_uncertainty_ms: 1
            },
            100
        )),
        Some(ErrorCode::RateLimited)
    );
    assert_eq!(f.state().sequence, 1);
    assert_eq!(
        error(&f.send(
            f.host,
            Command::Ready {
                media_revision: 1,
                clock_uncertainty_ms: 1
            },
            120_101
        )),
        Some(ErrorCode::NoMedia)
    );
}

fn social_message(deliveries: &[Delivery]) -> &cine_rooms::social::SocialEntry {
    deliveries
        .iter()
        .find_map(|d| match &d.effect {
            Effect::Social {
                payload: cine_rooms::social::SocialPayload::Message(e),
                ..
            } => Some(e),
            _ => None,
        })
        .unwrap()
}
#[test]
fn social_authority_order_dedup_and_playback_isolation() {
    let mut f = Fixture::new();
    let (b, credentials) = f.join();
    let before = f.state().clone();
    let request = f.req(Command::Chat {
        text: "  Hola 😂\nsegunda línea  ".into(),
    });
    let d = f.service.execute(b, request.clone(), 100);
    let e = social_message(&d);
    assert_eq!(e.sender_id, credentials.member_id);
    assert_eq!(e.display_name, "Participant");
    assert_ne!(e.message_id, request.event_id);
    assert_eq!(e.text(), "Hola 😂\nsegunda línea");
    assert_eq!(e.sent_at_ms, 100);
    let seq = e.social_sequence;
    assert_eq!(f.state(), &before);
    let duplicate = f.service.execute(b, request.clone(), 101);
    assert!(!duplicate.iter().any(|d| matches!(
        d.effect,
        Effect::Social {
            payload: cine_rooms::social::SocialPayload::Message(_),
            ..
        }
    )));
    let mut different = request;
    different.command = Command::Chat {
        text: "changed".into(),
    };
    assert_eq!(
        error(&f.service.execute(b, different, 102)),
        Some(ErrorCode::InvalidEvent)
    );
    let request = f.req(Command::Chat {
        text: "host".into(),
    });
    let d = f.service.execute(f.host, request, 103);
    assert_eq!(social_message(&d).social_sequence, seq + 1);
    assert_eq!(f.state(), &before);
}
#[test]
fn social_validation_scope_and_independent_member_quotas() {
    let mut f = Fixture::new();
    let (b, _) = f.join();
    for (text, code) in [
        (" \n\t".into(), ErrorCode::InvalidEvent),
        ("😂".repeat(513), ErrorCode::PayloadTooLarge),
        ("a\n".repeat(10), ErrorCode::InvalidEvent),
        ("\0".into(), ErrorCode::InvalidEvent),
    ] {
        let r = f.req(Command::Chat { text });
        assert_eq!(error(&f.service.execute(b, r, 100)), Some(code));
    }
    for i in 0..20 {
        let r = f.req(Command::Chat {
            text: format!("{i}"),
        });
        assert_eq!(
            error(&f.service.execute(b, r, 100)),
            if i < 5 {
                None
            } else {
                Some(ErrorCode::RateLimited)
            }
        );
    }
    let r = f.req(Command::Chat {
        text: "other quota".into(),
    });
    assert_eq!(error(&f.service.execute(f.host, r, 100)), None);
    let r = f.req(Command::Chat {
        text: "refilled".into(),
    });
    assert_eq!(error(&f.service.execute(b, r, 2100)), None);
    for wrong_epoch in [true, false] {
        let mut r = f.req(Command::Chat {
            text: "bad scope".into(),
        });
        if wrong_epoch {
            r.room_epoch = Some(Uuid::new_v4());
        } else {
            r.room_id = Some(Uuid::new_v4());
        }
        assert!(error(&f.service.execute(b, r, 2200)).is_some());
    }
}
#[test]
fn social_resume_replays_chat_once_retains_names_and_quota_but_not_reactions() {
    use cine_rooms::social::SocialPayload;
    let mut f = Fixture::new();
    let (b, c) = f.join();
    for i in 0..5 {
        let r = f.req(Command::Chat {
            text: format!("{i}"),
        });
        f.service.execute(b, r, 10);
    }
    let r = f.req(Command::React {
        emoji: "😂".into()
    });
    assert!(f.service.execute(b, r, 10).iter().any(|d| matches!(
        d.effect,
        Effect::Social {
            payload: SocialPayload::Reaction { .. },
            ..
        }
    )));
    let d = f.service.disconnect(b, 20);
    assert!(!d.iter().any(|d| matches!(d.effect, Effect::Social { .. })));
    let new = Uuid::new_v4();
    let r = f.req(Command::Resume {
        resume_token: c.resume_token,
        last_sequence: 0,
    });
    let d = f.service.execute(new, r, 30);
    let entries = d
        .iter()
        .find_map(|d| match &d.effect {
            Effect::Social {
                payload: SocialPayload::Snapshot { entries, .. },
                ..
            } => Some(entries),
            _ => None,
        })
        .unwrap();
    assert_eq!(entries.iter().filter(|e| e.kind() == "chat").count(), 5);
    assert_eq!(entries.iter().filter(|e| e.kind() == "joined").count(), 1);
    assert_eq!(entries.iter().filter(|e| e.kind() == "resumed").count(), 1);
    assert!(!d.iter().any(|d| matches!(
        d.effect,
        Effect::Social {
            payload: SocialPayload::Reaction { .. },
            ..
        }
    )));
    let r = f.req(Command::Chat {
        text: "still limited".into(),
    });
    assert_eq!(
        error(&f.service.execute(new, r, 31)),
        Some(ErrorCode::RateLimited)
    );
    let r = f.req(Command::Leave);
    f.service.execute(new, r, 40);
    let r = f.req(Command::Sync);
    let d = f.service.execute(f.host, r, 41);
    let entries = d
        .iter()
        .find_map(|d| match &d.effect {
            Effect::Social {
                payload: SocialPayload::Snapshot { entries, .. },
                ..
            } => Some(entries),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        entries
            .iter()
            .filter(|e| e.kind() == "chat" && e.display_name == "Participant")
            .count(),
        5
    );
    assert!(entries.iter().any(|e| e.kind() == "left"));
}
#[test]
fn social_reaction_allowlist_burst_and_no_room_mutation() {
    let mut f = Fixture::new();
    let before = f.state().clone();
    let r = f.req(Command::React {
        emoji: "🚫".into()
    });
    assert_eq!(
        error(&f.service.execute(f.host, r, 100)),
        Some(ErrorCode::InvalidEvent)
    );
    for i in 0..50 {
        let r = f.req(Command::React {
            emoji: "❤️".into()
        });
        assert_eq!(
            error(&f.service.execute(f.host, r, 100)),
            if i < 8 {
                None
            } else {
                Some(ErrorCode::RateLimited)
            }
        );
    }
    let r = f.req(Command::React {
        emoji: "👏".into()
    });
    assert_eq!(error(&f.service.execute(f.host, r, 600)), None);
    assert_eq!(f.state(), &before);
}
#[test]
fn social_history_is_bounded_by_count_and_escaped_wire_budget() {
    use cine_rooms::social::*;
    for text in ["small".into(), "\"".repeat(CHAT_MAX_BYTES)] {
        let mut f = Fixture::new();
        for i in 0..400 {
            let r = f.req(Command::Chat { text: text.clone() });
            assert_eq!(error(&f.service.execute(f.host, r, i * 2000)), None);
        }
        let r = f.req(Command::Sync);
        let d = f.service.execute(f.host, r, 800_001);
        let entries = d
            .iter()
            .find_map(|d| match &d.effect {
                Effect::Social {
                    payload: SocialPayload::Snapshot { entries, .. },
                    ..
                } => Some(entries),
                _ => None,
            })
            .unwrap();
        assert!(entries.len() <= HISTORY_MAX_COUNT);
        assert!(entries.iter().map(|e| e.budget()).sum::<usize>() <= HISTORY_MAX_BYTES);
        assert_eq!(entries.last().unwrap().social_sequence, 400);
    }
}

#[test]
fn social_ids_cannot_collide_across_members_and_resume_presence_is_coalesced() {
    let mut f = Fixture::new();
    let (b, c) = f.join();
    let r = f.req(Command::Chat {
        text: "same client event ID".into(),
    });
    let first = f.service.execute(b, r.clone(), 10);
    let second = f.service.execute(f.host, r, 10);
    assert_ne!(
        social_message(&first).message_id,
        social_message(&second).message_id
    );
    f.service.disconnect(b, 20);
    let connection = Uuid::new_v4();
    let r = f.req(Command::Resume {
        resume_token: c.resume_token,
        last_sequence: 0,
    });
    let d = f.service.execute(connection, r, 30);
    let rotated = credentials(&d);
    assert_eq!(social_message(&d).kind(), "resumed");
    f.service.disconnect(connection, 40);
    let connection = Uuid::new_v4();
    let r = f.req(Command::Resume {
        resume_token: rotated.resume_token,
        last_sequence: 0,
    });
    let d = f.service.execute(connection, r, 50);
    assert!(!d.iter().any(|d| matches!(
        d.effect,
        Effect::Social {
            payload: cine_rooms::social::SocialPayload::Message(_),
            ..
        }
    )));
    assert!(d.iter().any(|d| matches!(
        d.effect,
        Effect::Social {
            payload: cine_rooms::social::SocialPayload::Snapshot { .. },
            ..
        }
    )));
}

fn fixture_gif() -> cine_rooms::social::MessageContent {
    cine_rooms::social::MessageContent::Gif(cine_rooms::social::GifDescriptor {
        provider: "fixture".into(),
        provider_content_id: "celebrate".into(),
        media_url: "https://fixtures.cine.invalid/celebrate.gif".into(),
        preview_url: None,
        width: 160,
        height: 100,
        alt_text: "Celebración".into(),
    })
}
#[test]
fn rich_gif_reply_reaction_toggle_dedup_scope_and_authority() {
    let mut f = Fixture::new();
    let (b, c) = f.join();
    let before = f.state().clone();
    let req = f.req(Command::RichMessage {
        content: fixture_gif(),
        reply_to_message_id: None,
    });
    let d = f.service.execute(b, req.clone(), 100);
    let gif = social_message(&d).clone();
    assert_eq!(gif.sender_id, c.member_id);
    assert_eq!(gif.display_name, "Participant");
    assert!(gif.sent_at_utc_ms.unwrap() > 1_700_000_000_000);
    let retry = f.service.execute(b, req, 101);
    assert!(!retry.iter().any(|d| matches!(
        d.effect,
        Effect::Social {
            payload: cine_rooms::social::SocialPayload::Message(_),
            ..
        }
    )));
    let req = f.req(Command::RichMessage {
        content: cine_rooms::social::MessageContent::Text("JAJAJA".into()),
        reply_to_message_id: Some(gif.message_id),
    });
    let d = f.service.execute(f.host, req, 102);
    let reply = social_message(&d).clone();
    assert_eq!(reply.reply_to_message_id, Some(gif.message_id));
    for (now, count) in [(103, 1), (104, 0)] {
        let req = f.req(Command::MessageReact {
            message_id: reply.message_id,
            emoji: "❤️".into(),
        });
        let d = f.service.execute(b, req.clone(), now);
        assert!(error(&d).is_none());
        let entries = d
            .iter()
            .find_map(|d| {
                if let Effect::Social {
                    payload: cine_rooms::social::SocialPayload::MessageReactions { entries, .. },
                    ..
                } = &d.effect
                {
                    Some(entries)
                } else {
                    None
                }
            })
            .unwrap();
        let e = entries
            .iter()
            .find(|e| e.message_id == reply.message_id)
            .unwrap();
        assert_eq!(e.message_reactions.get("❤️").map_or(0, |s| s.len()), count);
        assert!(error(&f.service.execute(b, req, now)).is_none());
    }
    let req = f.req(Command::RichMessage {
        content: fixture_gif(),
        reply_to_message_id: Some(Uuid::new_v4()),
    });
    assert_eq!(
        error(&f.service.execute(b, req, 105)),
        Some(ErrorCode::InvalidEvent)
    );
    let mut req = f.req(Command::RichMessage {
        content: fixture_gif(),
        reply_to_message_id: None,
    });
    req.room_id = Some(Uuid::new_v4());
    assert!(error(&f.service.execute(b, req, 106)).is_some());
    let mut req = f.req(Command::RichMessage {
        content: fixture_gif(),
        reply_to_message_id: None,
    });
    req.room_epoch = Some(Uuid::new_v4());
    assert!(error(&f.service.execute(b, req, 107)).is_some());
    let req = f.req(Command::MessageReact {
        message_id: Uuid::new_v4(),
        emoji: "❤️".into(),
    });
    assert_eq!(
        error(&f.service.execute(b, req, 108)),
        Some(ErrorCode::InvalidEvent)
    );
    assert_eq!(f.state(), &before);
}
#[test]
fn rich_reaction_quota_is_independent_and_aggregation_is_per_member() {
    let mut f = Fixture::new();
    let (b, _) = f.join();
    let req = f.req(Command::RichMessage {
        content: fixture_gif(),
        reply_to_message_id: None,
    });
    let d = f.service.execute(f.host, req, 100);
    let id = social_message(&d).message_id;
    for i in 0..6 {
        let req = f.req(Command::MessageReact {
            message_id: id,
            emoji: "😂".into(),
        });
        assert!(error(&f.service.execute(b, req, 101 + i)).is_none());
    }
    let req = f.req(Command::MessageReact {
        message_id: id,
        emoji: "😂".into(),
    });
    assert_eq!(
        error(&f.service.execute(b, req, 107)),
        Some(ErrorCode::RateLimited)
    );
    for connection in [f.host, b] {
        let req = f.req(Command::MessageReact {
            message_id: id,
            emoji: "❤️".into(),
        });
        let d = f.service.execute(connection, req, 1200);
        assert!(error(&d).is_none());
        if connection == b {
            let entries =
                d.iter()
                    .find_map(|d| {
                        if let Effect::Social {
                            payload:
                                cine_rooms::social::SocialPayload::MessageReactions { entries, .. },
                            ..
                        } = &d.effect
                        {
                            Some(entries)
                        } else {
                            None
                        }
                    })
                    .unwrap();
            assert_eq!(
                entries
                    .iter()
                    .find(|e| e.message_id == id)
                    .unwrap()
                    .message_reactions["❤️"]
                    .len(),
                2
            );
        }
    }
    let req = f.req(Command::Chat {
        text: "Cuota independiente".into(),
    });
    assert!(error(&f.service.execute(b, req, 1201)).is_none());
}
#[test]
fn mixed_history_soak_bounds_json_reactions_and_evicted_replies() {
    let mut f = Fixture::new();
    let mut first = None;
    let mut retained_reply = None;
    for i in 0..500 {
        let req = f.req(Command::RichMessage {
            content: if i % 2 == 0 {
                fixture_gif()
            } else {
                cine_rooms::social::MessageContent::Text("Texto 😂".repeat(100))
            },
            reply_to_message_id: None,
        });
        let d = f.service.execute(f.host, req, 100 + i * 2500);
        assert!(error(&d).is_none());
        let id = social_message(&d).message_id;
        if i == 0 {
            first = Some(id);
        }
        if i == 498 {
            retained_reply = Some(id);
        }
        for emoji in cine_rooms::social::REACTIONS.iter().take(2) {
            let req = f.req(Command::MessageReact {
                message_id: id,
                emoji: (*emoji).into(),
            });
            assert!(error(&f.service.execute(f.host, req, 100 + i * 2500)).is_none());
        }
    }
    let req = f.req(Command::RichMessage {
        content: fixture_gif(),
        reply_to_message_id: retained_reply,
    });
    let d = f.service.execute(f.host, req, 1_300_000);
    assert!(error(&d).is_none());
    let req = f.req(Command::RichMessage {
        content: fixture_gif(),
        reply_to_message_id: first,
    });
    assert_eq!(
        error(&f.service.execute(f.host, req, 1_300_001)),
        Some(ErrorCode::InvalidEvent)
    );
    let req = f.req(Command::Sync);
    let d = f.service.execute(f.host, req, 1_300_002);
    let entries = d
        .iter()
        .find_map(|d| {
            if let Effect::Social {
                payload: cine_rooms::social::SocialPayload::Snapshot { entries, .. },
                ..
            } = &d.effect
            {
                Some(entries)
            } else {
                None
            }
        })
        .unwrap();
    assert!(entries.len() <= 100);
    assert!(entries.iter().map(|e| e.budget()).sum::<usize>() <= 48 * 1024);
    assert!(entries.iter().all(|e| e.message_id != first.unwrap()));
    assert!(entries.iter().all(|e| e.message_reactions.len() <= 6));
}

#[test]
fn actual_cross_room_reply_and_unapproved_provider_are_rejected() {
    let mut f = Fixture::new();
    let mut other = Fixture::new();
    let req = other.req(Command::RichMessage {
        content: fixture_gif(),
        reply_to_message_id: None,
    });
    let d = other.service.execute(other.host, req, 100);
    let id = social_message(&d).message_id;
    let req = f.req(Command::RichMessage {
        content: fixture_gif(),
        reply_to_message_id: Some(id),
    });
    assert_eq!(
        error(&f.service.execute(f.host, req, 100)),
        Some(ErrorCode::InvalidEvent)
    );
    let cine_rooms::social::MessageContent::Gif(mut gif) = fixture_gif() else {
        unreachable!()
    };
    gif.provider = "giphy".into();
    gif.provider_content_id = "abc".into();
    gif.media_url = "https://media.giphy.com/media/abc/a.gif".into();
    let req = f.req(Command::RichMessage {
        content: cine_rooms::social::MessageContent::Gif(gif),
        reply_to_message_id: None,
    });
    assert_eq!(
        error(&f.service.execute(f.host, req, 101)),
        Some(ErrorCode::FeatureNotSupported)
    );
}
