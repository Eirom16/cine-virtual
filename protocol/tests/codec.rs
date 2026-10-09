use cine_protocol::*;
use cine_rooms::model::ErrorCode;
use serde_json::{Value, json};
use uuid::Uuid;

fn hello() -> Value {
    json!({"protocol_version":1,"event_id":Uuid::new_v4(),"type":"SESSION_HELLO","room_id":null,"room_epoch":null,"sender_id":null,"sequence":null,"sent_at_ms":0,
    "payload":{"supported_versions":[1],"client_name":"Test"}})
}
fn rejected(v: Value) -> ErrorCode {
    match decode(&v.to_string()) {
        Err(e) => e,
        Ok(m) => match incoming(&m) {
            Err(e) => e,
            _ => panic!("accepted invalid input"),
        },
    }
}
#[test]
fn accepts_valid_hello_and_roundtrips_envelope() {
    let m = decode(&hello().to_string()).unwrap();
    assert!(matches!(incoming(&m), Ok(Incoming::Hello { .. })));
    assert!(decode(&encode(&m).unwrap()).is_ok());
}
#[test]
fn version_size_unknown_type_and_bad_uuid_are_rejected() {
    let mut v = hello();
    v["protocol_version"] = json!(2);
    assert_eq!(rejected(v), ErrorCode::ProtocolVersionUnsupported);
    assert!(matches!(
        decode(&"x".repeat(MAX_MESSAGE_BYTES + 1)),
        Err(ErrorCode::PayloadTooLarge)
    ));
    let mut v = hello();
    v["type"] = json!("UNKNOWN");
    assert_eq!(rejected(v), ErrorCode::InvalidEvent);
    let mut v = hello();
    v["event_id"] = json!("invalid");
    assert_eq!(rejected(v), ErrorCode::InvalidEvent);
}
#[test]
fn malformed_missing_payload_null_and_duplicate_keys_are_rejected() {
    assert!(decode("{").is_err());
    let mut v = hello();
    v.as_object_mut().unwrap().remove("payload");
    assert_eq!(rejected(v), ErrorCode::InvalidEvent);
    let mut v = hello();
    v["payload"] = Value::Null;
    assert_eq!(rejected(v), ErrorCode::InvalidEvent);
    let mut v = hello();
    v.as_object_mut().unwrap().remove("room_id");
    assert_eq!(rejected(v), ErrorCode::InvalidEvent);
    let text = hello()
        .to_string()
        .replacen("{", "{\"protocol_version\":1,", 1);
    assert!(decode(&text).is_err());
    let text = hello().to_string().replace(
        "\"client_name\":\"Test\"",
        "\"client_name\":\"Test\",\"client_name\":\"Other\"",
    );
    assert!(decode(&text).is_err());
}
#[test]
fn ranges_depth_enums_and_server_only_commands_are_rejected() {
    let mut v = hello();
    v["sent_at_ms"] = json!(MAX_INTEGER + 1);
    assert_eq!(rejected(v), ErrorCode::InvalidEvent);
    let mut v = hello();
    v["sent_at_ms"] = json!(-1);
    assert_eq!(rejected(v), ErrorCode::InvalidEvent);
    let mut v = hello();
    v["payload"]["client_name"] = json!("x".repeat(65));
    assert_eq!(rejected(v), ErrorCode::InvalidEvent);
    let mut v = hello();
    v["type"] = json!("PLAY");
    assert_eq!(rejected(v), ErrorCode::NotAuthorized);
    let mut v = hello();
    v["type"] = json!("MEDIA_NOT_READY");
    v["room_id"] = json!(Uuid::new_v4());
    v["room_epoch"] = json!(Uuid::new_v4());
    v["payload"] = json!({"media_revision":1,"reason":"invalid_enum"});
    assert_eq!(rejected(v), ErrorCode::InvalidEvent);
    let mut deep = json!(0);
    for _ in 0..13 {
        deep = json!([deep]);
    }
    assert!(decode(&deep.to_string()).is_err());
}
#[test]
fn documented_json_envelopes_validate() {
    let document = include_str!("../../docs/PROTOCOL.md");
    for block in document.split("```json\n").skip(1) {
        let text = block.split("\n```").next().unwrap();
        let value: Value = serde_json::from_str(text).unwrap();
        if value.get("protocol_version").is_some() {
            let message = decode(text).unwrap();
            if message.kind == "PLAY" {
                assert!(state_from_message(&message).unwrap().is_some());
            } else {
                assert!(incoming(&message).is_ok());
            }
        } else {
            let descriptor: MediaDto = serde_json::from_value(value).unwrap();
            assert!(descriptor.validate().is_ok());
        }
    }
}

