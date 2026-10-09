use cine_client::{Client, ClientError, require_ack};
use cine_core::player::Player;
use cine_server::Server;
use serde_json::json;
use tokio::{net::TcpListener, sync::oneshot};
use uuid::Uuid;

#[tokio::test]
async fn real_websocket_two_clients_control_and_resume() -> Result<(), ClientError> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("ws://{}", listener.local_addr()?);
    let (stop_tx, stop_rx) = oneshot::channel();
    let server = tokio::spawn(Server::default().serve(listener, async {
        let _ = stop_rx.await;
    }));
    let mut a = Client::connect(&url, "Host").await?;
    let mut b = Client::connect(&url, "Participant").await?;
    let invitation = a.create().await?;
    b.join(
        Uuid::parse_str(invitation["room_id"].as_str().unwrap())?,
        Uuid::parse_str(invitation["room_epoch"].as_str().unwrap())?,
        invitation["invite_token"].as_str().unwrap(),
    )
    .await?;
    a.wait_state(2).await?;
    assert_eq!(a.state().unwrap(), b.state().unwrap());
    assert_eq!(a.state().unwrap().members.len(), 2);
    assert!(a.clock().unwrap().2 >= 8 && b.clock().unwrap().2 >= 8);
    a.media_demo().await?;
    b.wait_state(a.state().unwrap().sequence).await?;
    let denied = a.control("PLAY_REQUEST", Some(100_000)).await?;
    assert_eq!(denied.payload["error"]["code"], "MEDIA_NOT_READY");
    a.ready().await?;
    b.ready().await?;
    a.wait_state(b.state().unwrap().sequence).await?;
    let before = a.state().unwrap().sequence;
    let denied = b.control("PLAY_REQUEST", Some(100_000)).await?;
    assert_eq!(denied.kind, "ERROR");
    assert_eq!(denied.payload["error"]["code"], "NOT_AUTHORIZED");
    assert_eq!(a.state().unwrap().sequence, before);
    assert_eq!(b.state().unwrap().sequence, before);
    let mut timings = vec![];
    for (kind, target) in [
        ("PLAY_REQUEST", Some(100_000)),
        ("PAUSE_REQUEST", None),
        ("SEEK_REQUEST", Some(120_000)),
    ] {
        let ack = a.control(kind, target).await?;
        require_ack(&ack)?;
        let seq = ack.payload["room_sequence"].as_u64().unwrap();
        assert_eq!(seq, a.state().unwrap().sequence);
        b.wait_state(seq).await?;
        let sa = a.state().unwrap();
        let sb = b.state().unwrap();
        assert_eq!(sa, sb);
        let p = sa.playback.as_ref().unwrap().pending.as_ref().unwrap();
        assert_eq!(p.execute_at_ms, sa.updated_at_ms + 500);
        assert_eq!(p.timeline_after.anchor_time_ms, p.execute_at_ms as i64);
        let (ea, eb) = tokio::try_join!(a.wait_execution(seq), b.wait_execution(seq))?;
        assert_eq!(ea.expected_server_ms, eb.expected_server_ms);
        assert!(ea.lateness_ms.abs() < 200.0 && eb.lateness_ms.abs() < 200.0);
        let difference = (ea.actual_server_ms - eb.actual_server_ms).abs();
        assert!(difference < 150.0);
        timings.push(json!({"type":kind,"sequence":seq,"lateness_a_ms":ea.lateness_ms,"lateness_b_ms":eb.lateness_ms,"execution_difference_ms":difference}));
        assert_eq!(a.player().playing, b.player().playing);
        assert!(
            a.player()
                .position()
                .unwrap()
                .abs_diff(b.player().position().unwrap())
                < 150
        );
    }
    let old_member = b.state().unwrap().members[1].member_id;
    let before = a.state().unwrap().sequence;
    b.disconnect().await;
    a.wait_state(before + 1).await?;
    assert!(!a.state().unwrap().members[1].connected);
    let ack = a.control("PLAY_REQUEST", Some(120_000)).await?;
    require_ack(&ack)?;
    a.wait_execution(ack.payload["room_sequence"].as_u64().unwrap())
        .await?;
    b.resume().await?;
    a.wait_state(b.state().unwrap().sequence).await?;
    assert_eq!(b.state().unwrap().members[1].member_id, old_member);
    assert!(b.player().playing);
    let resumed_difference = a
        .player()
        .position()
        .unwrap()
        .abs_diff(b.player().position().unwrap());
    assert!(resumed_difference < 150);
    let seq = a.state().unwrap().sequence;
    let snapshot = b
        .request(
            "SYNC_REQUEST",
            json!({"last_sequence":seq,"reason":"manual"}),
        )
        .await?;
    assert_eq!(snapshot.kind, "ROOM_STATE");
    assert_eq!(a.state().unwrap().sequence, seq);
    let ack = a.control("PAUSE_REQUEST", None).await?;
    require_ack(&ack)?;
    let seq = ack.payload["room_sequence"].as_u64().unwrap();
    tokio::try_join!(a.wait_execution(seq), b.wait_execution(seq))?;
    println!(
        "{}",
        json!({"scenario":"websocket_two_clients","timings":timings,"clock_a":a.clock(),"clock_b":b.clock(),"resume_position_difference_ms":resumed_difference,"final_sequence":seq})
    );
    b.disconnect().await;
    a.disconnect().await;
    let _ = stop_tx.send(());
    server.await??;
    Ok(())
}

