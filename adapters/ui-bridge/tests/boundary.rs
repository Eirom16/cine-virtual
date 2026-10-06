use cine_ui_bridge::{
    Application, Reply, cine_bridge_call, cine_bridge_create, cine_bridge_destroy,
    cine_bridge_hash_fd,
};
use serde_json::{Value, json};
fn command(app: &mut Application, generation: u64, kind: &str, payload: Value, now: u64) -> Reply {
    app.dispatch(&serde_json::to_vec(&json!({"api_version":1,"generation":generation,"command":{"type":kind,"payload":payload}})).unwrap(), now)
}
fn simple(app: &mut Application, generation: u64, kind: &str, now: u64) -> Reply {
    app.dispatch(
        &serde_json::to_vec(
            &json!({"api_version":1,"generation":generation,"command":{"type":kind}}),
        )
        .unwrap(),
        now,
    )
}
fn prepared() -> Application {
    let mut app = Application::default();
    command(
        &mut app,
        1,
        "configure",
        json!({"playback_rate":true,"content_uri_input":true}),
        0,
    );
    for _ in 0..3 {
        command(
            &mut app,
            1,
            "clock",
            json!({"t1":0,"t2":1,"t3":1,"t4":2}),
            0,
        );
    }
    simple(&mut app, 1, "snapshot", 0);
    sample(&mut app, 10120, 0);
    app
}
fn sample(app: &mut Application, position_ms: u64, now: u64) {
    assert!(command(app,1,"sample",json!({"position_ms":position_ms,"duration_ms":30000,"playing":true,"loaded":true,"buffering":false,"seeking":false,"age_ms":0}),now).ok);
}
#[test]
fn validates_boundary_version_enums_limits_and_payload() {
    let mut app = Application::default();
    for bytes in [
        b"{}".as_slice(),
        b"{\"api_version\":1,\"generation\":1,\"command\":{\"type\":\"unknown\"}}",
        b"{\"api_version\":1,\"generation\":1,\"command\":{\"type\":\"seek\"}}",
    ] {
        assert_eq!(app.dispatch(bytes, 0).error.unwrap().code, "INVALID_DTO");
    }
    assert_eq!(
        app.dispatch(&vec![b' '; 65537], 0).error.unwrap().code,
        "PAYLOAD_TOO_LARGE"
    );
    assert_eq!(
        app.dispatch(
            b"{\"api_version\":2,\"generation\":1,\"command\":{\"type\":\"state\"}}",
            0
        )
        .error
        .unwrap()
        .code,
        "VERSION_UNSUPPORTED"
    );
}
#[test]
fn native_observations_feed_unchanged_sync_engine() {
    let mut app = prepared();
    assert!(
        command(&mut app, 1, "observe", json!({"target_ms":10000}), 0)
            .effects
            .is_empty()
    );
    command(&mut app, 1, "observe", json!({"target_ms":10000}), 500);
    let reply = command(&mut app, 1, "observe", json!({"target_ms":10000}), 1000);
    assert_eq!(reply.effects[0].action, "rate");
    assert_eq!(reply.effects[0].value, 0.98);
    sample(&mut app, 10020, 1500);
    assert_eq!(
        command(&mut app, 1, "observe", json!({"target_ms":10000}), 1500).effects[0].value,
        1.0
    );
    for now in [4000, 4500, 5000] {
        sample(&mut app, 12000, now);
        let r = command(&mut app, 1, "observe", json!({"target_ms":10000}), now);
        if now == 5000 {
            assert_eq!(r.effects[0].action, "seek");
            assert_eq!(r.effects[0].value, 10000.0);
        }
    }
}
#[test]
fn lifecycle_cancels_deadlines_and_requires_clock_plus_snapshot() {
    let mut app = prepared();
    assert!(command(&mut app, 1, "schedule_play", json!({"deadline_ms":500}), 0).ok);
    let s = simple(&mut app, 1, "suspend", 100);
    assert_eq!(s.generation, 2);
    assert!(!s.clock_trusted);
    assert!(s.snapshot_required);
    assert_eq!(
        simple(&mut app, 1, "play", 500).error.unwrap().code,
        "STALE_GENERATION"
    );
    let r = simple(&mut app, 2, "resume", 600);
    assert_eq!(r.generation, 3);
    assert!(!r.clock_trusted);
    assert!(simple(&mut app, 3, "state", 1000).effects.is_empty());
    assert!(
        !command(
            &mut app,
            3,
            "schedule_play",
            json!({"deadline_ms":1500}),
            1000
        )
        .ok
    );
}
#[test]
fn capabilities_and_errors_never_fake_player_readiness() {
    let mut app = Application::default();
    assert_eq!(
        simple(&mut app, 1, "play", 0).error.unwrap().code,
        "PLAYER_NOT_READY"
    );
    assert_eq!(
        command(&mut app, 1, "rate", json!({"rate":1.02}), 0)
            .error
            .unwrap()
            .code,
        "RATE_UNSUPPORTED"
    );
    let mut app = prepared();
    assert_eq!(
        command(&mut app, 1, "seek", json!({"position_ms":30001}), 0)
            .error
            .unwrap()
            .code,
        "SEEK_OUT_OF_RANGE"
    );
    assert_eq!(
        command(&mut app, 1, "rate", json!({"rate":12}), 0)
            .error
            .unwrap()
            .code,
        "INVALID_RATE"
    );
    assert!(!command(&mut app,1,"sample",json!({"position_ms":0,"duration_ms":100,"playing":false,"loaded":true,"buffering":false,"seeking":false,"age_ms":101}),0).ok);
    let r = simple(&mut app, 1, "player_error", 0);
    assert!(!r.sample.loaded);
    assert!(!r.clock_trusted);
    assert!(r.snapshot_required);
}
#[test]
fn c_abi_uses_caller_buffers_and_rejects_destroyed_handles() {
    let h = cine_bridge_create();
    let mut output = vec![0; 65536];
    let input = b"{\"api_version\":1,\"generation\":1,\"command\":{\"type\":\"state\"}}";
    let n = unsafe {
        cine_bridge_call(
            h,
            input.as_ptr(),
            input.len(),
            output.as_mut_ptr(),
            output.len(),
        )
    };
    let r: Value = serde_json::from_slice(&output[..n as usize]).unwrap();
    assert_eq!(r["api_version"], 1);
    assert_eq!(
        unsafe { cine_bridge_call(h, input.as_ptr(), input.len(), output.as_mut_ptr(), 1) },
        -2
    );
    assert_eq!(cine_bridge_hash_fd(h, -1), -6);
    assert_eq!(cine_bridge_destroy(h), 0);
    assert_eq!(cine_bridge_destroy(h), -1);
    assert_eq!(
        unsafe {
            cine_bridge_call(
                h,
                input.as_ptr(),
                input.len(),
                output.as_mut_ptr(),
                output.len(),
            )
        },
        -1
    );
}
#[test]
fn cancellation_intent_is_safe_without_worker() {
    let mut app = Application::default();
    assert!(simple(&mut app, 1, "cancel_hash", 0).ok);
    let r = simple(&mut app, 1, "suspend", 0);
    assert_eq!(r.effects.len(), 1);
}