#[test]
fn canonical_payload_fingerprint_is_stable_and_detects_extensions() {
    let mut v = hello();
    v["type"] = json!("ROOM_CREATE");
    v["payload"] = json!({"display_name":"Host","extension":1});
    let Incoming::Room(first) = incoming(&decode(&v.to_string()).unwrap()).unwrap() else {
        panic!("room command")
    };
    let text = v.to_string().replace(
        "\"display_name\":\"Host\",\"extension\":1",
        "\"extension\":1,\"display_name\":\"Host\"",
    );
    let Incoming::Room(reordered) = incoming(&decode(&text).unwrap()).unwrap() else {
        panic!("room command")
    };
    assert_eq!(first.payload_fingerprint, reordered.payload_fingerprint);
    v["payload"]["extension"] = json!(2);
    let Incoming::Room(changed) = incoming(&decode(&v.to_string()).unwrap()).unwrap() else {
        panic!("room command")
    };
    assert_ne!(first.payload_fingerprint, changed.payload_fingerprint);
}

#[test]
fn social_intents_are_plain_bounded_and_server_events_are_not_client_intents() {
    let mut v = hello();
    v["type"] = json!("CHAT_SEND");
    v["room_id"] = json!(Uuid::new_v4());
    v["room_epoch"] = json!(Uuid::new_v4());
    v["sender_id"] = json!(Uuid::new_v4());
    v["payload"] = json!({"text":"Hola 😂\n世界"});
    assert!(matches!(
        incoming(&decode(&v.to_string()).unwrap()),
        Ok(Incoming::Room(_))
    ));
    for payload in [
        json!({"text":""}),
        json!({"text":" \n\t"}),
        json!({"text":"a","sender_id":Uuid::new_v4()}),
        json!({"text":"a","display_name":"Host"}),
        json!({"text":"x".repeat(2049)}),
    ] {
        v["payload"] = payload;
        assert!(matches!(
            rejected(v.clone()),
            ErrorCode::InvalidEvent | ErrorCode::PayloadTooLarge
        ));
    }
    v["payload"] = json!({"text":"valid"});
    v["sequence"] = json!(1);
    assert_eq!(rejected(v.clone()), ErrorCode::NotAuthorized);
    v["sequence"] = json!(null);
    v["type"] = json!("REACTION_SEND");
    v["payload"] = json!({"emoji":"😂"});
    assert!(incoming(&decode(&v.to_string()).unwrap()).is_ok());
    v["payload"] = json!({"emoji":"a".repeat(4096)});
    assert_eq!(rejected(v.clone()), ErrorCode::InvalidEvent);
    for kind in ["CHAT_MESSAGE", "SOCIAL_STATE", "REACTION"] {
        v["type"] = json!(kind);
        assert_eq!(rejected(v.clone()), ErrorCode::NotAuthorized);
    }
    let bad = hello().to_string().replace("Test", r"\ud800");
    assert!(decode(&bad).is_err());
}
#[test]
fn social_snapshot_encoding_stays_below_transport_limit_with_escaped_unicode() {
    use cine_rooms::{
        model::Member,
        model::{MemberStatus, Role},
        social::{SocialPayload, SocialState},
    };
    let member = Member {
        member_id: Uuid::new_v4(),
        display_name: "\"".repeat(64),
        role: Role::Host,
        connected: true,
        ready: false,
        verified_media_revision: None,
        status: MemberStatus::Idle,
        joined_at_ms: 0,
        lease_expires_at_ms: None,
    };
    let mut social = SocialState::default();
    for i in 0..300 {
        social.entry(
            Uuid::new_v4(),
            &member,
            "chat",
            &format!("a{}z", "\t".repeat(2046)),
            i,
        );
    }
    let m = social_message(
        Uuid::new_v4(),
        Uuid::new_v4(),
        &SocialPayload::Snapshot {
            sequence: social.sequence,
            entries: social.history.into(),
        },
        999,
    );
    let text = encode(&m).unwrap();
    assert!(text.len() < MAX_MESSAGE_BYTES);
    let snapshot: SocialSnapshotDto =
        serde_json::from_value(decode(&text).unwrap().payload).unwrap();
    snapshot.validate().unwrap();
}

