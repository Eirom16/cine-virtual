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
