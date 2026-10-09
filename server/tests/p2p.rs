use cine_client::{Client, ClientError};
use cine_core::media::{MediaDescriptor, SourceType};
use cine_server::Server;
use cine_transfer::{CHUNK_SIZE, tls::Identity};
use cine_transfer_model::{TransferIntent, hex};
use serde_json::json;
use std::{fs::File, time::Duration};
use tokio::{net::TcpListener, sync::oneshot};
use uuid::Uuid;
async fn wait(mut predicate: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while !predicate() {
            tokio::time::sleep(Duration::from_millis(10)).await
        }
    })
    .await
    .unwrap()
}
#[tokio::test]
async fn authenticated_wss_room_grant_and_separate_tls_file_connection() -> Result<(), ClientError>
{
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let tls = Identity::generate_names(vec!["127.0.0.1".into()])?;
    let url = format!("wss://{address}/#tls={}", hex(&tls.certificate));
    let (tx, rx) = oneshot::channel();
    let server = tokio::spawn(Server::default().serve_tls(listener, tls.server, async {
        let _ = rx.await;
    }));
    let mut host = Client::connect(&url, "Host").await?;
    let mut receiver = Client::connect(&url, "Receiver").await?;
    assert_eq!(host.transfer_summary()["supported"], true);
    let invite = host.create().await?;
    receiver
        .join(
            invite["room_id"].as_str().unwrap().parse()?,
            invite["room_epoch"].as_str().unwrap().parse()?,
            invite["invite_token"].as_str().unwrap(),
        )
        .await?;
    host.wait_state(2).await?;
    let root = std::env::temp_dir().join(format!("cine-room-transfer-{}", Uuid::new_v4()));
    std::fs::create_dir(&root)?;
    let source = root.join("fixture");
    let bytes = vec![31; CHUNK_SIZE as usize * 2 + 99];
    std::fs::write(&source, &bytes)?;
    let identity = cine_local_media::hash_reader(File::open(&source)?, None, |_| true)?;
    host.attach_media(MediaDescriptor {
        media_id: Uuid::new_v4().to_string(),
        source_type: SourceType::LocalFile,
        title: None,
        duration_ms: 30000,
        identity,
        mime: None,
        codecs: vec![],
    })
    .await?;
    receiver.wait_state(host.state().unwrap().sequence).await?;
    let seq = host.state().unwrap().sequence;
    host.share_file("127.0.0.1:0".parse()?, File::open(&source)?)
        .await?;
    wait(|| receiver.transfer_summary()["offer"].is_object()).await;
    let id: Uuid = receiver.transfer_summary()["offer"]["transfer_id"]
        .as_str()
        .unwrap()
        .parse()?;
    let denied=receiver.request("P2P_TRANSFER_REQUEST",json!({"signal":TransferIntent::Accept{transfer_id:id,receiver_id:host.state().unwrap().host_id}})).await?;
    assert_eq!(denied.payload["error"]["code"], "NOT_AUTHORIZED");
    receiver.receive_file(&root).await?;
    wait(|| {
        host.transfer_summary()["receivers"]
            .as_array()
            .is_some_and(|a| a.len() == 1)
    })
    .await;
    let member: Uuid = host.transfer_summary()["receivers"][0]["receiver_id"]
        .as_str()
        .unwrap()
        .parse()?;
    assert!(receiver.transfer_completed().is_none());
    host.transfer_action("accept", Some(member)).await?;
    for _ in 0..100 {
        if receiver.transfer_completed().is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        receiver.transfer_completed().is_some(),
        "host={}, receiver={}",
        host.transfer_summary(),
        receiver.transfer_summary()
    );
    assert_eq!(
        std::fs::read(receiver.transfer_completed().unwrap())?,
        bytes
    );
    assert_eq!(host.state().unwrap().sequence, seq); // transfer never changes playback ordering
    assert!(
        !receiver
            .state()
            .unwrap()
            .members
            .iter()
            .find(|m| m.member_id == member)
            .unwrap()
            .ready
    );
    let summary = receiver.transfer_summary().to_string();
    assert!(
        !summary.contains("secret")
            && !summary.contains("certificate")
            && !summary.contains("/tmp/")
    );
    receiver.disconnect().await;
    host.disconnect().await;
    drop(receiver);
    drop(host);
    let _ = tx.send(());
    server.await??;
    std::fs::remove_dir_all(root)?;
    Ok(())
}
#[tokio::test]
async fn plain_ws_never_negotiates_or_authorizes_file_transfers() -> Result<(), ClientError> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("ws://{}", listener.local_addr()?);
    let (tx, rx) = oneshot::channel();
    let server = tokio::spawn(Server::default().serve(listener, async {
        let _ = rx.await;
    }));
    let mut client = Client::connect(&url, "Legacy").await?;
    client.create().await?;
    assert_eq!(client.transfer_summary()["supported"], false);
    let denied = client
        .request(
            "P2P_TRANSFER_REQUEST",
            json!({"signal":{"action":"request","transfer_id":Uuid::new_v4()}}),
        )
        .await?;
    assert_eq!(denied.payload["error"]["code"], "FEATURE_NOT_SUPPORTED");
    client.disconnect().await;
    let _ = tx.send(());
    server.await??;
    Ok(())
}