fn rich_request() -> Value {
    let mut v = hello();
    v["type"] = json!("MESSAGE_SEND");
    v["room_id"] = json!(Uuid::new_v4());
    v["room_epoch"] = json!(Uuid::new_v4());
    v["payload"] = json!({"content":{"type":"gif","gif":{"provider":"fixture","provider_content_id":"celebrate","media_url":"https://fixtures.cine.invalid/celebrate.gif","preview_url":null,"width":160,"height":100,"alt_text":"Celebración"}},"reply_to_message_id":null});
    v
}
#[test]
fn rich_content_validates_strict_descriptors_schemes_identity_and_sizes() {
    assert!(incoming(&decode(&rich_request().to_string()).unwrap()).is_ok());
    for url in [
        "http://fixtures.cine.invalid/celebrate.gif",
        "file:///tmp/a.gif",
        "javascript:alert(1)",
        "data:image/gif;base64,a",
        "https://127.0.0.1/a.gif",
        "https://localhost/a.gif",
        "https://10.0.0.1/a.gif",
        "https://fixtures.cine.invalid.evil/a.gif",
        "https://u:p@fixtures.cine.invalid/celebrate.gif",
        "https://fixtures.cine.invalid:443/celebrate.gif",
    ] {
        let mut v = rich_request();
        v["payload"]["content"]["gif"]["media_url"] = json!(url);
        assert_eq!(rejected(v), ErrorCode::InvalidEvent, "{url}");
    }
    for (field, value) in [
        ("provider", json!("evil")),
        ("provider_content_id", json!("../evil")),
        ("width", json!(0)),
        ("height", json!(4000)),
        ("alt_text", json!("a".repeat(257))),
        ("media_url", json!("a".repeat(1025))),
        ("extra", json!("metadata")),
    ] {
        let mut v = rich_request();
        v["payload"]["content"]["gif"][field] = value;
        assert_eq!(rejected(v), ErrorCode::InvalidEvent);
    }
    let mut v = rich_request();
    v["payload"]["sender_id"] = json!(Uuid::new_v4());
    assert_eq!(rejected(v), ErrorCode::InvalidEvent);
    let mut v = rich_request();
    v["payload"]["reply_to_message_id"] = json!("wrong");
    assert_eq!(rejected(v), ErrorCode::InvalidEvent);
    let mut v = rich_request();
    v["payload"]["content"]["type"] = json!("system");
    assert_eq!(rejected(v), ErrorCode::InvalidEvent);
}
#[test]
fn giphy_urls_are_host_and_content_bound_without_server_fetch() {
    let mut v = rich_request();
    let g = &mut v["payload"]["content"]["gif"];
    g["provider"] = json!("giphy");
    g["provider_content_id"] = json!("abc123");
    g["media_url"] =
        json!("https://media2.giphy.com/media/abc123/200w_d.gif?cid=test&rid=200w_d.gif&ct=g");
    assert!(incoming(&decode(&v.to_string()).unwrap()).is_ok());
    for url in [
        "https://media.giphy.com/media/other/a.gif",
        "https://evil.giphy.com/media/abc123/a.gif",
        "https://media.giphy.com/media/abc123/a.mp4",
        "https://media.giphy.com/media/abc123/a.gif?api_key=secret",
    ] {
        let mut bad = v.clone();
        bad["payload"]["content"]["gif"]["media_url"] = json!(url);
        assert_eq!(rejected(bad), ErrorCode::InvalidEvent);
    }
}

