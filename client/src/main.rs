use cine_client::{Client, ClientError, require_ack};
use cine_core::player::Player;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, BufReader};

async fn command(client: &mut Client, parts: &[&str]) -> Result<Value, ClientError> {
    match parts {
        ["social-state"] => Ok(client.social_summary()),
        ["chat", text @ ..] => {
            client.send_chat(&text.join(" ")).await?;
            Ok(json!({"event":"chat_sent"}))
        }
        ["reaction", emoji] => {
            client.send_reaction(emoji).await?;
            Ok(json!({"event":"reaction_sent"}))
        }
        ["transfer-state"] => Ok(client.transfer_summary()),
        ["share", address] => {
            client.share(address.parse()?).await?;
            Ok(json!({"event":"offered"}))
        }
        ["receive", root] => {
            client.receive_file(std::path::Path::new(root)).await?;
            Ok(json!({"event":"requested"}))
        }
        ["transfer", action] => {
            client.transfer_action(action, None).await?;
            Ok(json!({"event":"transfer_action"}))
        }
        ["transfer", action, receiver] => {
            client
                .transfer_action(action, Some(receiver.parse()?))
                .await?;
            Ok(json!({"event":"transfer_action"}))
        }
        ["load-transfer"] => {
            let path = client.transfer_completed().ok_or("TRANSFER_INCOMPLETE")?;
            client.select(&path).await?;
            Ok(json!({"event":"transfer_loaded"}))
        }
        ["create"] => {
            let c = client.create().await?;
            // This explicit private terminal output is the invitation, never a tracing field.
            Ok(json!({"event":"created","room_id":c["room_id"],
                "room_epoch":c["room_epoch"],"invite_token":c["invite_token"]}))
        }
        ["join", room, epoch, token] => {
            client.join(room.parse()?, epoch.parse()?, token).await?;
            Ok(json!({"event":"joined"}))
        }
        ["media"] => Ok(client.media_summary()),
        ["hash-status"] => Ok(client.hash_status()),
        ["player-state"] | ["sync-state"] => Ok(client.sync_summary()),
        ["fault", kind] => {
            client.fault(kind, 0)?;
            Ok(json!({"event":"fault_applied"}))
        }
        ["fault", kind, value] => {
            client.fault(kind, value.parse()?)?;
            Ok(json!({"event":"fault_applied"}))
        }
        ["media-demo"] => {
            client.media_demo().await?;
            Ok(json!({"event":"media_selected"}))
        }
        ["ready"] => {
            client.ready().await?;
            Ok(json!({"event":"ready"}))
        }
        ["play"] | ["play", _] | ["pause"] | ["seek", _] => {
            let kind = match parts[0] {
                "play" => "PLAY_REQUEST",
                "pause" => "PAUSE_REQUEST",
                _ => "SEEK_REQUEST",
            };
            let position = if kind == "PAUSE_REQUEST" {
                None
            } else {
                Some(
                    parts
                        .get(1)
                        .map(|p| p.parse())
                        .transpose()?
                        .unwrap_or(client.player().position().map_err(|e| e.message)?),
                )
            };
            let reply = client.control(kind, position).await?;
            require_ack(&reply)?;
            Ok(
                json!({"event":"accepted","type":kind.trim_end_matches("_REQUEST"),
                "sequence":reply.payload["room_sequence"]}),
            )
        }
        ["sync"] => {
            let state = client.state().ok_or("No room")?;
            let reply = client
                .request(
                    "SYNC_REQUEST",
                    json!({"last_sequence":state.sequence,"reason":"manual"}),
                )
                .await?;
            if reply.kind != "ROOM_STATE" {
                require_ack(&reply)?;
            }
            Ok(client.state_summary())
        }
        ["state"] => Ok(client.state_summary()),
        ["disconnect"] => {
            client.disconnect().await;
            Ok(json!({"event":"disconnected"}))
        }
        ["resume"] => {
            client.resume().await?;
            Ok(json!({"event":"resumed"}))
        }
        ["resume-room"] => {
            client.resume_snapshot().await?;
            Ok(json!({"event":"resumed"}))
        }
        ["leave"] => {
            require_ack(&client.request("ROOM_LEAVE", json!({})).await?)?;
            Ok(json!({"event":"left"}))
        }
        ["quit"] => Ok(json!({"event":"quit"})),
        _ => Err(concat!(
            "Commands: create | join <room> <epoch> <invite> | media-demo | ready | ",
            "select <path> | media | hash-status | player-state | sync-state | ",
            "play [ms] | pause | seek <ms> | state | sync | disconnect | resume | leave | quit"
        )
        .into()),
    }
}

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    tracing_subscriber::fmt()
        .json()
        .with_writer(std::io::stderr)
        .init();
    let mut url = "ws://127.0.0.1:8765".to_owned();
    let mut name = "Participant".to_owned();
    let mut backend = "fake".to_owned();
    let mut visible = false;
    let mut allow_lan = false;
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() % 2 != 0 {
        return Err("Usage: cine-client [--server ws://127.0.0.1:8765] [--name Host] [--player fake|mpv] [--visible true|false] [--allow-lan true|false]".into());
    }
    for pair in args.chunks(2) {
        match pair[0].as_str() {
            "--server" => url = pair[1].clone(),
            "--name" => name = pair[1].clone(),
            "--player" => backend = pair[1].clone(),
            "--allow-lan" => allow_lan = pair[1].parse()?,
            "--visible" => visible = pair[1].parse()?,
            _ => return Err("Unknown argument".into()),
        }
    }
    let mut client = Client::connect_configured(&url, &name, &backend, visible, allow_lan).await?;
    println!(
        "{}",
        json!({"event":"cli_connected","clock":client.clock()})
    );
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    while let Some(line) = lines.next_line().await? {
        let parts: Vec<_> = line.split_whitespace().collect();
        let result = if let Some(path) = line.strip_prefix("select ") {
            client.select(std::path::Path::new(path)).await.map(|_|json!({"event":"media_selected","media":client.media_summary(),"hash":client.hash_status()}))
        } else {
            command(&mut client, &parts).await
        };
        match result {
            Ok(value) => println!("{value}"),
            Err(error) => println!(
                "{}",
                json!({"event":"command_error","message":error.to_string()})
            ),
        }
        if parts == ["quit"] {
            break;
        }
    }
    client.disconnect().await;
    Ok(())
}
