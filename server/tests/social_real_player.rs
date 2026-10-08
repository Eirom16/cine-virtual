//! Opt-in Linux decoding/network regression. No surface/frame/Android assertions.
use cine_client::{Client, ClientError, require_ack};
use cine_server::Server;
use std::{path::Path, time::Duration};
use tokio::{net::TcpListener, sync::oneshot};
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires libmpv and generated synthetic corpus; no visual assertion"]
async fn two_real_libmpv_clients_rich_social_resume_preserves_playback() -> Result<(), ClientError>
{
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("ws://{}", listener.local_addr()?);
    let (stop_tx, stop_rx) = oneshot::channel();
    let server = tokio::spawn(Server::default().serve(listener, async {
        let _ = stop_rx.await;
    }));
    let mut a = Client::connect_with_player(&url, "Alex", "mpv", false).await?;
    let mut b = Client::connect_with_player(&url, "Sam", "mpv", false).await?;
    let invite = a.create().await?;
    b.join(
        Uuid::parse_str(invite["room_id"].as_str().unwrap())?,
        Uuid::parse_str(invite["room_epoch"].as_str().unwrap())?,
        invite["invite_token"].as_str().unwrap(),
    )
    .await?;
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../test-media/long-duration.mp4");
    a.select(&file).await?;
    b.wait_state(a.state().unwrap().sequence).await?;
    b.select(&file).await?;
    a.ready().await?;
    b.ready().await?;
    a.wait_state(b.state().unwrap().sequence).await?;
    let ack = a.control("PLAY_REQUEST", Some(10_000)).await?;
    require_ack(&ack)?;
    let seq = ack.payload["room_sequence"].as_u64().unwrap();
    let _ = tokio::try_join!(a.wait_execution(seq), b.wait_execution(seq))?;
    let before = a.state().unwrap();
    a.send_chat("¿Viste eso? 😂").await?;
    b.send_message(
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
        },
        None,
    )
    .await?;
    tokio::time::timeout(Duration::from_secs(4), async {
        while a.social_summary()["buffer_count"] != 3 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    let id = Uuid::parse_str(
        a.social_summary()["entries"][2]["message_id"]
            .as_str()
            .unwrap(),
    )?;
    a.send_message(
        cine_protocol::MessageContentDto::Text {
            text: "JAJAJA".into(),
        },
        Some(id),
    )
    .await?;
    tokio::time::timeout(Duration::from_secs(4), async {
        while b.social_summary()["buffer_count"] != 4 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    let reply_id = Uuid::parse_str(
        b.social_summary()["entries"][3]["message_id"]
            .as_str()
            .unwrap(),
    )?;
    b.react_message(reply_id, "❤️").await?;
    a.send_reaction("❤️").await?;
    b.send_reaction("😂").await?;
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            if a.social_summary()["buffer_count"] == 4
                && b.social_summary()["buffer_count"] == 4
                && a.social_summary()["reactions"].as_array().unwrap().len() == 2
                && b.social_summary()["reactions"].as_array().unwrap().len() == 2
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    assert_eq!(a.state().unwrap(), before);
    assert!(a.player().ready && b.player().ready);
    b.disconnect().await;
    a.wait_state(before.sequence + 1).await?;
    a.send_chat("Durante reconnect").await?;
    b.resume().await?;
    tokio::time::timeout(Duration::from_secs(4), async {
        while b.social_summary()["buffer_count"] != 6 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    let history = b.social_summary()["entries"].clone();
    assert_eq!(history[3]["reply_to_message_id"], id.to_string());
    assert_eq!(
        history[3]["message_reactions"]["❤️"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(
        b.social_summary()["reactions"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    a.wait_state(b.state().unwrap().sequence).await?;
    for (kind, position) in [
        ("PAUSE_REQUEST", None),
        ("SEEK_REQUEST", Some(45_000)),
        ("PLAY_REQUEST", Some(45_000)),
    ] {
        let ack = a.control(kind, position).await?;
        require_ack(&ack)?;
        let seq = ack.payload["room_sequence"].as_u64().unwrap();
        let _ = tokio::try_join!(a.wait_execution(seq), b.wait_execution(seq))?;
    }
    assert_eq!(b.social_summary()["entries"], history);
    a.disconnect().await;
    b.disconnect().await;
    let _ = stop_tx.send(());
    server.await??;
    Ok(())
}