#[tokio::test]
async fn forged_sender_cannot_grant_host_authority_over_websocket() -> Result<(), ClientError> {
    use cine_protocol::{WireMessage, decode, encode};
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::{connect_async, tungstenite::Message};
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("ws://{}", listener.local_addr()?);
    let (stop_tx, stop_rx) = oneshot::channel();
    let server = tokio::spawn(Server::default().serve(listener, async {
        let _ = stop_rx.await;
    }));
    let mut host = Client::connect(&url, "Host").await?;
    let c = host.create().await?;
    let (mut participant, _) = connect_async(url).await?;
    let mut message = WireMessage {
        protocol_version: 1,
        event_id: Uuid::new_v4(),
        kind: "SESSION_HELLO".into(),
        room_id: None,
        room_epoch: None,
        sender_id: None,
        sequence: None,
        sent_at_ms: 0,
        payload: json!({"supported_versions":[1],"client_name":"Spoof"}),
    };
    participant
        .send(Message::Text(encode(&message).unwrap().into()))
        .await?;
    let response = participant.next().await.unwrap()?;
    assert_eq!(decode(response.to_text()?).unwrap().kind, "SESSION_ACCEPT");
    message.event_id = Uuid::new_v4();
    message.kind = "ROOM_JOIN".into();
    message.room_id = Some(Uuid::parse_str(c["room_id"].as_str().unwrap())?);
    message.room_epoch = Some(Uuid::parse_str(c["room_epoch"].as_str().unwrap())?);
    message.payload = json!({"display_name":"Spoof","invite_token":c["invite_token"]});
    participant
        .send(Message::Text(encode(&message).unwrap().into()))
        .await?;
    for expected in ["ACK", "ROOM_STATE", "MEMBER_JOINED"] {
        let response = participant.next().await.unwrap()?;
        assert_eq!(decode(response.to_text()?).unwrap().kind, expected);
    }
    host.wait_state(2).await?;
    host.send_chat("compatible opt-in").await?;
    message.sender_id = Some(c["member_id"].as_str().unwrap().into());
    message.event_id = Uuid::new_v4();
    message.kind = "PLAY_REQUEST".into();
    message.payload =
        json!({"expected_sequence":2,"authority_revision":1,"media_revision":1,"position_ms":0});
    participant
        .send(Message::Text(encode(&message).unwrap().into()))
        .await?;
    let response = participant.next().await.unwrap()?;
    let response = decode(response.to_text()?).unwrap();
    assert_eq!(response.kind, "ERROR");
    assert_eq!(response.payload["error"]["code"], "NOT_AUTHORIZED");
    assert_eq!(host.state().unwrap().sequence, 2);
    participant.close(None).await?;
    host.disconnect().await;
    let _ = stop_tx.send(());
    server.await??;
    Ok(())
}