#[test]
fn rich_snapshot_budget_covers_encoded_json_with_maximum_reaction_metadata() {
    use cine_rooms::{
        model::{Member, MemberStatus, Role},
        social::{GifDescriptor, MessageContent, SocialPayload, SocialState},
    };
    let member = Member {
        member_id: Uuid::new_v4(),
        display_name: "😂".repeat(16),
        role: Role::Host,
        connected: true,
        ready: false,
        verified_media_revision: None,
        status: MemberStatus::Idle,
        joined_at_ms: 0,
        lease_expires_at_ms: None,
    };
    let mut state = SocialState::default();
    let reaction_members: Vec<_> = (0..16).map(|_| Uuid::new_v4()).collect();
    for i in 0..400 {
        let content = if i % 2 == 0 {
            MessageContent::Text("\"\\\t😂".repeat(200))
        } else {
            MessageContent::Gif(GifDescriptor {
                provider: "fixture".into(),
                provider_content_id: "celebrate".into(),
                media_url: "https://fixtures.cine.invalid/celebrate.gif".into(),
                preview_url: None,
                width: 160,
                height: 100,
                alt_text: "\"\\😂".repeat(30),
            })
        };
        let entry = state.message(Uuid::new_v4(), &member, content, None, i * 10000);
        for member_id in &reaction_members {
            for emoji in cine_rooms::social::REACTIONS {
                state
                    .toggle(entry.message_id, *member_id, emoji, i * 10000)
                    .unwrap();
            }
        }
        let wire = social_message(
            Uuid::new_v4(),
            Uuid::new_v4(),
            &SocialPayload::Snapshot {
                sequence: state.sequence,
                entries: state.history.iter().cloned().collect(),
            },
            i * 10000,
        );
        let encoded = encode(&wire).unwrap();
        assert!(encoded.len() < 48 * 1024);
        assert!(state.bytes <= 48 * 1024);
        let snapshot: SocialSnapshotDto = serde_json::from_value(wire.payload).unwrap();
        snapshot.validate().unwrap();
    }
}

#[test]
fn transfer_control_is_strict_and_server_state_cannot_be_spoofed() {
    let mut v = hello();
    v["type"] = json!("P2P_TRANSFER_REQUEST");
    v["room_id"] = json!(Uuid::new_v4());
    v["room_epoch"] = json!(Uuid::new_v4());
    v["sender_id"] = json!(Uuid::new_v4());
    v["payload"] = json!({"signal":{"action":"request","transfer_id":Uuid::new_v4()}});
    assert!(incoming(&decode(&v.to_string()).unwrap()).is_ok());
    let mut bad = v.clone();
    bad["payload"]["signal"]["secret"] = json!("spoof");
    assert_eq!(rejected(bad), ErrorCode::InvalidEvent);
    let mut bad = v.clone();
    bad["payload"]["file_bytes"] = json!([1, 2, 3]);
    assert_eq!(rejected(bad), ErrorCode::InvalidEvent);
    let mut bad = v.clone();
    bad["payload"]["signal"]["action"] = json!("unknown");
    assert_eq!(rejected(bad), ErrorCode::InvalidEvent);
    v["type"] = json!("P2P_TRANSFER_STATE");
    assert_eq!(rejected(v), ErrorCode::NotAuthorized);
}