#[test]
fn network_boundary_rejects_invalid_intents_and_offline_control_in_network_mode() {
    let mut app = Application::default();
    let invalid = serde_json::json!({"api_version":1,"generation":1,"command":{"type":"network","payload":{"action":"join","room_id":"invalid","room_epoch":"invalid","invite_token":"test"}}});
    assert_eq!(
        app.dispatch(invalid.to_string().as_bytes(), 0)
            .error
            .unwrap()
            .code,
        "INVALID_DTO"
    );
    let state = serde_json::json!({"api_version":1,"generation":1,"command":{"type":"network","payload":{"action":"ready"}}});
    assert_eq!(
        app.dispatch(state.to_string().as_bytes(), 0)
            .error
            .unwrap()
            .code,
        "MEDIA_NOT_READY"
    );
    let offline = serde_json::json!({"api_version":1,"generation":1,"command":{"type":"snapshot"}});
    assert_eq!(
        app.dispatch(offline.to_string().as_bytes(), 0)
            .error
            .unwrap()
            .code,
        "NETWORK_INTENT_REQUIRED"
    );
}

#[test]
fn desktop_intents_validate_paths_and_cannot_use_offline_or_mobile_truth() {
    let mut app = Application::default();
    for payload in [
        json!({"action":"select"}),
        json!({"action":"select","path":4}),
        json!({"action":"select","path":"fixture.mp4","token":"unexpected"}),
    ] {
        assert_eq!(
            command(&mut app, 1, "desktop_network", payload, 0)
                .error
                .unwrap()
                .code,
            "INVALID_DTO"
        );
    }
    assert!(
        command(
            &mut app,
            1,
            "desktop_network",
            json!({"action":"disconnect"}),
            0
        )
        .ok
    );
    assert_eq!(
        simple(&mut app, 1, "snapshot", 0).error.unwrap().code,
        "NETWORK_INTENT_REQUIRED"
    );
    assert_eq!(
        command(&mut app, 1, "network", json!({"action":"disconnect"}), 0)
            .error
            .unwrap()
            .code,
        "NETWORK_INTENT_REQUIRED"
    );
}
