use axum::{
    Router,
    extract::{
        State,
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
    routing::get,
};
use cine_protocol::{
    Incoming, MAX_MESSAGE_BYTES, WireMessage, decode, effect_message, encode, error_message,
    incoming,
};
use cine_rooms::{
    RoomService, ServiceConfig,
    model::{Delivery, Effect, ErrorCode, EventKind},
};
use serde_json::json;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{
    net::TcpListener,
    sync::{Semaphore, mpsc},
    time,
};
use uuid::Uuid;

const QUEUE_CAPACITY: usize = 32;
#[derive(Clone)]
pub struct Server {
    hub: Arc<Mutex<Hub>>,
    slots: Arc<Semaphore>,
}
struct Hub {
    service: RoomService,
    social: HashSet<Uuid>,
    senders: HashMap<Uuid, mpsc::Sender<Effect>>,
    boot: Instant,
    clock_epoch: Uuid,
}
impl Hub {
    fn now(&self) -> u64 {
        self.boot.elapsed().as_millis() as u64
    }
    fn dispatch(&mut self, deliveries: Vec<Delivery>) {
        let mut pending: VecDeque<_> = deliveries.into();
        while let Some(delivery) = pending.pop_front() {
            if let Effect::Event(e) = &delivery.effect {
                let event = match e.kind {
                    EventKind::MemberJoined => "room_joined",
                    EventKind::MemberLeft | EventKind::RoomClosed => "room_left",
                    EventKind::Play | EventKind::Pause | EventKind::Seek => "playback_scheduled",
                    EventKind::MemberStatus if e.reason == Some("resumed") => "client_resumed",
                    EventKind::MemberStatus if e.reason == Some("disconnected") => {
                        "client_disconnected"
                    }
                    _ => "room_updated",
                };
                tracing::info!(event,room_id=%e.state.room_id,sequence=e.state.sequence,event_type=e.kind.as_str());
            }
            for id in delivery.recipients {
                if matches!(delivery.effect, Effect::Social { .. }) && !self.social.contains(&id) {
                    continue;
                }
                if matches!(delivery.effect, Effect::Close) {
                    self.senders.remove(&id);
                    self.social.remove(&id);
                    continue;
                }
                let failed = self
                    .senders
                    .get(&id)
                    .is_some_and(|tx| tx.try_send(delivery.effect.clone()).is_err());
                if failed {
                    self.senders.remove(&id);
                    let now = self.now();
                    pending.extend(self.service.disconnect(id, now));
                    tracing::warn!(event="client_queue_full",connection_id=%id);
                }
            }
        }
    }
    fn remove(&mut self, id: Uuid) {
        self.social.remove(&id);
        self.senders.remove(&id);
        let deliveries = self.service.disconnect(id, self.now());
        self.dispatch(deliveries);
    }
}
impl Default for Server {
    fn default() -> Self {
        Self::new(ServiceConfig::default())
    }
}
impl Server {
    pub fn new(config: ServiceConfig) -> Self {
        Self {
            hub: Arc::new(Mutex::new(Hub {
                service: RoomService::new(config),
                social: HashSet::new(),
                senders: HashMap::new(),
                boot: Instant::now(),
                clock_epoch: Uuid::new_v4(),
            })),
            slots: Arc::new(Semaphore::new(256)),
        }
    }
    pub async fn serve(
        self,
        listener: TcpListener,
        stop: impl std::future::Future<Output = ()> + Send + 'static,
    ) -> std::io::Result<()> {
        let ticker = self.clone();
        let task = tokio::spawn(async move {
            let mut ticks = time::interval(Duration::from_millis(250));
            let mut count = 0;
            loop {
                ticks.tick().await;
                count += 1;
                let mut h = ticker.hub.lock().unwrap();
                let now = h.now();
                let effects = h.service.tick(now);
                h.dispatch(effects);
                if count % 20 == 0 {
                    let effects = h.service.sync_states(now);
                    h.dispatch(effects);
                }
            }
        });
        let app = Router::new()
            .route("/", get(upgrade))
            .route("/ws", get(upgrade))
            .with_state(self);
        let result = axum::serve(listener, app)
            .with_graceful_shutdown(stop)
            .await;
        task.abort();
        result
    }
}
async fn upgrade(ws: WebSocketUpgrade, State(server): State<Server>) -> Response {
    ws.max_message_size(MAX_MESSAGE_BYTES)
        .max_frame_size(MAX_MESSAGE_BYTES)
        .on_upgrade(move |socket| connection(socket, server))
}
async fn send(socket: &mut WebSocket, message: &WireMessage) -> bool {
    let Ok(text) = encode(message) else {
        return false;
    };
    matches!(
        time::timeout(
            Duration::from_secs(2),
            socket.send(Message::Text(text.into()))
        )
        .await,
        Ok(Ok(()))
    )
}
async fn connection(mut socket: WebSocket, server: Server) {
    let Ok(_permit) = server.slots.clone().try_acquire_owned() else {
        return;
    };
    let id = Uuid::new_v4();
    let (tx, mut rx) = mpsc::channel(QUEUE_CAPACITY);
    server.hub.lock().unwrap().senders.insert(id, tx);
    let mut negotiated = false;
    let mut rich_social = false;
    let mut hello_payload = None;
    let mut heartbeat = time::interval(Duration::from_secs(5));
    heartbeat.tick().await;
    let mut last_pong = Instant::now();
    let mut quota_updated = Instant::now();
    let mut quota = 40.0_f64;
    loop {
        tokio::select! {
            item=rx.recv()=>{
                let Some(effect)=item else {let _=socket.send(Message::Close(None)).await;break;};
                let (clock_epoch,at)={let h=server.hub.lock().unwrap();(h.clock_epoch,h.now())};
                let mut message=effect_message(&effect,clock_epoch,at);
                if !rich_social && let Some(message) = &mut message { cine_protocol::legacy_social(message); }
                if let Some(message)=message && !send(&mut socket,&message).await {break;}
            },
            _=heartbeat.tick()=>{
                if last_pong.elapsed()>Duration::from_secs(15){break;}
                if socket.send(Message::Ping(vec![].into())).await.is_err(){break;}
            },
            frame=socket.recv()=>{
                let Some(Ok(frame))=frame else {break};
                let text=match frame {
                    Message::Text(t)=>t,
                    Message::Pong(_)=>{last_pong=Instant::now();continue;},
                    Message::Ping(bytes)=>{if socket.send(Message::Pong(bytes)).await.is_err(){break;}continue;},
                    Message::Close(_)=>break,
                    _=>{let now=server.hub.lock().unwrap().now();if !send(&mut socket,&error_message(None,ErrorCode::InvalidEvent,now)).await {break;}continue;},
                };
                quota=(quota+quota_updated.elapsed().as_secs_f64()*20.0).min(40.0);
                quota_updated=Instant::now();
                if quota<1.0 {
                    let now=server.hub.lock().unwrap().now();
                    if !send(&mut socket,&error_message(None,ErrorCode::RateLimited,now)).await {break;}continue;
                }
                quota-=1.0;
                let t2=server.hub.lock().unwrap().now();
                let message=match decode(&text) {
                    Ok(m)=>m,
                    Err(code)=>{
                        tracing::warn!(event="protocol_error",code=code.as_str(),connection_id=%id);
                        if !send(&mut socket,&error_message(None,code,t2)).await {break;}
                        if code==ErrorCode::ProtocolVersionUnsupported {
                            let _=socket.send(Message::Close(Some(CloseFrame{code:1002,reason:"Unsupported protocol".into()}))).await;break;
                        }continue;
                    }
                };
                let input=match incoming(&message) {
                    Ok(i)=>i,Err(code)=>{
                        tracing::warn!(event="protocol_error",code=code.as_str(),connection_id=%id);
                        if !send(&mut socket,&error_message(Some(message.event_id),code,t2)).await {break;}continue;
                    }
                };
                match input {
                    Incoming::Hello{supported_versions,..}=>{
                        if !supported_versions.contains(&1) {
                            let _=send(&mut socket,&error_message(Some(message.event_id),ErrorCode::ProtocolVersionUnsupported,t2)).await;break;
                        }
                        if negotiated && (hello_payload.as_ref()!=Some(&message.payload) || server.hub.lock().unwrap().service.binding(id).is_some()) {
                            if !send(&mut socket,&error_message(Some(message.event_id),ErrorCode::InvalidEvent,t2)).await {break;}continue;
                        }
                        let social=message.payload.get("capabilities").and_then(|v|v.as_array()).is_some_and(|a|a.iter().any(|v|v.as_str()==Some("social_v1")));
                        rich_social=social && message.payload.get("capabilities").and_then(|v|v.as_array()).is_some_and(|a|a.iter().any(|v|v.as_str()==Some("rich_social_v1")));
                        if social {server.hub.lock().unwrap().social.insert(id);}
                        negotiated=true;hello_payload=Some(message.payload);
                        let response={let h=server.hub.lock().unwrap();WireMessage::server("SESSION_ACCEPT",json!({"capabilities":if rich_social {vec!["social_v1", "rich_social_v1"]} else if social {vec!["social_v1"]} else {vec![]},"selected_version":1,"connection_id":id,"clock_epoch":h.clock_epoch,
                            "limits":{"max_message_bytes":MAX_MESSAGE_BYTES,"max_members":16,"queue_capacity":QUEUE_CAPACITY,"lease_ms":30_000}}),h.now())};
                        if !send(&mut socket,&response).await {break;}
                    },
                    _ if !negotiated=>{
                        if !send(&mut socket,&error_message(Some(message.event_id),ErrorCode::NotAuthorized,t2)).await {break;}
                    },
                    Incoming::Ping{sample_id,t1_ms}=>{
                        let response={let h=server.hub.lock().unwrap();WireMessage::server("TIME_PONG",json!({"sample_id":sample_id,"clock_epoch":h.clock_epoch,"t1_ms":t1_ms,"t2_ms":t2,"t3_ms":h.now()}),h.now())};
                        if !send(&mut socket,&response).await {break;}
                    },
                    Incoming::Room(request)=>{
                        if matches!(request.command,cine_rooms::model::Command::Chat{..}|cine_rooms::model::Command::React{..}) && !server.hub.lock().unwrap().social.contains(&id) {
                            if !send(&mut socket,&error_message(Some(message.event_id),ErrorCode::FeatureNotSupported,t2)).await {break;}continue;
                        }
                        if matches!(request.command,cine_rooms::model::Command::RichMessage{..}|cine_rooms::model::Command::MessageReact{..}) && !rich_social {
                            if !send(&mut socket,&error_message(Some(message.event_id),ErrorCode::FeatureNotSupported,t2)).await {break;}continue;
                        }
                        let created=matches!(request.command,cine_rooms::model::Command::Create{..});
                        let mut h=server.hub.lock().unwrap();let now=h.now();let effects=h.service.execute(id,*request,now);
                        if created && effects.iter().any(|d|matches!(d.effect,Effect::Ack{..})) {tracing::info!(event="room_created",connection_id=%id);}
                        for d in &effects {if let Effect::Error{code,..}=d.effect {tracing::warn!(event="protocol_error",code=code.as_str(),connection_id=%id);}}
                        h.dispatch(effects);
                    }
                }
            }
        }
    }
    server.hub.lock().unwrap().remove(id);
}
