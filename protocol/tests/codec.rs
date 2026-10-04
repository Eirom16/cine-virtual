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