async fn wait_social(
    c: &Client,
    predicate: impl Fn(&serde_json::Value) -> bool,
) -> serde_json::Value {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let v = c.social_summary();
            if predicate(&v) {
                return v;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("social state did not converge")
}
#[tokio::test]
async fn social_two_clients_chat_reactions_resume_and_player_regression() -> Result<(), ClientError>
{
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("ws://{}", listener.local_addr()?);
    let (stop_tx, stop_rx) = oneshot::channel();
    let server = tokio::spawn(Server::default().serve(listener, async {
        let _ = stop_rx.await;
    }));
    let mut a = Client::connect(&url, "Alex").await?;
    let mut b = Client::connect(&url, "Sam").await?;
    let invite = a.create().await?;
    b.join(
        Uuid::parse_str(invite["room_id"].as_str().unwrap())?,
        Uuid::parse_str(invite["room_epoch"].as_str().unwrap())?,
        invite["invite_token"].as_str().unwrap(),
    )
    .await?;
    wait_social(&b, |v| v["social_sequence"] == 1).await;
    // The social snapshot does not guarantee the Host has applied the join.
    a.wait_state(b.state().unwrap().sequence).await?;
    a.media_demo().await?;
    b.wait_state(a.state().unwrap().sequence).await?;
    a.ready().await?;
    b.ready().await?;
    a.wait_state(b.state().unwrap().sequence).await?;
    let ack = a.control("PLAY_REQUEST", Some(1000)).await?;
    require_ack(&ack)?;
    a.wait_execution(ack.payload["room_sequence"].as_u64().unwrap())
        .await?;
    let state = a.state().unwrap();
    a.send_chat("¿Viste eso? 😂").await?;
    b.send_chat("Sí jajaja").await?;
    let s = wait_social(&a, |v| v["social_sequence"] == 3).await;
    let t = wait_social(&b, |v| v["social_sequence"] == 3).await;
    assert_eq!(s["entries"], t["entries"]);
    b.send_reaction("😂").await?;
    a.send_reaction("❤️").await?;
    wait_social(&a, |v| v["reactions"].as_array().unwrap().len() == 2).await;
    wait_social(&b, |v| v["reactions"].as_array().unwrap().len() == 2).await;
    assert_eq!(a.state().unwrap(), state);
    assert_eq!(b.state().unwrap(), state);
    let member = b.state().unwrap().members[1].member_id;
    b.disconnect().await;
    a.wait_state(state.sequence + 1).await?;
    a.send_chat("Mientras reconectabas").await?;
    b.resume_snapshot().await?;
    let t = wait_social(&b, |v| v["social_sequence"] == 5).await;
    assert_eq!(
        t["entries"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["kind"] == "chat")
            .count(),
        3
    );
    assert!(t["reactions"].as_array().unwrap().is_empty());
    assert_eq!(b.state().unwrap().members[1].member_id, member);
    b.ready().await?;
    a.wait_state(b.state().unwrap().sequence).await?;
    for (kind, pos) in [
        ("PAUSE_REQUEST", None),
        ("SEEK_REQUEST", Some(45_000)),
        ("PLAY_REQUEST", Some(45_000)),
    ] {
        let ack = a.control(kind, pos).await?;
        require_ack(&ack)?;
        let seq = ack.payload["room_sequence"].as_u64().unwrap();
        b.wait_state(seq).await?;
        let _ = tokio::try_join!(a.wait_execution(seq), b.wait_execution(seq))?;
    }
    assert_eq!(b.social_summary()["entries"], t["entries"]);
    a.disconnect().await;
    b.disconnect().await;
    let _ = stop_tx.send(());
    server.await??;
    Ok(())
}
#[tokio::test]
async fn social_spoofing_duplicate_ack_drop_and_resume() -> Result<(), ClientError> {
    use cine_protocol::{WireMessage, decode, encode};
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::{connect_async, tungstenite::Message};
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("ws://{}", listener.local_addr()?);
    let (stop_tx, stop_rx) = oneshot::channel();
    let server = tokio::spawn(Server::default().serve(listener, async {
        let _ = stop_rx.await;
    }));
    let mut host = Client::connect(&url, "Alex").await?;
    let invite = host.create().await?;
    let (mut raw, _) = connect_async(&url).await?;
    let mut m = WireMessage {
        protocol_version: 1,
        event_id: Uuid::new_v4(),
        kind: "SESSION_HELLO".into(),
        room_id: None,
        room_epoch: None,
        sender_id: None,
        sequence: None,
        sent_at_ms: 0,
        payload: json!({"supported_versions":[1],"client_name":"test","capabilities":["social_v1"]}),
    };
    raw.send(Message::Text(encode(&m).unwrap().into())).await?;
    let _ = raw.next().await.unwrap()?;
    m.event_id = Uuid::new_v4();
    m.kind = "ROOM_JOIN".into();
    m.room_id = Some(Uuid::parse_str(invite["room_id"].as_str().unwrap())?);
    m.room_epoch = Some(Uuid::parse_str(invite["room_epoch"].as_str().unwrap())?);
    m.payload = json!({"display_name":"Sam","invite_token":invite["invite_token"]});
    raw.send(Message::Text(encode(&m).unwrap().into())).await?;
    let credentials = loop {
        let frame = raw.next().await.unwrap()?;
        let response = decode(frame.to_text()?).unwrap();
        if response.kind == "ACK" {
            break response.payload["result"].clone();
        }
    };
    host.wait_state(2).await?;
    m.kind = "CHAT_SEND".into();
    m.event_id = Uuid::new_v4();
    m.sender_id = Some(invite["member_id"].as_str().unwrap().into());
    m.payload = json!({"text":"hola","sender_name":"Alex"});
    raw.send(Message::Text(encode(&m).unwrap().into())).await?;
    loop {
        let frame = raw.next().await.unwrap()?;
        if !frame.is_text() {
            continue;
        }
        let response = decode(frame.to_text()?).unwrap();
        if response.kind == "ERROR" {
            assert_eq!(response.payload["error"]["code"], "INVALID_EVENT");
            break;
        }
    }
    m.event_id = Uuid::new_v4();
    m.payload = json!({"text":"hola"});
    // Envelope sender is deliberately forged. Only the bound member may be attributed.
    raw.send(Message::Text(encode(&m).unwrap().into())).await?;
    loop {
        let frame = raw.next().await.unwrap()?;
        if !frame.is_text() {
            continue;
        }
        let response = decode(frame.to_text()?).unwrap();
        if response.kind == "CHAT_MESSAGE" && response.payload["kind"] == "chat" {
            assert_eq!(response.payload["sender_id"], credentials["member_id"]);
            assert_eq!(response.payload["display_name"], "Sam");
            assert_ne!(response.payload["message_id"], m.event_id.to_string());
            break;
        }
    }
    raw.send(Message::Text(encode(&m).unwrap().into())).await?;
    raw.close(None).await?;
    let (mut raw, _) = connect_async(&url).await?;
    let mut hello = m.clone();
    hello.kind = "SESSION_HELLO".into();
    hello.event_id = Uuid::new_v4();
    hello.room_id = None;
    hello.room_epoch = None;
    hello.sender_id = None;
    hello.payload =
        json!({"supported_versions":[1],"client_name":"test","capabilities":["social_v1"]});
    raw.send(Message::Text(encode(&hello).unwrap().into()))
        .await?;
    let _ = raw.next().await.unwrap()?;
    hello.kind = "ROOM_RESUME".into();
    hello.event_id = Uuid::new_v4();
    hello.room_id = m.room_id;
    hello.room_epoch = m.room_epoch;
    hello.payload = json!({"resume_token":credentials["resume_token"],"last_sequence":2});
    raw.send(Message::Text(encode(&hello).unwrap().into()))
        .await?;
    loop {
        let frame = raw.next().await.unwrap()?;
        if !frame.is_text() {
            continue;
        }
        let response = decode(frame.to_text()?).unwrap();
        if response.kind == "SOCIAL_STATE" {
            let chats: Vec<_> = response.payload["entries"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|e| e["kind"] == "chat")
                .collect();
            assert_eq!(chats.len(), 1);
            break;
        }
    }
    // Same intent across a new connection remains deduplicated in the member scope.
    raw.send(Message::Text(encode(&m).unwrap().into())).await?;
    loop {
        let frame = raw.next().await.unwrap()?;
        if !frame.is_text() {
            continue;
        }
        let response = decode(frame.to_text()?).unwrap();
        if response.kind == "SOCIAL_STATE" {
            assert_eq!(
                response.payload["entries"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|e| e["kind"] == "chat")
                    .count(),
                1
            );
            break;
        }
    }
    raw.close(None).await?;
    host.disconnect().await;
    let _ = stop_tx.send(());
    server.await??;
    Ok(())
}

fn rich_fixture() -> cine_protocol::MessageContentDto {
    cine_protocol::MessageContentDto::Gif {
        gif: cine_protocol::GifDto {
            provider: "fixture".into(),
            provider_content_id: "celebrate".into(),
            media_url: "https://fixtures.cine.invalid/celebrate.gif".into(),
            preview_url: None,
            width: 160,
            height: 100,
            alt_text: "Celebración".into(),
        },
    }
}
#[tokio::test]
async fn rich_two_clients_gif_reply_reactions_replay_and_player_isolation()
-> Result<(), ClientError> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("ws://{}", listener.local_addr()?);
    let (stop_tx, stop_rx) = oneshot::channel();
    let server = tokio::spawn(Server::default().serve(listener, async {
        let _ = stop_rx.await;
    }));
    let mut a = Client::connect(&url, "Alex").await?;
    let mut b = Client::connect(&url, "Sam").await?;
    let invite = a.create().await?;
    b.join(
        Uuid::parse_str(invite["room_id"].as_str().unwrap())?,
        Uuid::parse_str(invite["room_epoch"].as_str().unwrap())?,
        invite["invite_token"].as_str().unwrap(),
    )
    .await?;
    a.media_demo().await?;
    b.wait_state(a.state().unwrap().sequence).await?;
    a.ready().await?;
    b.ready().await?;
    a.wait_state(b.state().unwrap().sequence).await?;
    let ack = a.control("PLAY_REQUEST", Some(1000)).await?;
    require_ack(&ack)?;
    a.wait_execution(ack.payload["room_sequence"].as_u64().unwrap())
        .await?;
    let before = a.state().unwrap();
    a.send_chat("¿Viste esa escena?").await?;
    b.send_message(rich_fixture(), None).await?;
    let s = wait_social(&a, |v| v["social_sequence"] == 3).await;
    assert_eq!(s["rich_supported"], true);
    let gif_id = Uuid::parse_str(s["entries"][2]["message_id"].as_str().unwrap())?;
    a.send_message(
        cine_protocol::MessageContentDto::Text {
            text: "JAJAJA".into(),
        },
        Some(gif_id),
    )
    .await?;
    let s = wait_social(&b, |v| v["social_sequence"] == 4).await;
    let reply_id = Uuid::parse_str(s["entries"][3]["message_id"].as_str().unwrap())?;
    b.react_message(reply_id, "❤️").await?;
    let s = wait_social(&a, |v| v["social_sequence"] == 5).await;
    assert_eq!(
        s["entries"][3]["message_reactions"]["❤️"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    a.send_reaction("😂").await?;
    b.send_reaction("😂").await?;
    wait_social(&a, |v| v["reactions"].as_array().unwrap().len() == 2).await;
    b.react_message(reply_id, "❤️").await?;
    let s = wait_social(&a, |v| v["social_sequence"] == 6).await;
    assert!(s["entries"][3].get("message_reactions").is_none());
    assert_eq!(s["reactions"].as_array().unwrap().len(), 2);
    b.react_message(reply_id, "❤️").await?;
    wait_social(&a, |v| v["social_sequence"] == 7).await;
    assert_eq!(a.state().unwrap(), before);
    assert!(a.player().playing && b.player().playing);
    b.disconnect().await;
    a.wait_state(before.sequence + 1).await?;
    a.send_message(rich_fixture(), Some(reply_id)).await?;
    b.resume().await?;
    let s = wait_social(&b, |v| v["social_sequence"] == 9).await;
    let ids = s["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["message_id"].as_str().unwrap())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(ids.len(), s["entries"].as_array().unwrap().len());
    assert_eq!(s["entries"][3]["reply_to_message_id"], gif_id.to_string());
    assert_eq!(
        s["entries"][3]["message_reactions"]["❤️"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let ack = b
        .request(
            "SYNC_REQUEST",
            json!({"last_sequence":b.state().unwrap().sequence,"reason":"manual"}),
        )
        .await?;
    assert_eq!(ack.kind, "ROOM_STATE");
    assert_eq!(
        wait_social(&b, |v| v["social_sequence"] == 9).await["entries"],
        s["entries"]
    );
    assert!(b.player().ready);
    a.disconnect().await;
    b.disconnect().await;
    let _ = stop_tx.send(());
    server.await??;
    Ok(())
}

#[tokio::test]
async fn legacy_social_client_receives_safe_gif_fallback_and_cannot_send_rich()
-> Result<(), ClientError> {
    use cine_protocol::{WireMessage, decode, encode};
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::{connect_async, tungstenite::Message};
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("ws://{}", listener.local_addr()?);
    let (stop_tx, stop_rx) = oneshot::channel();
    let server = tokio::spawn(Server::default().serve(listener, async {
        let _ = stop_rx.await;
    }));
    let mut host = Client::connect(&url, "Alex").await?;
    let invite = host.create().await?;
    let (mut raw, _) = connect_async(&url).await?;
    let mut m = WireMessage {
        protocol_version: 1,
        event_id: Uuid::new_v4(),
        kind: "SESSION_HELLO".into(),
        room_id: None,
        room_epoch: None,
        sender_id: None,
        sequence: None,
        sent_at_ms: 0,
        payload: json!({"supported_versions":[1],"client_name":"old social","capabilities":["social_v1"]}),
    };
    raw.send(Message::Text(encode(&m).unwrap().into())).await?;
    let response = decode(raw.next().await.unwrap()?.to_text()?).unwrap();
    assert_eq!(response.payload["capabilities"], json!(["social_v1"]));
    m.event_id = Uuid::new_v4();
    m.kind = "ROOM_JOIN".into();
    m.room_id = Some(Uuid::parse_str(invite["room_id"].as_str().unwrap())?);
    m.room_epoch = Some(Uuid::parse_str(invite["room_epoch"].as_str().unwrap())?);
    m.payload = json!({"display_name":"Legacy","invite_token":invite["invite_token"]});
    raw.send(Message::Text(encode(&m).unwrap().into())).await?;
    host.wait_state(2).await?;
    host.send_message(rich_fixture(), None).await?;
    let id = tokio::time::timeout(std::time::Duration::from_secs(4), async {
        loop {
            let frame = raw.next().await.unwrap().unwrap();
            if !frame.is_text() {
                continue;
            }
            let msg = decode(frame.to_text().unwrap()).unwrap();
            if msg.kind == "CHAT_MESSAGE" && msg.payload["kind"] == "chat" {
                assert_eq!(msg.payload["text"], "[GIF]");
                assert!(msg.payload.get("content").is_none());
                assert!(msg.payload.get("reply_to_message_id").is_none());
                break Uuid::parse_str(msg.payload["message_id"].as_str().unwrap()).unwrap();
            }
        }
    })
    .await?;
    host.react_message(id, "❤️").await?;
    tokio::time::timeout(std::time::Duration::from_secs(4),async {
        loop {let frame=raw.next().await.unwrap().unwrap();if !frame.is_text(){continue;}let msg=decode(frame.to_text().unwrap()).unwrap();
        if msg.kind=="SOCIAL_STATE" && msg.payload["social_sequence"]==3 { let snapshot:cine_protocol::SocialSnapshotDto=serde_json::from_value(msg.payload.clone()).unwrap();snapshot.validate().unwrap();assert!(msg.payload["entries"].as_array().unwrap().iter().all(|e|e.get("content").is_none()&&e.get("message_reactions").is_none()));break;}}
    }).await?;
    m.event_id = Uuid::new_v4();
    m.kind = "MESSAGE_SEND".into();
    m.payload = json!({"content":rich_fixture(),"reply_to_message_id":null});
    raw.send(Message::Text(encode(&m).unwrap().into())).await?;
    tokio::time::timeout(std::time::Duration::from_secs(4), async {
        loop {
            let frame = raw.next().await.unwrap().unwrap();
            if !frame.is_text() {
                continue;
            }
            let msg = decode(frame.to_text().unwrap()).unwrap();
            if msg.kind == "ERROR" {
                assert_eq!(msg.payload["error"]["code"], "FEATURE_NOT_SUPPORTED");
                break;
            }
        }
    })
    .await?;
    raw.close(None).await?;
    host.disconnect().await;
    let _ = stop_tx.send(());
    server.await??;
    Ok(())
}
