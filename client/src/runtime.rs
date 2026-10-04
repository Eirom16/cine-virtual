use crate::replica::{Execution, Replica};
use cine_core::replica::Delivery;
use cine_core::{
    clock::ClockSample,
    media::{ContentIdentity, MediaDescriptor, SourceType},
};
use cine_protocol::{MAX_MESSAGE_BYTES, MediaDto, WireMessage, decode, encode, state_from_message};
use cine_rooms::model::RoomState;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{
    sync::{Notify, mpsc, oneshot},
    time,
};
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{Message, protocol::WebSocketConfig},
};
use uuid::Uuid;

pub type ClientError = Box<dyn std::error::Error + Send + Sync>;
struct Request {
    message: WireMessage,
    reply: oneshot::Sender<WireMessage>,
}
struct Session {
    replica: Replica,
    credentials: Option<Value>,
}
pub struct Client {
    url: String,
    name: String,
    boot: Instant,
    session: Arc<Mutex<Session>>,
    notify: Arc<Notify>,
    sender: Option<mpsc::Sender<Request>>,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl Drop for Client {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}
impl Client {
    pub async fn connect(url: &str, name: &str) -> Result<Self, ClientError> {
        let mut client = Self {
            url: url.into(),
            name: name.into(),
            boot: Instant::now(),
            session: Arc::new(Mutex::new(Session {
                replica: Replica::default(),
                credentials: None,
            })),
            notify: Arc::new(Notify::new()),
            sender: None,
            task: None,
        };
        client.start().await?;
        Ok(client)
    }
    pub fn now(&self) -> u64 {
        self.boot.elapsed().as_millis() as u64
    }
    async fn start(&mut self) -> Result<(), ClientError> {
        if !self.url.starts_with("ws://127.0.0.1:")
            && !self.url.starts_with("ws://localhost:")
            && !self.url.starts_with("ws://[::1]:")
        {
            return Err("This spike uses ws:// loopback endpoints only".into());
        }
        let config = WebSocketConfig::default()
            .max_message_size(Some(MAX_MESSAGE_BYTES))
            .max_frame_size(Some(MAX_MESSAGE_BYTES));
        let (mut ws, _) = connect_async_with_config(&self.url, Some(config), true).await?;
        let (tx, mut rx) = mpsc::channel::<Request>(32);
        self.sender = Some(tx);
        let (ready_tx, ready_rx) = oneshot::channel();
        let shared = self.session.clone();
        let notify = self.notify.clone();
        let boot = self.boot;
        let name = self.name.clone();
        {
            let mut s = shared.lock().unwrap();
            s.replica.start_connection();
        }
        self.task = Some(tokio::spawn(async move {
            let now = || boot.elapsed().as_millis() as u64;
            let hello = WireMessage {
                protocol_version: 1,
                event_id: Uuid::new_v4(),
                kind: "SESSION_HELLO".into(),
                room_id: None,
                room_epoch: None,
                sender_id: None,
                sequence: None,
                sent_at_ms: now(),
                payload: json!({"supported_versions":[1],"client_name":name}),
            };
            if ws
                .send(Message::Text(encode(&hello).unwrap().into()))
                .await
                .is_err()
            {
                return;
            }
            let mut ready_tx = Some(ready_tx);
            let mut pending: HashMap<Uuid, oneshot::Sender<WireMessage>> = HashMap::new();
            let mut samples: HashMap<Uuid, u64> = HashMap::new();
            let mut sync_reply: Option<(Uuid, oneshot::Sender<WireMessage>)> = None;
            let mut warmup = time::interval(Duration::from_millis(100));
            let mut sync_tick = time::interval(Duration::from_millis(500));
            let mut negotiated = false;
            let mut gap_requested = false;
            let mut last_ping = 0u64;
            loop {
                let deadline = {
                    let s = shared.lock().unwrap();
                    s.replica.deadline_local_ms(now())
                };
                let delay =
                    deadline.map_or(60_000.0, |deadline| (deadline - now() as f64).max(0.0));
                tokio::select! {
                    _=time::sleep(Duration::from_secs_f64(delay/1000.0)),if deadline.is_some()=>{
                        let record=shared.lock().unwrap().replica.execute_due(now());
                        if let Some(r)=record {tracing::info!(event="player_executed",sequence=r.sequence,expected_server_ms=r.expected_server_ms,
                            actual_server_ms=r.actual_server_ms,lateness_ms=r.lateness_ms,position_ms=r.position_ms,playing=r.playing);}
                        notify.notify_waiters();
                    },
                    _=sync_tick.tick()=>{
                        let correction=shared.lock().unwrap().replica.correct_drift(now());
                        match correction {
                            cine_core::sync::Correction::None=>{},
                            cine_core::sync::Correction::SetRate(rate)=>tracing::info!(event="sync_correction",kind="rate",rate),
                            cine_core::sync::Correction::Seek(position_ms)=>tracing::info!(event="sync_correction",kind="seek",position_ms),
                        }
                        notify.notify_waiters();
                    },
                    _=warmup.tick(),if negotiated=>{
                        let count=shared.lock().unwrap().replica.samples;
                        if count<8 || now().saturating_sub(last_ping)>=5_000 {
                            if samples.len()>=8 {samples.clear();}
                            let id=Uuid::new_v4();let t1=now();samples.insert(id,t1);last_ping=t1;
                            let ping=WireMessage{protocol_version:1,event_id:Uuid::new_v4(),kind:"TIME_PING".into(),room_id:None,room_epoch:None,
                                sender_id:shared.lock().unwrap().replica.member_id.map(|id|id.to_string()),sequence:None,sent_at_ms:t1,payload:json!({"sample_id":id,"t1_ms":t1})};
                            if ws.send(Message::Text(encode(&ping).unwrap().into())).await.is_err(){break;}
                        }
                    },
                    item=rx.recv()=>{
                        let Some(request)=item else{let _=ws.close(None).await;break;};
                        if pending.len()>=32 {break;}
                        let id=request.message.event_id;
                        let text=match encode(&request.message){Ok(t)=>t,Err(_)=>break};
                        if request.message.kind=="SYNC_REQUEST" { sync_reply=Some((id,request.reply)); } else { pending.insert(id,request.reply); }
                        if !matches!(time::timeout(Duration::from_secs(2),ws.send(Message::Text(text.into()))).await,Ok(Ok(()))){break;}
                    },
                    item=ws.next()=>{
                        let Some(Ok(frame))=item else{break};
                        let text=match frame {
                            Message::Text(t)=>t,Message::Ping(p)=>{if ws.send(Message::Pong(p)).await.is_err(){break;}continue;},
                            Message::Pong(_)=>continue,Message::Close(_)=>break,_=>break,
                        };
                        let t4=now();
                        let Ok(message)=decode(&text) else{break};
                        if message.sender_id.as_deref()!=Some("server"){break;}
                        match message.kind.as_str() {
                            "SESSION_ACCEPT"=>{
                                let Some(epoch)=message.payload["clock_epoch"].as_str().and_then(|s|Uuid::parse_str(s).ok()) else{break};
                                let mut s=shared.lock().unwrap();
                                if s.replica.clock_epoch.is_some_and(|old|old!=epoch){s.credentials=None;}
                                s.replica.set_clock_epoch(epoch);negotiated=true;
                            },
                            "TIME_PONG"=>{
                                let Some(id)=message.payload["sample_id"].as_str().and_then(|s|Uuid::parse_str(s).ok()) else{break};
                                let Some(t1)=samples.remove(&id) else{continue};
                                let Some(t2)=message.payload["t2_ms"].as_u64() else{break};
                                let Some(t3)=message.payload["t3_ms"].as_u64() else{break};
                                let mut s=shared.lock().unwrap();
                                if message.payload["t1_ms"].as_u64()!=Some(t1)||message.payload["clock_epoch"].as_str()!=s.replica.clock_epoch.map(|id|id.to_string()).as_deref(){break;}
                                s.replica.sample(ClockSample{t1:t1 as i64,t2:t2 as i64,t3:t3 as i64,t4:t4 as i64},t4);
                                if s.replica.samples>=8 && s.replica.trusted(t4) {
                                    let estimate=s.replica.estimate().unwrap();
                                    tracing::info!(event="clock_sync",sample_count=s.replica.samples,rtt_ms=estimate.rtt_ms,offset_ms=estimate.offset_ms,uncertainty_ms=s.replica.uncertainty().unwrap());
                                    if let Some(tx)=ready_tx.take(){let _=tx.send(());}
                                }
                            },
                            "ACK"|"ERROR"=>{
                                let Some(id)=message.payload["request_event_id"].as_str().and_then(|s|Uuid::parse_str(s).ok()) else{continue};
                                if message.kind=="ACK" && message.payload["result"]["resume_token"].is_string() {
                                    let mut s=shared.lock().unwrap();s.replica.member_id=message.payload["result"]["member_id"].as_str().and_then(|s|Uuid::parse_str(s).ok());
                                    s.credentials=Some(message.payload["result"].clone());
                                }
                                if sync_reply.as_ref().is_some_and(|(request,_)|*request==id) {
                                    let (_,tx)=sync_reply.take().unwrap();let _=tx.send(message);
                                } else if let Some(tx)=pending.remove(&id){let _=tx.send(message);}
                            },
                            "ROOM_CLOSED"=>{shared.lock().unwrap().replica.disconnect(now());break;},
                            _=>{
                                let Ok(Some(state))=state_from_message(&message) else{break};
                                let snapshot=message.kind=="ROOM_STATE"||message.kind=="SYNC_STATE";
                                if snapshot {
                                    if message.kind=="ROOM_STATE" && message.payload["clock_epoch"].as_str()!=shared.lock().unwrap().replica.clock_epoch.map(|id|id.to_string()).as_deref(){break;}
                                    gap_requested=false;
                                }
                                let result=shared.lock().unwrap().replica.install(state,snapshot,now());
                                if message.kind=="ROOM_STATE" && let Some((_,tx))=sync_reply.take() { let _=tx.send(message.clone()); }
                                if result==Delivery::WrongEpoch {break;}
                                if result==Delivery::NeedSnapshot && !gap_requested {
                                    gap_requested=true;
                                    let request={let s=shared.lock().unwrap();let Some(state)=s.replica.state.as_ref() else{break};
                                        WireMessage{protocol_version:1,event_id:Uuid::new_v4(),kind:"SYNC_REQUEST".into(),room_id:Some(state.room_id),room_epoch:Some(state.room_epoch),
                                            sender_id:s.replica.member_id.map(|id|id.to_string()),sequence:None,sent_at_ms:now(),payload:json!({"last_sequence":state.sequence,"reason":"gap"})}};
                                    if ws.send(Message::Text(encode(&request).unwrap().into())).await.is_err(){break;}
                                }
                            }
                        }
                        notify.notify_waiters();
                    }
                }
            }
            shared.lock().unwrap().replica.disconnect(now());
            notify.notify_waiters();
        }));
        time::timeout(Duration::from_secs(5), ready_rx).await??;
        Ok(())
    }
    pub fn state(&self) -> Option<RoomState> {
        self.session.lock().unwrap().replica.state.clone()
    }
    pub fn connected(&self) -> bool {
        self.session.lock().unwrap().replica.connected
    }
    pub fn executions(&self) -> Vec<Execution> {
        self.session
            .lock()
            .unwrap()
            .replica
            .executions
            .iter()
            .cloned()
            .collect()
    }
    pub fn player(&self) -> crate::fake_player::FakePlayer {
        let mut p = self.session.lock().unwrap().replica.player.clone();
        p.set_time(self.now());
        p
    }
    pub fn clock(&self) -> Option<(f64, f64, usize)> {
        let s = self.session.lock().unwrap();
        let e = s.replica.estimate()?;
        Some((e.rtt_ms, e.offset_ms, s.replica.samples))
    }
    pub async fn request(&self, kind: &str, payload: Value) -> Result<WireMessage, ClientError> {
        let (tx, rx) = oneshot::channel();
        let message = {
            let s = self.session.lock().unwrap();
            WireMessage {
                protocol_version: 1,
                event_id: Uuid::new_v4(),
                kind: kind.into(),
                room_id: if kind == "ROOM_CREATE" {
                    None
                } else {
                    s.replica.state.as_ref().map(|s| s.room_id)
                },
                room_epoch: if kind == "ROOM_CREATE" {
                    None
                } else {
                    s.replica.state.as_ref().map(|s| s.room_epoch)
                },
                sender_id: s.replica.member_id.map(|id| id.to_string()),
                sequence: None,
                sent_at_ms: self.now(),
                payload,
            }
        };
        self.sender
            .as_ref()
            .ok_or("Disconnected")?
            .send(Request { message, reply: tx })
            .await?;
        let reply = time::timeout(Duration::from_secs(3), rx).await??;
        if reply.kind == "ACK" && kind == "ROOM_LEAVE" {
            let mut s = self.session.lock().unwrap();
            s.replica.clear_room(self.now());
            s.credentials = None;
        }
        if reply.kind == "ACK"
            && kind != "ROOM_LEAVE"
            && let Some(seq) = reply.payload["room_sequence"].as_u64()
        {
            self.wait_state(seq).await?;
        }
        Ok(reply)
    }
    pub async fn wait_state(&self, seq: u64) -> Result<(), ClientError> {
        time::timeout(Duration::from_secs(3), async {
            loop {
                let notified = self.notify.notified();
                if self.state().is_some_and(|s| s.sequence >= seq) {
                    break;
                }
                notified.await;
            }
        })
        .await?;
        Ok(())
    }
    pub async fn wait_execution(&self, seq: u64) -> Result<Execution, ClientError> {
        time::timeout(Duration::from_secs(3), async {
            loop {
                let notified = self.notify.notified();
                if let Some(e) = self.executions().into_iter().find(|e| e.sequence == seq) {
                    return e;
                }
                notified.await;
            }
        })
        .await
        .map_err(Into::into)
    }
    pub async fn create(&self) -> Result<Value, ClientError> {
        let response = self
            .request("ROOM_CREATE", json!({"display_name":self.name}))
            .await?;
        require_ack(&response)?;
        Ok(response.payload["result"].clone())
    }
    pub async fn join(&self, room: Uuid, epoch: Uuid, token: &str) -> Result<(), ClientError> {
        let message = WireMessage {
            protocol_version: 1,
            event_id: Uuid::new_v4(),
            kind: "ROOM_JOIN".into(),
            room_id: Some(room),
            room_epoch: Some(epoch),
            sender_id: None,
            sequence: None,
            sent_at_ms: self.now(),
            payload: json!({"invite_token":token,"display_name":self.name}),
        };
        let (tx, rx) = oneshot::channel();
        self.sender
            .as_ref()
            .ok_or("Disconnected")?
            .send(Request { message, reply: tx })
            .await?;
        let reply = time::timeout(Duration::from_secs(3), rx).await??;
        require_ack(&reply)?;
        self.wait_state(
            reply.payload["room_sequence"]
                .as_u64()
                .ok_or("Invalid ACK")?,
        )
        .await
    }
    pub async fn media_demo(&self) -> Result<(), ClientError> {
        let s = self.state().ok_or("No room")?;
        let descriptor = MediaDto::from(&demo_media());
        let r=self.request("MEDIA_SELECT_REQUEST",json!({"expected_sequence":s.sequence,"authority_revision":s.authority_revision,
            "previous_media_revision":s.media.as_ref().map_or(0,|m|m.media_revision),"descriptor":descriptor})).await?;
        require_ack(&r)
    }
    pub async fn ready(&self) -> Result<(), ClientError> {
        let s = self.state().ok_or("No room")?;
        let m = s.media.ok_or("No media")?;
        if m.descriptor.identity != demo_media().identity {
            return Err("Selected identity differs from local synthetic content".into());
        }
        let uncertainty = {
            let session = self.session.lock().unwrap();
            if !session.replica.trusted(self.now()) {
                return Err("Clock not trusted".into());
            }
            session.replica.uncertainty().unwrap().ceil() as u64
        };
        let r=self.request("MEDIA_METADATA",json!({"media_revision":m.media_revision,"identity":cine_protocol::IdentityDto::from(&m.descriptor.identity),
            "duration_ms":m.descriptor.duration_ms,"mime":m.descriptor.mime,"codecs":m.descriptor.codecs})).await?;
        require_ack(&r)?;
        let r = self
            .request(
                "MEDIA_READY",
                json!({"media_revision":m.media_revision,"clock_uncertainty_ms":uncertainty}),
            )
            .await?;
        require_ack(&r)
    }
    pub async fn control(
        &self,
        kind: &str,
        position: Option<u64>,
    ) -> Result<WireMessage, ClientError> {
        let s = self.state().ok_or("No room")?;
        let mut payload = json!({"expected_sequence":s.sequence,"authority_revision":s.authority_revision,
            "media_revision":s.media.as_ref().ok_or("No media")?.media_revision});
        if let Some(pos) = position {
            payload["position_ms"] = json!(pos);
        }
        self.request(kind, payload).await
    }
    pub async fn disconnect(&mut self) {
        self.session.lock().unwrap().replica.disconnect(self.now());
        self.sender.take();
        if let Some(mut task) = self.task.take()
            && time::timeout(Duration::from_secs(2), &mut task)
                .await
                .is_err()
        {
            task.abort();
            let _ = task.await;
        }
    }
    pub async fn resume(&mut self) -> Result<(), ClientError> {
        if self.connected() {
            return Err("Disconnect before resume".into());
        }
        self.disconnect().await;
        let credentials = self
            .session
            .lock()
            .unwrap()
            .credentials
            .clone()
            .ok_or("No resume credentials")?;
        self.start().await?;
        if self.session.lock().unwrap().credentials.is_none() {
            return Err("Server clock epoch changed; resume credentials invalidated".into());
        }
        let message = WireMessage {
            protocol_version: 1,
            event_id: Uuid::new_v4(),
            kind: "ROOM_RESUME".into(),
            room_id: Some(Uuid::parse_str(
                credentials["room_id"].as_str().ok_or("Missing room")?,
            )?),
            room_epoch: Some(Uuid::parse_str(
                credentials["room_epoch"].as_str().ok_or("Missing epoch")?,
            )?),
            sender_id: None,
            sequence: None,
            sent_at_ms: self.now(),
            payload: json!({"resume_token":credentials["resume_token"],"last_sequence":self.state().map_or(0,|s|s.sequence)}),
        };
        let (tx, rx) = oneshot::channel();
        self.sender
            .as_ref()
            .unwrap()
            .send(Request { message, reply: tx })
            .await?;
        let reply = time::timeout(Duration::from_secs(3), rx).await??;
        require_ack(&reply)?;
        self.wait_state(
            reply.payload["room_sequence"]
                .as_u64()
                .ok_or("Invalid ACK")?,
        )
        .await?;
        self.ready().await
    }
    pub fn state_summary(&self) -> Value {
        let s = self.state();
        let p = self.player();
        json!({"connected":self.connected(),"room_id":s.as_ref().map(|s|s.room_id),"sequence":s.as_ref().map(|s|s.sequence),
            "host_id":s.as_ref().map(|s|s.host_id),"members":s.as_ref().map(|s|s.members.iter().map(|m|json!({"member_id":m.member_id,"connected":m.connected,"ready":m.ready})).collect::<Vec<_>>()),
            "playing":p.playing,"position_ms":cine_core::player::Player::position(&p).ok(),"rate":p.rate,
            "clock":self.clock().map(|(rtt,offset,count)|json!({"rtt_ms":rtt,"offset_ms":offset,"sample_count":count}))})
    }
}
pub fn require_ack(reply: &WireMessage) -> Result<(), ClientError> {
    if reply.kind == "ACK" {
        Ok(())
    } else {
        Err(reply.payload["error"]["code"]
            .as_str()
            .unwrap_or("INVALID_RESPONSE")
            .to_owned()
            .into())
    }
}
pub fn demo_media() -> MediaDescriptor {
    MediaDescriptor {
        media_id: Uuid::new_v4().to_string(),
        source_type: SourceType::LocalFile,
        title: Some("Synthetic spike media".into()),
        duration_ms: 300_000,
        identity: ContentIdentity {
            size_bytes: 1_024,
            sha256: [0xaa; 32],
        },
        mime: Some("video/mp4".into()),
        codecs: vec!["synthetic".into()],
    }
}
