use crate::{
    player_backend::{ApplicationPlayer, BackendPlayer, PlayerView},
    replica::{Execution, Replica},
};
use cine_core::replica::Delivery;
use cine_core::{
    clock::ClockSample,
    media::{ContentIdentity, MediaDescriptor, SourceType},
};
use cine_local_media::LocalMedia;
use cine_protocol::{MAX_MESSAGE_BYTES, MediaDto, WireMessage, decode, encode, state_from_message};
use cine_rooms::model::RoomState;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::path::Path;
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
struct Session<P: ApplicationPlayer> {
    replica: Replica<P>,
    local: Option<LocalMedia>,
    local_descriptor: Option<MediaDescriptor>,
    hash_status: Value,
    announced_failure: bool,
    credentials: Option<Value>,
    corrections: [u64; 3],
    last_correction: &'static str,
}
pub struct Client<P: ApplicationPlayer = BackendPlayer> {
    url: String,
    name: String,
    boot: Instant,
    allow_lan: bool,
    session: Arc<Mutex<Session<P>>>,
    notify: Arc<Notify>,
    sender: Option<mpsc::Sender<Request>>,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl<P: ApplicationPlayer> Drop for Client<P> {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}
impl Client {
    pub async fn connect(url: &str, name: &str) -> Result<Self, ClientError> {
        Self::connect_with_player(url, name, "fake", false).await
    }
    pub async fn connect_with_player(
        url: &str,
        name: &str,
        backend: &str,
        visible: bool,
    ) -> Result<Self, ClientError> {
        Self::connect_configured(url, name, backend, visible, false).await
    }
    pub async fn connect_configured(
        url: &str,
        name: &str,
        backend: &str,
        visible: bool,
        allow_lan: bool,
    ) -> Result<Self, ClientError> {
        let boot = Instant::now();
        let player = BackendPlayer::new(backend, boot, visible)?;
        Self::connect_injected(url, name, player, boot, allow_lan).await
    }
}
impl<P: ApplicationPlayer + Send + 'static> Client<P> {
    pub async fn connect_injected(
        url: &str,
        name: &str,
        player: P,
        boot: Instant,
        allow_lan: bool,
    ) -> Result<Self, ClientError> {
        let mut client = Self {
            url: url.into(),
            name: name.into(),
            boot,
            allow_lan,
            session: Arc::new(Mutex::new(Session {
                replica: Replica::with_player(player),
                local: None,
                local_descriptor: None,
                hash_status: json!({"status":"idle"}),
                announced_failure: false,
                credentials: None,
                corrections: [0; 3],
                last_correction: "none",
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
        if !self.allow_lan
            && !self.url.starts_with("ws://127.0.0.1:")
            && !self.url.starts_with("ws://localhost:")
            && !self.url.starts_with("ws://[::1]:")
        {
            return Err("This spike uses ws:// loopback endpoints only".into());
        }
        if !self.url.starts_with("ws://") || self.url.len() > 512 {
            return Err("INVALID_ENDPOINT".into());
        }
        let config = WebSocketConfig::default()
            .max_message_size(Some(MAX_MESSAGE_BYTES))
            .max_frame_size(Some(MAX_MESSAGE_BYTES));
        let (mut ws, _) = time::timeout(
            Duration::from_secs(8),
            connect_async_with_config(&self.url, Some(config), true),
        )
        .await??;
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
            let mut samples: HashMap<Uuid, (u64, u64)> = HashMap::new();
            let mut sync_reply: Option<(Uuid, oneshot::Sender<WireMessage>)> = None;
            let mut warmup = time::interval(Duration::from_millis(100));
            let mut sync_tick = time::interval(Duration::from_millis(500));
            let mut player_tick = time::interval(Duration::from_millis(10));
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
                    _=player_tick.tick()=>{
                        if !shared.lock().unwrap().replica.suspended() {shared.lock().unwrap().replica.prepare_pending(now());}
                    },
                    _=sync_tick.tick()=>{
                        let correction={let mut s=shared.lock().unwrap();let c=s.replica.correct_drift(now());
                            s.last_correction=match c {cine_core::sync::Correction::None=>"none",cine_core::sync::Correction::SetRate(1.0)=>{s.corrections[1]+=1;"restore"},cine_core::sync::Correction::SetRate(_)=>{s.corrections[0]+=1;"rate"},cine_core::sync::Correction::Seek(_)=>{s.corrections[2]+=1;"seek"}};c};
                        match correction {
                            cine_core::sync::Correction::None=>{},
                            cine_core::sync::Correction::SetRate(rate)=>tracing::info!(event="sync_correction",kind="rate",rate),
                            cine_core::sync::Correction::Seek(position_ms)=>tracing::info!(event="sync_correction",kind="seek",position_ms),
                        }
                        let status={let s=shared.lock().unwrap();let r=&s.replica;let view=r.player.view();
                            if r.player.asynchronous() && r.connected && r.state.is_some(){
                                let target=r.target_position(view.sampled_at_ms);
                                tracing::info!(event="sync_sample",at_ms=now(),sample_at_ms=view.sampled_at_ms,sample_age_ms=now().saturating_sub(view.sampled_at_ms),ready=view.ready,room_ready=r.effective_ready(),server_ms=r.server_now(view.sampled_at_ms),sequence=r.state.as_ref().map(|s|s.sequence),
                                    position_ms=view.position_ms,target_ms=target,drift_ms=target.map(|t|view.position_ms as i64-t as i64),
                                    playing=view.playing,seeking=view.seeking,buffering=view.buffering,clock_trusted=r.trusted(now()),rate=view.rate,
                                    correction=match correction {cine_core::sync::Correction::None=>"none",cine_core::sync::Correction::SetRate(1.0)=>"restore",cine_core::sync::Correction::SetRate(_)=>"rate",cine_core::sync::Correction::Seek(_)=>"seek"});
                            }
                            let ready_member=r.member_id.is_some_and(|id|r.state.as_ref().is_some_and(|st|st.members.iter().any(|m|m.member_id==id && m.ready)));
                            (view.failed,view.buffering && !view.seeking,ready_member && !r.trusted(now()))};
                        let report={let mut s=shared.lock().unwrap();let failing=status.0||status.1||status.2;
                            if failing && !s.announced_failure {s.announced_failure=true;s.replica.state.as_ref().and_then(|st|st.media.as_ref().map(|m|(st.room_id,st.room_epoch,m.media_revision,s.replica.member_id)))}else{if !failing{s.announced_failure=false;}None}};
                        if let Some((room,epoch,revision,member))=report {
                            let message=WireMessage{protocol_version:1,event_id:Uuid::new_v4(),kind:"MEDIA_NOT_READY".into(),room_id:Some(room),room_epoch:Some(epoch),sender_id:member.map(|v|v.to_string()),sequence:None,sent_at_ms:now(),
                                payload:json!({"media_revision":revision,"reason":if status.0 {"player_error"}else if status.1 {"buffering"}else{"user"}})};
                            if ws.send(Message::Text(encode(&message).unwrap().into())).await.is_err(){break;}
                        }
                        notify.notify_waiters();
                    },
                    _=warmup.tick(),if negotiated=>{
                        let count=shared.lock().unwrap().replica.samples;
                        if !shared.lock().unwrap().replica.suspended() && (count<8 || now().saturating_sub(last_ping)>=5_000) {
                            if samples.len()>=8 {samples.clear();}
                            let id=Uuid::new_v4();let t1=now();samples.insert(id,(t1,shared.lock().unwrap().replica.clock_generation()));last_ping=t1;
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
                                let Some((t1,generation))=samples.remove(&id) else{continue};
                                let Some(t2)=message.payload["t2_ms"].as_u64() else{break};
                                let Some(t3)=message.payload["t3_ms"].as_u64() else{break};
                                let mut s=shared.lock().unwrap();
                                if message.payload["t1_ms"].as_u64()!=Some(t1)||message.payload["clock_epoch"].as_str()!=s.replica.clock_epoch.map(|id|id.to_string()).as_deref(){break;}
                                if s.replica.suspended() || s.replica.clock_generation()!=generation {continue;}
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
                                shared.lock().unwrap().replica.player.diagnostic(json!({"event":"authoritative_received","kind":message.kind,"at_ms":t4,
                                    "sent_at_server_ms":message.sent_at_ms,"sequence":state.sequence,"media_revision":state.media.as_ref().map(|m|m.media_revision),
                                    "deadline_server_ms":state.playback.as_ref().and_then(|p|p.pending.as_ref()).map(|p|p.execute_at_ms)}));
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
    pub fn player(&self) -> PlayerView {
        let mut s = self.session.lock().unwrap();
        s.replica.player.tick(self.now());
        s.replica.player.view()
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
    pub async fn attach_media(&self, descriptor: MediaDescriptor) -> Result<(), ClientError> {
        if !descriptor.valid_local() {
            return Err("INVALID_METADATA".into());
        }
        if let Some(m) = self.state().and_then(|s| s.media) {
            require_ack(
                &self
                    .request(
                        "MEDIA_NOT_READY",
                        json!({"media_revision":m.media_revision,"reason":"loading"}),
                    )
                    .await?,
            )?;
        }
        self.session.lock().unwrap().local_descriptor = Some(descriptor.clone());
        let state = self.state().ok_or("No room")?;
        if self.session.lock().unwrap().replica.member_id == Some(state.host_id) {
            require_ack(&self.request("MEDIA_SELECT_REQUEST", json!({"expected_sequence":state.sequence,"authority_revision":state.authority_revision,
                "previous_media_revision":state.media.as_ref().map_or(0,|m|m.media_revision),"descriptor":MediaDto::from(&descriptor)})).await?)?;
        }
        Ok(())
    }
    /// Rebind a freshly validated local file to the current room selection.
    /// Foreground recovery must not publish a new Host selection or timeline.
    pub async fn revalidate_media(&self, descriptor: MediaDescriptor) -> Result<(), ClientError> {
        self.session.lock().unwrap().local_descriptor = None;
        if !descriptor.valid_local() {
            return Err("INVALID_METADATA".into());
        }
        let state = self.state().ok_or("No room")?;
        let selected = state.media.ok_or("No media")?;
        if !descriptor.identity.matches(&selected.descriptor.identity) {
            return Err("MEDIA_MISMATCH".into());
        }
        self.session.lock().unwrap().local_descriptor = Some(descriptor);
        Ok(())
    }
    pub async fn suspend(&self) -> Result<(), ClientError> {
        {
            let mut s = self.session.lock().unwrap();
            s.replica.suspend(self.now());
        }
        if let Some(m) = self.state().and_then(|s| s.media) {
            require_ack(
                &self
                    .request(
                        "MEDIA_NOT_READY",
                        json!({"media_revision":m.media_revision,"reason":"user"}),
                    )
                    .await?,
            )?;
        }
        Ok(())
    }
    pub async fn recover_foreground(&self) -> Result<(), ClientError> {
        {
            self.session.lock().unwrap().replica.start_connection();
        }
        time::timeout(Duration::from_secs(8), async {
            while !self.session.lock().unwrap().replica.trusted(self.now())
                || self.session.lock().unwrap().replica.samples < 8
            {
                time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await?;
        let last = self.state().map_or(0, |s| s.sequence);
        let response = self
            .request(
                "SYNC_REQUEST",
                json!({"last_sequence":last,"reason":"resume"}),
            )
            .await?;
        if response.kind != "ROOM_STATE" {
            require_ack(&response)?;
        }
        Ok(())
    }
    pub fn hash_status(&self) -> Value {
        self.session.lock().unwrap().hash_status.clone()
    }
    pub fn media_summary(&self) -> Value {
        let s = self.session.lock().unwrap();
        let local = s.local_descriptor.as_ref();
        let selected = s.replica.state.as_ref().and_then(|st| st.media.as_ref());
        json!({"local_loaded":s.replica.player.view().ready,"duration_ms":local.map(|m|m.duration_ms),"size_bytes":local.map(|m|m.identity.size_bytes),
            "codecs":local.map(|m|&m.codecs),"mime":local.and_then(|m|m.mime.as_ref()),"media_revision":selected.map(|m|m.media_revision),
            "identity_match":local.zip(selected).map(|(a,b)|a.identity.matches(&b.descriptor.identity))})
    }
    pub fn sync_summary(&self) -> Value {
        let s = self.session.lock().unwrap();
        let r = &s.replica;
        let p = r.player.view();
        let sample = if r.player.asynchronous() {
            p.sampled_at_ms
        } else {
            self.now()
        };
        let target = r.target_position(sample);
        json!({"room_ready":r.effective_ready(),"snapshot_required":r.snapshot_required(),"sequence":r.state.as_ref().map(|s|s.sequence),"media_revision":r.state.as_ref().and_then(|s|s.media.as_ref().map(|m|m.media_revision)),"at_ms":self.now(),"server_ms":r.server_now(sample),"sample_age_ms":self.now().saturating_sub(sample),"ready":p.ready,"failed":p.failed,"position_ms":p.position_ms,"target_ms":target,"drift_ms":target.map(|t|p.position_ms as i64-t as i64),"playing":p.playing,"seeking":p.seeking,"buffering":p.buffering,"clock_trusted":r.trusted(self.now()),"uncertainty_ms":r.uncertainty(),"clock_diagnostics":r.clock_diagnostics(),"rate":p.rate,"correction":s.last_correction,"corrections":{"rate":s.corrections[0],"restore":s.corrections[1],"seek":s.corrections[2]}})
    }
    pub async fn ready(&self) -> Result<(), ClientError> {
        let state = self.state().ok_or("No room")?;
        let selected = state.media.ok_or("No media")?;
        let (descriptor, uncertainty) = {
            let s = self.session.lock().unwrap();
            let view = s.replica.player.view();
            validate_ready_player(&view)?;
            if s.replica.player.asynchronous()
                && self.now().saturating_sub(view.sampled_at_ms) > 100
            {
                return Err("MEDIA_NOT_READY".into());
            }
            if !s.replica.trusted(self.now()) {
                return Err("CLOCK_UNCERTAIN".into());
            }
            let descriptor = if let Some(local) = &s.local {
                local.handle.unchanged()?;
                s.local_descriptor.clone().ok_or("MEDIA_NOT_READY")?
            } else if let Some(descriptor) = &s.local_descriptor {
                descriptor.clone()
            } else if !s.replica.player.asynchronous() {
                demo_media()
            } else {
                return Err("MEDIA_NOT_READY".into());
            };
            (descriptor, s.replica.uncertainty().unwrap().ceil() as u64)
        };
        let r=self.request("MEDIA_METADATA",json!({"media_revision":selected.media_revision,"identity":cine_protocol::IdentityDto::from(&descriptor.identity),
            "duration_ms":descriptor.duration_ms,"mime":descriptor.mime,"codecs":descriptor.codecs})).await?;
        require_ack(&r)?;
        self.session
            .lock()
            .unwrap()
            .replica
            .local_reverification_required = false;
        let r=self.request("MEDIA_READY",json!({"media_revision":selected.media_revision,"clock_uncertainty_ms":uncertainty})).await?;
        require_ack(&r)
    }
    pub fn fault(&self, kind: &str, value: u64) -> Result<(), ClientError> {
        let mut s = self.session.lock().unwrap();
        match kind {
            "pause" => s.replica.player.pause()?,
            "clock" => s.replica.clock_blocked_until = self.now() + value,
            "seek" => s.replica.player.seek(value)?,
            _ => return Err("Unknown fault".into()),
        };
        Ok(())
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
        self.resume_snapshot().await?;
        self.ready().await
    }
    /// Resume transport and room truth without authorizing a new local Ready claim.
    pub async fn resume_snapshot(&mut self) -> Result<(), ClientError> {
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
        Ok(())
    }
    /// Read-only UI projection; no credentials or device handles.
    pub fn presentation_summary(&self) -> Value {
        let s = self.session.lock().unwrap();
        json!({"member_id":s.replica.member_id,
            "room":s.replica.state.as_ref().map(cine_protocol::StateDto::from)})
    }
    pub fn state_summary(&self) -> Value {
        let s = self.state();
        let p = self.player();
        json!({"connected":self.connected(),"room_id":s.as_ref().map(|s|s.room_id),"sequence":s.as_ref().map(|s|s.sequence),
            "host_id":s.as_ref().map(|s|s.host_id),"members":s.as_ref().map(|s|s.members.iter().map(|m|json!({"member_id":m.member_id,"connected":m.connected,"ready":m.ready})).collect::<Vec<_>>()),
            "playing":p.playing,"position_ms":cine_core::player::Player::position(&p).ok(),"rate":p.rate,"player_ready":p.ready,"seeking":p.seeking,
            "clock":self.clock().map(|(rtt,offset,count)|json!({"rtt_ms":rtt,"offset_ms":offset,"sample_count":count,"uncertainty_ms":self.session.lock().unwrap().replica.uncertainty()}))})
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

fn validate_ready_player(p: &PlayerView) -> Result<(), ClientError> {
    if !p.ready || p.failed || p.buffering || p.seeking || p.duration_ms == 0 {
        Err("MEDIA_NOT_READY".into())
    } else {
        Ok(())
    }
}
impl Client<BackendPlayer> {
    pub async fn select(&self, path: &Path) -> Result<(), ClientError> {
        if let Some(m) = self.state().and_then(|s| s.media) {
            require_ack(
                &self
                    .request(
                        "MEDIA_NOT_READY",
                        json!({"media_revision":m.media_revision,"reason":"loading"}),
                    )
                    .await?,
            )?;
        }
        {
            let mut s = self.session.lock().unwrap();
            s.local = None;
            s.local_descriptor = None;
            s.hash_status = json!({"status":"hashing"});
        }
        let shared = self.session.clone();
        let path = path.to_path_buf();
        let inspected=tokio::task::spawn_blocking(move||LocalMedia::inspect(&path,|p|{
            shared.lock().unwrap().hash_status=json!({"status":"hashing","read_bytes":p.read_bytes,"total_bytes":p.total_bytes});
            tracing::info!(event="hash_progress",read_bytes=p.read_bytes,total_bytes=p.total_bytes);true
        })).await?;
        let local = match inspected {
            Ok(m) => m,
            Err(e) => {
                self.session.lock().unwrap().hash_status =
                    json!({"status":"error","code":e.to_string()});
                return Err(e.into());
            }
        };
        self.session.lock().unwrap().hash_status = json!({"status":"complete","size_bytes":local.identity.size_bytes,"hash_ms":local.hash_ms});
        let real = self.session.lock().unwrap().replica.player.real();
        let duration = if let Some(real) = real {
            let path = local.handle.path().to_owned();
            tokio::task::spawn_blocking(move || real.load(path)).await??
        } else {
            let duration = local.probe.as_ref().ok_or("INVALID_METADATA")?.duration_ms;
            self.session
                .lock()
                .unwrap()
                .replica
                .player
                .configure_duration(duration);
            duration
        };
        let descriptor = local.descriptor(Uuid::new_v4().to_string(), duration)?;
        let size = local.identity.size_bytes;
        let hash_ms = local.hash_ms;
        {
            let mut s = self.session.lock().unwrap();
            s.hash_status = json!({"status":"complete","size_bytes":size,"hash_ms":hash_ms});
            s.local = Some(local);
            s.local_descriptor = Some(descriptor.clone());
        }
        let state = self.state().ok_or("No room")?;
        if self.session.lock().unwrap().replica.member_id == Some(state.host_id) {
            require_ack(&self.request("MEDIA_SELECT_REQUEST",json!({"expected_sequence":state.sequence,"authority_revision":state.authority_revision,
                "previous_media_revision":state.media.as_ref().map_or(0,|m|m.media_revision),"descriptor":MediaDto::from(&descriptor)})).await?)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ready_requires_loaded_usable_stable_player() {
        let valid = PlayerView {
            ready: true,
            duration_ms: 30000,
            ..Default::default()
        };
        assert!(validate_ready_player(&valid).is_ok());
        for invalid in [
            PlayerView {
                ready: false,
                ..valid.clone()
            },
            PlayerView {
                seeking: true,
                ..valid.clone()
            },
            PlayerView {
                buffering: true,
                ..valid.clone()
            },
            PlayerView {
                failed: true,
                ..valid.clone()
            },
            PlayerView {
                duration_ms: 0,
                ..valid.clone()
            },
        ] {
            assert_eq!(
                validate_ready_player(&invalid).unwrap_err().to_string(),
                "MEDIA_NOT_READY"
            );
        }
    }
}
