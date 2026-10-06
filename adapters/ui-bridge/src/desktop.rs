//! Presentation bridge to the existing desktop Client/LocalMedia/libmpv owner.
//! Video uses the existing native window; no new surface or sync implementation.
use crate::network::{Intent, safe_error};
use cine_client::{Client, player_backend::BackendPlayer, require_ack};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, watch};

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum DesktopIntent {
    Select(Selection),
    Room(Intent),
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    action: SelectAction,
    path: PathBuf,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SelectAction {
    Select,
}

pub struct Desktop {
    tx: mpsc::Sender<DesktopIntent>,
    stop: watch::Sender<bool>,
    join: Option<thread::JoinHandle<()>>,
    status: Arc<Mutex<Value>>,
}
fn publish(c: &Client<BackendPlayer>, status: &Mutex<Value>) {
    let mut v = status.lock().unwrap();
    v["connected"] = json!(c.connected());
    v["state"] = c.state_summary();
    v["sync"] = c.sync_summary();
    v["media"] = c.media_summary();
    v["presentation"] = c.presentation_summary();
    v["hash"] = c.hash_status();
}
impl Desktop {
    pub fn new(boot: Instant) -> Result<Self, &'static str> {
        let (tx, mut rx) = mpsc::channel(32);
        let (stop, mut stopped) = watch::channel(false);
        let status = Arc::new(Mutex::new(json!({"connected":false,"busy":false})));
        let published = status.clone();
        let join = thread::Builder::new().name("cine-desktop-ui".into()).spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            runtime.block_on(async move {
                let mut client: Option<Client<BackendPlayer>> = None;
                let mut tick = tokio::time::interval(Duration::from_millis(100));
                loop {
                    tokio::select! {
                        _ = stopped.changed() => break,
                        _ = tick.tick() => if let Some(c) = &client { publish(c, &published); },
                        cmd = rx.recv() => {
                            let Some(cmd) = cmd else { break };
                            let action = match &cmd {
                                DesktopIntent::Select(s) => { let _ = &s.action; "select" },
                                DesktopIntent::Room(i) => match i {
                                    Intent::Connect{..}=>"connect",Intent::Create=>"create",Intent::Join{..}=>"join",
                                    Intent::Ready=>"ready",Intent::Play=>"play",Intent::Pause=>"pause",Intent::Seek{..}=>"seek",
                                    Intent::Disconnect=>"disconnect",Intent::Reconnect=>"reconnect",Intent::Leave=>"leave",
                                    Intent::Suspend=>"suspend",Intent::Foreground=>"foreground",Intent::Attach=>"attach",
                                },
                            };
                            let operation = async {
                                if let DesktopIntent::Room(Intent::Connect{url,name,allow_lan}) = &cmd {
                                    if let Some(mut old) = client.take() { old.disconnect().await; }
                                    let player = tokio::task::spawn_blocking(move || BackendPlayer::new("mpv",boot,true)).await??;
                                    client = Some(Client::connect_injected(url,name,player,boot,*allow_lan).await?);
                                    return Ok(json!({}));
                                }
                                let c = client.as_mut().ok_or("NETWORK_DISCONNECTED")?;
                                match cmd {
                                    DesktopIntent::Select(s) => {
                                        let selection = c.select(&s.path);
                                        tokio::pin!(selection);
                                        loop { tokio::select! {
                                            r = &mut selection => {r?; break;},
                                            _ = tick.tick() => publish(c, &published),
                                        }}
                                        Ok(json!({}))
                                    },
                                    DesktopIntent::Room(i) => match i {
                                        Intent::Create => { let v=c.create().await?; Ok(json!({"room_id":v["room_id"],"room_epoch":v["room_epoch"],"invite_token":v["invite_token"]})) },
                                        Intent::Join{room_id,room_epoch,invite_token} => {c.join(room_id,room_epoch,&invite_token).await?;Ok(json!({}))},
                                        Intent::Ready => {c.ready().await?;Ok(json!({}))},
                                        Intent::Play|Intent::Pause|Intent::Seek{..} => {
                                            let (kind,p)=match i {Intent::Play=>("PLAY_REQUEST",Some(c.player().position_ms)),Intent::Pause=>("PAUSE_REQUEST",None),Intent::Seek{position_ms}=>("SEEK_REQUEST",Some(position_ms)),_=>unreachable!()};
                                            require_ack(&c.control(kind,p).await?)?;Ok(json!({}))
                                        },
                                        Intent::Disconnect => {c.disconnect().await;Ok(json!({}))},
                                        Intent::Reconnect => {c.resume_snapshot().await?;Ok(json!({}))},
                                        Intent::Leave => {require_ack(&c.request("ROOM_LEAVE",json!({})).await?)?;c.disconnect().await;Ok(json!({}))},
                                        Intent::Suspend => {c.suspend().await?;Ok(json!({}))},
                                        Intent::Foreground => {c.recover_foreground().await?;Ok(json!({}))},
                                        _ => Err("INVALID_DESKTOP_INTENT".into()),
                                    },
                                }
                            };
                            let result: Result<Value,cine_client::ClientError> = tokio::select! {
                                _ = stopped.changed() => break,
                                r = operation => r,
                            };
                            if let Some(c) = &client { publish(c,&published); }
                            let mut v=published.lock().unwrap();
                            v["busy"]=json!(false);v["last_action"]=json!(action);
                            v["error"]=result.as_ref().err().map(|e|json!(safe_error(&e.to_string()))).unwrap_or(Value::Null);
                            v["result"]=result.unwrap_or(Value::Null);
                        }
                    }
                }
                if let Some(mut c)=client { c.disconnect().await; }
            });
        }).map_err(|_| "ENGINE_START_FAILED")?;
        Ok(Self {
            tx,
            stop,
            join: Some(join),
            status,
        })
    }
    pub fn enqueue(&self, intent: DesktopIntent) -> Result<(), &'static str> {
        let mut s = self.status.lock().unwrap();
        self.tx.try_send(intent).map_err(|_| "NETWORK_QUEUE_FULL")?;
        s["busy"] = json!(true);
        s["last_action"] = Value::Null;
        Ok(())
    }
    pub fn status(&self) -> Value {
        self.status.lock().unwrap().clone()
    }
}
impl Drop for Desktop {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}
