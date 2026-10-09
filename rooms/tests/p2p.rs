use cine_core::media::{ContentIdentity, MediaDescriptor, SourceType};
use cine_rooms::{model::*, p2p::Transfers};
use cine_transfer_model::{CHUNK_SIZE, Manifest, TransferIntent as I};
use uuid::Uuid;
fn fixture() -> (RoomState, Manifest, Uuid) {
    let host = Uuid::new_v4();
    let receiver = Uuid::new_v4();
    let media = Uuid::new_v4();
    let identity = ContentIdentity {
        size_bytes: 32,
        sha256: [7; 32],
    };
    let room = RoomState {
        room_id: Uuid::new_v4(),
        room_epoch: Uuid::new_v4(),
        sequence: 3,
        updated_at_ms: 0,
        host_id: host,
        authority_revision: 1,
        members: [host, receiver]
            .iter()
            .map(|id| Member {
                member_id: *id,
                display_name: "fixture".into(),
                role: if *id == host {
                    Role::Host
                } else {
                    Role::Participant
                },
                connected: true,
                ready: false,
                verified_media_revision: None,
                status: MemberStatus::Idle,
                joined_at_ms: 0,
                lease_expires_at_ms: None,
            })
            .collect(),
        media: Some(MediaSelection {
            media_revision: 1,
            descriptor: MediaDescriptor {
                media_id: media.to_string(),
                source_type: SourceType::LocalFile,
                title: None,
                duration_ms: 1000,
                identity: identity.clone(),
                mime: None,
                codecs: vec![],
            },
        }),
        playback: None,
    };
    let manifest = Manifest {
        version: 1,
        transfer_id: Uuid::new_v4(),
        room_id: room.room_id,
        room_epoch: room.room_epoch,
        host_id: host,
        authority_revision: 1,
        media_revision: 1,
        media_id: media,
        display_name: "fixture".into(),
        size_bytes: 32,
        chunk_size: CHUNK_SIZE,
        chunk_count: 1,
        sha256: identity.sha256,
        duration_ms: 1000,
        block_hash: "sha256".into(),
    };
    (room, manifest, receiver)
}
fn offer(m: Manifest) -> I {
    I::Offer {
        manifest: Box::new(m),
        address: "127.0.0.1:1730".parse().unwrap(),
        certificate: vec![7; 32],
    }
}
#[test]
fn consent_host_authority_room_epoch_media_and_spoofing() {
    let (room, m, receiver) = fixture();
    let mut t = Transfers::default();
    assert!(matches!(
        t.apply(&room, receiver, &offer(m.clone()), 0),
        Err(ErrorCode::NotAuthorized)
    ));
    for field in 0..5 {
        let mut bad = m.clone();
        match field {
            0 => bad.room_id = Uuid::new_v4(),
            1 => bad.room_epoch = Uuid::new_v4(),
            2 => bad.authority_revision += 1,
            3 => bad.media_revision += 1,
            _ => bad.sha256 = [8; 32],
        }
        assert!(t.apply(&room, room.host_id, &offer(bad), 0).is_err());
    }
    t.apply(&room, room.host_id, &offer(m.clone()), 0).unwrap();
    let accept = I::Accept {
        transfer_id: m.transfer_id,
        receiver_id: receiver,
    };
    assert!(matches!(
        t.apply(&room, room.host_id, &accept, 0),
        Err(ErrorCode::InvalidState)
    ));
    t.apply(
        &room,
        receiver,
        &I::Request {
            transfer_id: m.transfer_id,
        },
        0,
    )
    .unwrap();
    assert!(matches!(
        t.apply(&room, receiver, &accept, 0),
        Err(ErrorCode::NotAuthorized)
    ));
    assert!(matches!(
        t.apply(&room, Uuid::new_v4(), &accept, 0),
        Err(ErrorCode::NotAuthorized)
    ));
    let grant = t.apply(&room, room.host_id, &accept, 1).unwrap().unwrap();
    assert_eq!(grant.credential.receiver_id, receiver);
    assert_eq!(grant.expires_at_ms, 600001);
    assert!(matches!(
        t.apply(&room, room.host_id, &accept, 1),
        Err(ErrorCode::RateLimited)
    ));
    assert_eq!(room.sequence, 3);
}
#[test]
fn pause_resume_rotates_authorization_expiration_disconnect_and_media_host_changes() {
    let (mut room, m, receiver) = fixture();
    let mut t = Transfers::default();
    t.apply(&room, room.host_id, &offer(m.clone()), 0).unwrap();
    let request = I::Request {
        transfer_id: m.transfer_id,
    };
    let accept = I::Accept {
        transfer_id: m.transfer_id,
        receiver_id: receiver,
    };
    t.apply(&room, receiver, &request, 0).unwrap();
    let first = t.apply(&room, room.host_id, &accept, 1).unwrap().unwrap();
    t.apply(
        &room,
        receiver,
        &I::Status {
            transfer_id: m.transfer_id,
            state: "paused".into(),
            verified_bytes: 16,
        },
        2,
    )
    .unwrap();
    assert!(t.grant(receiver).is_none());
    t.apply(&room, receiver, &request, 3).unwrap();
    let second = t.apply(&room, room.host_id, &accept, 4).unwrap().unwrap();
    assert_ne!(first.credential.grant_id, second.credential.grant_id);
    assert_eq!(t.snapshot().receivers[0].verified_bytes, 16);
    t.reconcile(&room, second.expires_at_ms);
    assert!(t.grant(receiver).is_none());
    assert!(!t.snapshot().receivers[0].authorized);
    t.apply(&room, receiver, &request, 700000).unwrap();
    t.apply(&room, room.host_id, &accept, 700001).unwrap();
    room.members[0].connected = false;
    t.reconcile(&room, 700002);
    assert!(t.grant(receiver).is_none());
    room.members[0].connected = true;
    room.media.as_mut().unwrap().media_revision += 1;
    t.reconcile(&room, 700003);
    assert!(t.snapshot().offer.is_none());
    t = Transfers::default();
    room.media.as_mut().unwrap().media_revision = 1;
    t.apply(&room, room.host_id, &offer(m), 0).unwrap();
    room.host_id = receiver;
    room.authority_revision += 1;
    t.reconcile(&room, 0);
    assert!(t.snapshot().offer.is_none());
}
#[test]
fn cancel_withdraw_reject_storage_progress_and_limits() {
    let (room, m, receiver) = fixture();
    let mut t = Transfers::default();
    t.apply(&room, room.host_id, &offer(m.clone()), 0).unwrap();
    t.apply(
        &room,
        receiver,
        &I::Request {
            transfer_id: m.transfer_id,
        },
        0,
    )
    .unwrap();
    assert!(matches!(
        t.apply(
            &room,
            receiver,
            &I::Status {
                transfer_id: m.transfer_id,
                state: "transferring".into(),
                verified_bytes: 33
            },
            0
        ),
        Err(ErrorCode::InvalidEvent)
    ));
    t.apply(
        &room,
        room.host_id,
        &I::Reject {
            transfer_id: m.transfer_id,
            receiver_id: receiver,
        },
        0,
    )
    .unwrap();
    assert!(t.snapshot().receivers.is_empty());
    t.apply(
        &room,
        receiver,
        &I::Request {
            transfer_id: m.transfer_id,
        },
        1,
    )
    .unwrap();
    t.apply(
        &room,
        receiver,
        &I::Cancel {
            transfer_id: m.transfer_id,
        },
        2,
    )
    .unwrap();
    assert!(t.snapshot().receivers.is_empty());
    t.apply(
        &room,
        room.host_id,
        &I::Withdraw {
            transfer_id: m.transfer_id,
        },
        3,
    )
    .unwrap();
    assert!(t.snapshot().offer.is_none());
}
