//! One owner thread for the shared Rust client; bounded intents and SDK effects.
use cine_client::{
    Client,
    player_backend::{ApplicationPlayer, ControlMark, PlayerView},
    require_ack,
};
use cine_core::{
    media::MediaDescriptor,
    player::{Player, PlayerError, PlayerErrorCode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, watch};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Intent {
    Connect {
        url: String,
        name: String,
        allow_lan: bool,
    },
    TransferAddress,
    Share {
        address: std::net::SocketAddr,
    },
    Receive {
        root: std::path::PathBuf,
    },
    TransferControl {
        operation: String,
        receiver_id: Option<Uuid>,
    },
    LoadTransfer,
    Create,
    Chat {
        text: String,
    },
    Message {
        content: cine_protocol::MessageContentDto,
        reply_to_message_id: Option<Uuid>,
    },
    MessageReaction {
        message_id: Uuid,
        emoji: String,
    },
    Reaction {
        emoji: String,
    },
    Join {
        room_id: Uuid,
        room_epoch: Uuid,
        invite_token: String,
    },
    Attach,
    Revalidate,
    Ready,
    Play,
    Pause,
    Seek {
        position_ms: u64,
    },
    Disconnect,
    Reconnect,
    Leave,
    Suspend,
    Foreground,
}
#[derive(Clone, Serialize)]
pub struct NativeEffect {
    action: &'static str,
    value: f64,
    generation: u64,
    sequence: u64,
    deadline_server_ms: u64,
    offset_ms: f64,
    target_ms: u64,
    reason: &'static str,
    enqueued_ms: u64,
    operation_id: u64,
    media_revision: u64,
    received_at_ms: u64,
    deadline_local_ms: f64,
    wake_at_ms: u64,
    advance_target: bool,
    target_at_ms: u64,
    seek_lead_ms: u64,
}
struct NativeState {
    view: PlayerView,
    effects: VecDeque<NativeEffect>,
    generation: u64,
    active: bool,
    supports_rate: bool,
    next_operation: u64,
    diagnostics: VecDeque<Value>,
    diagnostics_dropped: u64,
    seek_timing: crate::seek_timing::SeekTiming,
    last_seek: Option<(u64, u64)>,
}
#[derive(Clone)]
pub struct MobilePlayer {
    shared: Arc<Mutex<NativeState>>,
    boot: Instant,
    mark: ControlMark,
}
impl MobilePlayer {
    fn new(boot: Instant, generation: u64, supports_rate: bool) -> Self {
        Self {
            shared: Arc::new(Mutex::new(NativeState {
                view: PlayerView {
                    rate: 1.0,
                    ..Default::default()
                },
                effects: VecDeque::new(),
                generation,
                active: true,
                supports_rate,
                next_operation: 0,
                diagnostics: VecDeque::new(),
                diagnostics_dropped: 0,
                seek_timing: crate::seek_timing::SeekTiming::default(),
                last_seek: None,
            })),
            boot,
            mark: ControlMark::default(),
        }
    }
    fn action(&mut self, action: &'static str, value: f64) -> Result<(), PlayerError> {
        let mut s = self.shared.lock().unwrap();
        if !s.active || s.effects.len() >= 32 {
            return Err(PlayerError {
                code: PlayerErrorCode::BackendFailure,
                message: "NATIVE_QUEUE_UNAVAILABLE".into(),
            });
        }
        // Avoid flooding a suspended/idle SDK with identical no-op intents.
        if action == "pause" && !s.view.playing && !s.effects.iter().any(|e| e.action == "play") {
            return Ok(());
        }
        if action == "rate"
            && (s.view.rate - value).abs() < 0.00001
            && !s.effects.iter().any(|e| e.action == "rate")
        {
            return Ok(());
        }
        let generation = s.generation;
        s.next_operation += 1;
        let operation_id = s.next_operation;
        let seek_lead_ms = s.seek_timing.estimate();
        s.effects.push_back(NativeEffect {
            action,
            value,
            generation,
            sequence: self.mark.sequence,
            deadline_server_ms: self.mark.deadline_server_ms,
            offset_ms: self.mark.offset_ms,
            target_ms: self.mark.target_ms,
            reason: self.mark.reason,
            enqueued_ms: self.boot.elapsed().as_millis() as u64,
            operation_id,
            media_revision: self.mark.media_revision,
            received_at_ms: self.mark.received_at_ms,
            deadline_local_ms: self.mark.deadline_local_ms,
            wake_at_ms: self.mark.wake_at_ms,
            advance_target: self.mark.playing && self.mark.reason != "prepare",
            target_at_ms: self.mark.target_at_ms,
            seek_lead_ms,
        });
        if action == "seek" {
            s.view.seeking = true;
            s.last_seek = Some((generation, operation_id));
        }
        Ok(())
    }
    pub fn sample(&self, mut view: PlayerView, supports_rate: bool) {
        let mut s = self.shared.lock().unwrap();
        if !s.active {
            return;
        }
        // A pre-dispatch SDK sample cannot acknowledge an undispatched seek.
        if s.effects.iter().any(|e| e.action == "seek") {
            view.seeking = true;
        }
        s.view = view;
        s.supports_rate = supports_rate;
    }
    pub fn drain(&self) -> Vec<NativeEffect> {
        let mut s = self.shared.lock().unwrap();
        let now = self.boot.elapsed().as_millis() as u64;
        let duration = s.view.duration_ms;
        s.effects
            .drain(..)
            .map(|mut e| {
                if e.action == "seek" && e.advance_target {
                    e.value = crate::seek_timing::project_target(
                        e.value as u64,
                        e.target_at_ms,
                        now,
                        e.seek_lead_ms,
                        true,
                        duration,
                    ) as f64;
                    e.target_at_ms = now;
                }
                e
            })
            .collect()
    }
    pub fn drain_diagnostics(&self) -> (Vec<Value>, u64) {
        let mut s = self.shared.lock().unwrap();
        (s.diagnostics.drain(..).collect(), s.diagnostics_dropped)
    }
    pub fn seek_loss(&self, generation: u64, operation: u64, loss_ms: u64) {
        let mut s = self.shared.lock().unwrap();
        if s.generation == generation && s.last_seek == Some((generation, operation)) {
            s.seek_timing.observe(loss_ms);
        }
    }
    fn reset(&self, generation: u64) {
        let mut s = self.shared.lock().unwrap();
        s.generation = generation;
        s.effects.clear();
        s.diagnostics.clear();
        s.seek_timing = crate::seek_timing::SeekTiming::default();
        s.last_seek = None;
        s.view = PlayerView {
            rate: 1.0,
            ..Default::default()
        };
    }
}
impl Player for MobilePlayer {
    fn play(&mut self) -> Result<(), PlayerError> {
        self.action("play", 0.0)
    }
    fn pause(&mut self) -> Result<(), PlayerError> {
        self.action("pause", 0.0)
    }
    fn seek(&mut self, p: u64) -> Result<(), PlayerError> {
        self.action("seek", p as f64)
    }
    fn position(&self) -> Result<u64, PlayerError> {
        Ok(self.shared.lock().unwrap().view.position_ms)
    }
    fn duration(&self) -> Result<u64, PlayerError> {
        Ok(self.shared.lock().unwrap().view.duration_ms)
    }
    fn supports_playback_rate(&self) -> bool {
        self.shared.lock().unwrap().supports_rate
    }
    fn set_playback_rate(&mut self, r: f64) -> Result<(), PlayerError> {
        if !self.supports_playback_rate() {
            return Err(PlayerError {
                code: PlayerErrorCode::UnsupportedRate,
                message: "RATE_UNSUPPORTED".into(),
            });
        }
        if !r.is_finite() || !(0.5..=2.0).contains(&r) {
            return Err(PlayerError {
                code: PlayerErrorCode::BackendFailure,
                message: "INVALID_RATE".into(),
            });
        }
        self.action("rate", r)
    }
}
impl ApplicationPlayer for MobilePlayer {
    fn tick(&mut self, _: u64) {}
    fn configure_duration(&mut self, _: u64) {}
    fn view(&self) -> PlayerView {
        self.shared.lock().unwrap().view.clone()
    }
    fn mark(&mut self, m: ControlMark) {
        self.mark = m;
    }
    fn diagnostic(&self, mut value: Value) {
        let mut s = self.shared.lock().unwrap();
        if s.diagnostics.len() == 256 {
            s.diagnostics.pop_front();
            s.diagnostics_dropped += 1;
        }
        value["generation"] = json!(s.generation);
        s.diagnostics.push_back(value);
    }
    fn asynchronous(&self) -> bool {
        true
    }
}
struct Command {
    intent: Intent,
    generation: u64,
    descriptor: Option<MediaDescriptor>,
    source: Option<std::fs::File>,
}
pub struct Network {
    tx: mpsc::Sender<Command>,
    stop: watch::Sender<bool>,
    join: Option<thread::JoinHandle<()>>,
    status: Arc<Mutex<Value>>,
    pub player: MobilePlayer,
    generation: Arc<Mutex<u64>>,
}
impl Network {
    pub fn new(boot: Instant, generation: u64, supports_rate: bool) -> Result<Self, &'static str> {
        let (tx, mut rx) = mpsc::channel::<Command>(32);
        let (stop, mut stopped) = watch::channel(false);
        let status = Arc::new(Mutex::new(json!({"connected":false,"busy":false})));
        let published = status.clone();
        let generation_shared = Arc::new(Mutex::new(generation));
        let current = generation_shared.clone();
        let player = MobilePlayer::new(boot, generation, supports_rate);
        let proxy = player.clone();
        let join=thread::Builder::new().name("cine-mobile-app".into()).spawn(move||{
            let runtime=tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            runtime.block_on(async move{
                let mut client:Option<Client<MobilePlayer>>=None;
                let mut verified_generation=None;
                let mut tick=tokio::time::interval(Duration::from_millis(50));
                loop { tokio::select! {
                    _=stopped.changed()=>break,
                    _=tick.tick()=>{
                        if let Some(c)=&client {let mut v=published.lock().unwrap();v["state"]=c.state_summary();v["sync"]=c.sync_summary();v["media"]=c.media_summary();v["presentation"]=c.presentation_summary();v["social"]=c.social_summary();v["transfer"]=c.transfer_summary();
                            v["connected"]=json!(c.connected());v["executions"]=json!(c.executions().iter().map(|e|json!({"sequence":e.sequence,"expected_server_ms":e.expected_server_ms,"actual_server_ms":e.actual_server_ms,"lateness_ms":e.lateness_ms})).collect::<Vec<_>>());}
                    },
                    cmd=rx.recv()=>{
                        let Some(cmd)=cmd else{break};
                        if cmd.generation!=*current.lock().unwrap() && !matches!(cmd.intent,Intent::Suspend){continue;}
                        published.lock().unwrap()["busy"]=json!(true);
                        let intent_name=String::from(match &cmd.intent {Intent::TransferAddress=>"transfer_address",Intent::Share{..}=>"share",Intent::Receive{..}=>"receive",Intent::TransferControl{..}=>"transfer_control",Intent::LoadTransfer=>"load_transfer",Intent::Connect{..}=>"connect",Intent::Create=>"create",Intent::Chat{..}=>"chat",Intent::Message{..}=>"message",Intent::MessageReaction{..}=>"message_reaction",Intent::Reaction{..}=>"reaction",Intent::Join{..}=>"join",Intent::Attach=>"attach",Intent::Revalidate=>"revalidate",Intent::Ready=>"ready",Intent::Play=>"play",Intent::Pause=>"pause",Intent::Seek{..}=>"seek",Intent::Disconnect=>"disconnect",Intent::Reconnect=>"reconnect",Intent::Leave=>"leave",Intent::Suspend=>"suspend",Intent::Foreground=>"foreground"});
                        let operation=async {
                            if let Intent::Connect{url,name,allow_lan}=&cmd.intent {
                                if let Some(mut old)=client.take(){old.disconnect().await;}
                                verified_generation=None;
                                client=Some(Client::connect_injected(url,name,proxy.clone(),boot,*allow_lan).await?);
                                return Ok(json!({"connected":true}));
                            }
                            let c=client.as_mut().ok_or("NETWORK_DISCONNECTED")?;
                            match cmd.intent {
                                Intent::TransferAddress=>Ok(json!({"address":c.transfer_address()})),
                                        Intent::Share{address}=>{c.share_file(address,cmd.source.ok_or("MEDIA_NOT_READY")?).await?;Ok(json!({}))},
                                Intent::Receive{root}=>{c.receive_file(&root).await?;Ok(json!({}))},
                                Intent::TransferControl{operation,receiver_id}=>{c.transfer_action(&operation,receiver_id).await?;Ok(json!({}))},
                                Intent::LoadTransfer=>{let path=c.transfer_completed().ok_or("TRANSFER_INCOMPLETE")?;Ok(json!({"path":path}))},
                                Intent::Chat{text}=>{c.send_chat(&text).await?;Ok(json!({}))},
                                Intent::Message{content,reply_to_message_id}=>{c.send_message(content,reply_to_message_id).await?;Ok(json!({}))},
                                Intent::MessageReaction{message_id,emoji}=>{c.react_message(message_id,&emoji).await?;Ok(json!({}))},
                                Intent::Reaction{emoji}=>{c.send_reaction(&emoji).await?;Ok(json!({}))},
                                Intent::Create=>{let v=c.create().await?;Ok(json!({"room_id":v["room_id"],"room_epoch":v["room_epoch"],"invite_token":v["invite_token"]}))},
                                Intent::Join{room_id,room_epoch,invite_token}=>{verified_generation=None;c.join(room_id,room_epoch,&invite_token).await?;Ok(json!({}))},
                                Intent::Attach=>{c.attach_media(cmd.descriptor.ok_or("MEDIA_NOT_READY")?).await?;verified_generation=Some(cmd.generation);Ok(json!({}))},
                                Intent::Revalidate=>{verified_generation=None;c.revalidate_media(cmd.descriptor.ok_or("MEDIA_NOT_READY")?).await?;verified_generation=Some(cmd.generation);Ok(json!({}))},
                                Intent::Ready=>{if verified_generation!=Some(cmd.generation){return Err("MEDIA_NOT_READY".into());}ready_when_usable(c,cmd.generation,&current).await?;Ok(json!({}))},
                                Intent::Play|Intent::Pause|Intent::Seek{..}=>{let(kind,p)=match cmd.intent{Intent::Play=>("PLAY_REQUEST",Some(c.player().position_ms)),Intent::Pause=>("PAUSE_REQUEST",None),Intent::Seek{position_ms}=>("SEEK_REQUEST",Some(position_ms)),_=>unreachable!()};let r=c.control(kind,p).await?;require_ack(&r)?;Ok(json!({"sequence":r.payload["room_sequence"]}))},
                                Intent::Disconnect=>{c.disconnect().await;Ok(json!({}))},
                                Intent::Reconnect=>{c.resume_snapshot().await?;if verified_generation==Some(cmd.generation){ready_when_usable(c,cmd.generation,&current).await?;}Ok(json!({"snapshot":true,"media_verified":verified_generation==Some(cmd.generation)}))},
                                Intent::Leave=>{verified_generation=None;require_ack(&c.request("ROOM_LEAVE",json!({})).await?)?;Ok(json!({}))},
                                Intent::Suspend=>{c.suspend().await?;Ok(json!({}))},
                                Intent::Foreground=>{c.recover_foreground().await?;Ok(json!({"snapshot":true}))},
                                Intent::Connect{..}=>unreachable!(),
                            }
                        };
                        let result:Result<Value,cine_client::ClientError>=tokio::select!{_ = stopped.changed()=>break,r=operation=>r};
                        if cmd.generation==*current.lock().unwrap(){let mut v=published.lock().unwrap();v["busy"]=json!(false);v["last_action"]=json!(intent_name);v["error"]=result.as_ref().err().map(|e|json!(safe_error(&e.to_string()))).unwrap_or(Value::Null);v["result"]=result.unwrap_or(Value::Null);}
                    }
                }}
                if let Some(mut c)=client{c.disconnect().await;}
                let mut s=proxy.shared.lock().unwrap();s.active=false;s.effects.clear();
            });
        }).map_err(|_|"ENGINE_START_FAILED")?;
        Ok(Self {
            tx,
            stop,
            join: Some(join),
            status,
            player,
            generation: generation_shared,
        })
    }
    pub fn enqueue(
        &self,
        intent: Intent,
        generation: u64,
        descriptor: Option<MediaDescriptor>,
    ) -> Result<(), &'static str> {
        self.enqueue_source(intent, generation, descriptor, None)
    }
    pub fn enqueue_source(
        &self,
        intent: Intent,
        generation: u64,
        descriptor: Option<MediaDescriptor>,
        source: Option<std::fs::File>,
    ) -> Result<(), &'static str> {
        let mut s = self.status.lock().unwrap();
        self.tx
            .try_send(Command {
                intent,
                generation,
                descriptor,
                source,
            })
            .map_err(|_| "NETWORK_QUEUE_FULL")?;
        s["last_action"] = Value::Null;
        s["busy"] = json!(true);
        Ok(())
    }
    pub fn status(&self) -> Value {
        self.status.lock().unwrap().clone()
    }
    pub fn reset(&self, generation: u64) {
        *self.generation.lock().unwrap() = generation;
        self.player.reset(generation);
    }
}
impl Drop for Network {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
async fn ready_when_usable(
    client: &Client<MobilePlayer>,
    generation: u64,
    current: &Mutex<u64>,
) -> Result<(), cine_client::ClientError> {
    let started = Instant::now();
    loop {
        if generation != *current.lock().unwrap() {
            return Err("OPERATION_CANCELLED".into());
        }
        match client.ready().await {
            Ok(()) => return Ok(()),
            Err(e)
                if e.to_string() == "MEDIA_NOT_READY"
                    && started.elapsed() < Duration::from_secs(5) =>
            {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            Err(e) => return Err(e),
        }
    }
}
pub(crate) fn safe_error(e: &str) -> &'static str {
    match e {
        "TLS_PIN_REQUIRED" => "TLS_PIN_REQUIRED",
        "TRANSFER_Space" => "TRANSFER_SPACE",
        "TRANSFER_Storage" => "TRANSFER_STORAGE",
        "TRANSFER_INCOMPLETE" => "TRANSFER_INCOMPLETE",
        "TRANSFER_Unauthorized" => "NOT_AUTHORIZED",
        "RATE_LIMITED" => "RATE_LIMITED",
        "PAYLOAD_TOO_LARGE" => "PAYLOAD_TOO_LARGE",
        "INVALID_EVENT" => "INVALID_EVENT",
        "FEATURE_NOT_SUPPORTED" => "FEATURE_NOT_SUPPORTED",
        "NOT_AUTHORIZED" => "NOT_AUTHORIZED",
        "OPERATION_CANCELLED" => "OPERATION_CANCELLED",
        "NETWORK_DISCONNECTED" => "NETWORK_DISCONNECTED",
        "INVALID_ENDPOINT" => "INVALID_ENDPOINT",
        "MEDIA_MISMATCH" => "MEDIA_MISMATCH",
        "MEDIA_NOT_READY" => "MEDIA_NOT_READY",
        "ROOM_NOT_FOUND" => "ROOM_NOT_FOUND",
        "INVITE_INVALID" => "INVITE_INVALID",
        "ROOM_FULL" => "ROOM_FULL",
        "ROOM_CLOSED" => "ROOM_CLOSED",
        "RESUME_EXPIRED" => "RESUME_EXPIRED",
        "PROTOCOL_VERSION_UNSUPPORTED" => "PROTOCOL_VERSION_UNSUPPORTED",
        "CONTROL_PENDING" => "CONTROL_PENDING",
        "INVALID_STATE" => "INVALID_STATE",
        "STALE_MEDIA" => "STALE_MEDIA",
        "STALE_AUTHORITY" => "STALE_AUTHORITY",
        "CLOCK_UNCERTAIN" => "CLOCK_UNTRUSTED",
        "OUT_OF_SEQUENCE" => "OUT_OF_SEQUENCE",
        _ if e.contains("deadline has elapsed") => "NETWORK_TIMEOUT",
        _ if e.contains("Connection refused") => "NETWORK_UNREACHABLE",
        _ => "NETWORK_OR_PLAYER_ERROR",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timing_correlation_is_unique_across_generation_and_sink_is_bounded() {
        let mut p = MobilePlayer::new(Instant::now(), 1, true);
        p.mark(ControlMark {
            sequence: 7,
            media_revision: 3,
            ..Default::default()
        });
        p.seek(100).unwrap();
        let first = p.drain().remove(0);
        assert_eq!(first.sequence, 7);
        assert_eq!(first.media_revision, 3);
        p.reset(2);
        p.seek(200).unwrap();
        let second = p.drain().remove(0);
        assert!(second.operation_id > first.operation_id);
        assert_eq!(second.generation, 2);
        for at in 0..300 {
            p.diagnostic(json!({"event":"fixture","at_ms":at}));
        }
        let (records, dropped) = p.drain_diagnostics();
        assert_eq!(records.len(), 256);
        assert_eq!(dropped, 44);
        assert_eq!(records[0]["at_ms"], 44);
        assert_eq!(records[0]["generation"], 2);
        assert!(p.drain_diagnostics().0.is_empty());
    }
    #[test]
    fn completion_measurements_cannot_cross_operation_or_generation() {
        let mut p = MobilePlayer::new(Instant::now(), 1, true);
        p.seek(100).unwrap();
        let first = p.drain().remove(0);
        p.seek_loss(1, first.operation_id, 400);
        assert_eq!(p.shared.lock().unwrap().seek_timing.estimate(), 400);
        p.reset(2);
        p.seek_loss(1, first.operation_id, 900);
        assert_eq!(p.shared.lock().unwrap().seek_timing.estimate(), 0);
        p.seek(200).unwrap();
        let second = p.drain().remove(0);
        p.seek_loss(2, first.operation_id, 900);
        assert_eq!(p.shared.lock().unwrap().seek_timing.estimate(), 0);
        p.seek_loss(2, second.operation_id, 300);
        assert_eq!(p.shared.lock().unwrap().seek_timing.estimate(), 300);
    }
    #[test]
    fn paused_stable_observation_does_not_invalidate_later_playing_seek_measurement() {
        let mut p = MobilePlayer::new(Instant::now(), 1, true);
        p.seek(100).unwrap();
        let effect = p.drain().remove(0);
        p.sample(
            PlayerView {
                position_ms: 100,
                ready: true,
                ..Default::default()
            },
            true,
        );
        assert!(!p.view().seeking);
        p.sample(
            PlayerView {
                position_ms: 110,
                playing: true,
                ready: true,
                ..Default::default()
            },
            true,
        );
        p.seek_loss(effect.generation, effect.operation_id, 585);
        assert_eq!(p.shared.lock().unwrap().seek_timing.estimate(), 585);
    }
    #[test]
    fn native_queue_is_bounded_and_generation_reset_discards_old_effects() {
        let mut p = MobilePlayer::new(Instant::now(), 1, true);
        for _ in 0..32 {
            p.play().unwrap();
        }
        assert!(p.play().is_err());
        p.reset(2);
        assert!(p.drain().is_empty());
        p.seek(100).unwrap();
        p.sample(PlayerView::default(), true);
        assert!(p.view().seeking);
        let fx = p.drain();
        assert_eq!(fx[0].generation, 2);
        assert!(p.view().seeking);
        p.sample(
            PlayerView {
                position_ms: 100,
                ready: true,
                ..Default::default()
            },
            false,
        );
        assert!(!p.view().seeking);
        assert!(!p.supports_playback_rate());
        assert_eq!(
            p.set_playback_rate(1.02).unwrap_err().code,
            PlayerErrorCode::UnsupportedRate
        );
    }
    #[tokio::test]
    async fn stale_network_generation_does_not_connect_and_destroy_closes_effect_queue() {
        let n = Network::new(Instant::now(), 1, true).unwrap();
        n.reset(2);
        n.enqueue(
            Intent::Connect {
                url: "ws://127.0.0.1:1".into(),
                name: "stale".into(),
                allow_lan: false,
            },
            1,
            None,
        )
        .unwrap();
        n.enqueue(Intent::Ready, 2, None).unwrap();
        assert_eq!(done(&n, "ready").await["error"], "NETWORK_DISCONNECTED");
        let mut proxy = n.player.clone();
        drop(n);
        assert!(proxy.play().is_err());
        proxy.sample(
            PlayerView {
                position_ms: 123,
                ..Default::default()
            },
            true,
        );
        assert_ne!(proxy.view().position_ms, 123);
        assert!(proxy.drain().is_empty());
    }
    #[test]
    fn safe_network_errors_never_forward_arbitrary_details() {
        assert_eq!(safe_error("deadline has elapsed"), "NETWORK_TIMEOUT");
        assert_eq!(
            safe_error("details with private media location"),
            "NETWORK_OR_PLAYER_ERROR"
        );
        assert_eq!(safe_error("NETWORK_DISCONNECTED"), "NETWORK_DISCONNECTED");
    }
    #[tokio::test]
    async fn destroy_cancels_connect_and_joins_owner_without_sdk_callbacks() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let n = Network::new(Instant::now(), 1, true).unwrap();
        n.enqueue(
            Intent::Connect {
                url,
                name: "test".into(),
                allow_lan: false,
            },
            1,
            None,
        )
        .unwrap();
        // TCP is accepted, but this fixture never answers the WS handshake.
        let (mut peer, _) = tokio::time::timeout(Duration::from_secs(3), listener.accept())
            .await
            .unwrap()
            .unwrap();
        let now = Instant::now();
        drop(n);
        assert!(now.elapsed() < Duration::from_secs(3));
        let mut bytes = Vec::new();
        use tokio::io::AsyncReadExt;
        tokio::time::timeout(Duration::from_secs(1), peer.read_to_end(&mut bytes))
            .await
            .unwrap()
            .unwrap();
    }
    async fn done(n: &Network, action: &str) -> Value {
        tokio::time::timeout(Duration::from_secs(12), async {
            loop {
                let s = n.status();
                if s["last_action"] == action && s["busy"] == false {
                    return s;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap()
    }
    #[tokio::test]
    async fn shared_mobile_application_uses_real_websocket_authority_ready_snapshot_and_resume() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(cine_server::Server::default().serve(listener, async {
            let _ = stopped.await;
        }));
        let mut host = Client::connect(&url, "host").await.unwrap();
        let invitation = host.create().await.unwrap();
        let joined_sequence = host.state().unwrap().sequence + 1;
        let n = Network::new(Instant::now(), 1, true).unwrap();
        n.enqueue(
            Intent::Connect {
                url,
                name: "mobile-proxy-fixture".into(),
                allow_lan: false,
            },
            1,
            None,
        )
        .unwrap();
        assert!(done(&n, "connect").await["error"].is_null());
        n.enqueue(
            Intent::Join {
                room_id: Uuid::parse_str(invitation["room_id"].as_str().unwrap()).unwrap(),
                room_epoch: Uuid::parse_str(invitation["room_epoch"].as_str().unwrap()).unwrap(),
                invite_token: invitation["invite_token"].as_str().unwrap().into(),
            },
            1,
            None,
        )
        .unwrap();
        assert!(done(&n, "join").await["error"].is_null());
        // ACK on the participant socket does not deliver MEMBER_JOINED to the
        // host synchronously. Observe the authoritative mutation before control.
        host.wait_state(joined_sequence).await.unwrap();
        assert_eq!(host.state().unwrap().members.len(), 2);
        host.media_demo().await.unwrap();
        let descriptor = host.state().unwrap().media.unwrap().descriptor;
        n.enqueue(Intent::Attach, 1, Some(descriptor.clone()))
            .unwrap();
        assert!(done(&n, "attach").await["error"].is_null());
        // No SDK observations: Ready is blocked locally.
        n.enqueue(Intent::Ready, 1, None).unwrap();
        assert_eq!(done(&n, "ready").await["error"], "MEDIA_NOT_READY");
        let sample = || PlayerView {
            ready: true,
            duration_ms: 300000,
            sampled_at_ms: n.player.boot.elapsed().as_millis() as u64,
            rate: 1.0,
            ..Default::default()
        };
        let sdk_proxy = n.player.clone();
        let sdk_samples = tokio::spawn(async move {
            // Ready must wait for the first fresh observation, not accept a load dispatch.
            tokio::time::sleep(Duration::from_millis(120)).await;
            loop {
                if !sdk_proxy.shared.lock().unwrap().active {
                    break;
                }
                sdk_proxy.sample(
                    PlayerView {
                        ready: true,
                        duration_ms: 300000,
                        sampled_at_ms: sdk_proxy.boot.elapsed().as_millis() as u64,
                        rate: 1.0,
                        ..Default::default()
                    },
                    true,
                );
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        });
        n.enqueue(Intent::Ready, 1, None).unwrap();
        assert!(done(&n, "ready").await["error"].is_null());
        host.ready().await.unwrap();
        tokio::time::sleep(Duration::from_millis(80)).await;
        let seq = host.state().unwrap().sequence;
        n.enqueue(Intent::Play, 1, None).unwrap();
        assert_eq!(done(&n, "play").await["error"], "NOT_AUTHORIZED");
        assert_eq!(host.state().unwrap().sequence, seq);
        n.reset(2);
        n.enqueue(Intent::Suspend, 2, None).unwrap();
        assert!(done(&n, "suspend").await["error"].is_null());
        n.enqueue(Intent::Foreground, 2, None).unwrap();
        assert!(done(&n, "foreground").await["error"].is_null());
        assert!(
            host.state()
                .unwrap()
                .members
                .iter()
                .any(|m| m.member_id != host.state().unwrap().host_id && !m.ready)
        );
        n.player.sample(sample(), true);
        n.enqueue(Intent::Attach, 2, Some(descriptor.clone()))
            .unwrap();
        done(&n, "attach").await;
        n.enqueue(Intent::Ready, 2, None).unwrap();
        assert!(done(&n, "ready").await["error"].is_null());
        n.enqueue(Intent::Disconnect, 2, None).unwrap();
        assert!(done(&n, "disconnect").await["error"].is_null());
        n.player.sample(sample(), true);
        n.enqueue(Intent::Reconnect, 2, None).unwrap();
        assert!(done(&n, "reconnect").await["error"].is_null());
        // Real protocol claim mismatch; no media transfer or Android mock runtime claims.
        let mut wrong = descriptor;
        wrong.identity.sha256[0] ^= 1;
        n.enqueue(Intent::Attach, 2, Some(wrong)).unwrap();
        done(&n, "attach").await;
        n.enqueue(Intent::Ready, 2, None).unwrap();
        assert_eq!(done(&n, "ready").await["error"], "MEDIA_MISMATCH");
        drop(n);
        sdk_samples.await.unwrap();
        host.disconnect().await;
        let _ = stop.send(());
        server.await.unwrap().unwrap();
    }
    #[tokio::test]
    async fn host_revalidation_preserves_selection_and_nonzero_timeline() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(cine_server::Server::default().serve(listener, async {
            let _ = stopped.await;
        }));
        let n = Network::new(Instant::now(), 1, true).unwrap();
        n.enqueue(
            Intent::Connect {
                url,
                name: "mobile-host".into(),
                allow_lan: false,
            },
            1,
            None,
        )
        .unwrap();
        assert!(done(&n, "connect").await["error"].is_null());
        n.enqueue(Intent::Create, 1, None).unwrap();
        assert!(done(&n, "create").await["error"].is_null());
        let descriptor = MediaDescriptor {
            media_id: Uuid::new_v4().to_string(),
            source_type: cine_core::media::SourceType::LocalFile,
            title: None,
            duration_ms: 300000,
            identity: cine_core::media::ContentIdentity {
                size_bytes: 1024,
                sha256: [0xaa; 32],
            },
            mime: Some("video/mp4".into()),
            codecs: vec![],
        };
        n.enqueue(Intent::Attach, 1, Some(descriptor.clone()))
            .unwrap();
        assert!(done(&n, "attach").await["error"].is_null());
        let sdk_proxy = n.player.clone();
        let sdk_samples = tokio::spawn(async move {
            loop {
                if !sdk_proxy.shared.lock().unwrap().active {
                    break;
                }
                sdk_proxy.sample(
                    PlayerView {
                        ready: true,
                        duration_ms: 300000,
                        sampled_at_ms: sdk_proxy.boot.elapsed().as_millis() as u64,
                        rate: 1.0,
                        ..Default::default()
                    },
                    true,
                );
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        });
        n.enqueue(Intent::Ready, 1, None).unwrap();
        assert!(done(&n, "ready").await["error"].is_null());
        n.enqueue(Intent::Seek { position_ms: 45000 }, 1, None)
            .unwrap();
        assert!(done(&n, "seek").await["error"].is_null());
        tokio::time::sleep(Duration::from_millis(1400)).await;
        let before = n.status()["presentation"]["room"].clone();
        let expected_timeline = if before["playback"]["pending"].is_object() {
            before["playback"]["pending"]["timeline_after"].clone()
        } else {
            before["playback"]["current"].clone()
        };
        assert_eq!(expected_timeline["position_ms"], 45000);
        // The SDK deliberately keeps reporting the temporary reload position 0.
        // Revalidation must not turn that observation into Host room authority.
        n.reset(2);
        n.enqueue(Intent::Suspend, 2, None).unwrap();
        assert!(done(&n, "suspend").await["error"].is_null());
        n.enqueue(Intent::Foreground, 2, None).unwrap();
        assert!(done(&n, "foreground").await["error"].is_null());
        n.enqueue(Intent::Revalidate, 2, Some(descriptor.clone()))
            .unwrap();
        assert!(done(&n, "revalidate").await["error"].is_null());
        n.enqueue(Intent::Ready, 2, None).unwrap();
        assert!(done(&n, "ready").await["error"].is_null());
        tokio::time::sleep(Duration::from_millis(150)).await;
        let after = n.status()["presentation"]["room"].clone();
        assert_eq!(after["media"], before["media"]);
        assert_eq!(after["playback"]["current"], expected_timeline);
        // Wrong content is still rejected before Ready; no identity shortcut.
        let mut wrong = descriptor.clone();
        wrong.identity.sha256[0] ^= 1;
        n.enqueue(Intent::Revalidate, 2, Some(wrong)).unwrap();
        assert_eq!(done(&n, "revalidate").await["error"], "MEDIA_MISMATCH");
        n.enqueue(Intent::Ready, 2, None).unwrap();
        assert_eq!(done(&n, "ready").await["error"], "MEDIA_NOT_READY");
        // An explicit user selection retains its original reset semantics.
        n.enqueue(Intent::Attach, 2, Some(descriptor)).unwrap();
        assert!(done(&n, "attach").await["error"].is_null());
        tokio::time::sleep(Duration::from_millis(150)).await;
        let selected = n.status()["presentation"]["room"].clone();
        assert_ne!(selected["media"], before["media"]);
        assert_eq!(selected["playback"]["current"]["position_ms"], 0);
        drop(n);
        sdk_samples.await.unwrap();
        let _ = stop.send(());
        server.await.unwrap().unwrap();
    }
}
