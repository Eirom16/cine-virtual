//! Local opt-in transport evidence; private grants stay in memory, no product claim.
use cine_transfer::{relay, storage::Partial, tls::Identity, transport, *};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    net::{SocketAddr, TcpListener, TcpStream},
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

struct Workspace(std::path::PathBuf);
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[cfg(target_os = "linux")]
fn enter(path: &Option<String>) -> Result<(), Error> {
    use std::os::fd::AsRawFd;
    if let Some(path) = path {
        let file = File::open(path)?;
        // Only a newly spawned lab worker changes its network namespace.
        if unsafe { libc::setns(file.as_raw_fd(), libc::CLONE_NEWNET) } != 0 {
            return Err(Error::Io);
        }
    }
    Ok(())
}
#[cfg(not(target_os = "linux"))]
fn enter(path: &Option<String>) -> Result<(), Error> {
    if path.is_some() {
        Err(Error::Io)
    } else {
        Ok(())
    }
}
#[cfg(any(target_os = "linux", target_os = "android"))]
fn process_resources() -> serde_json::Value {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // RUSAGE_SELF counts this entire lab process, not an isolated peer/carrier.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return json!({"status":"NOT MEASURED"});
    }
    let usage = unsafe { usage.assume_init() };
    let seconds = |t: libc::timeval| t.tv_sec as f64 + t.tv_usec as f64 / 1_000_000.0;
    json!({"cpu_seconds":seconds(usage.ru_utime)+seconds(usage.ru_stime),"peak_rss_kib":usage.ru_maxrss,"fds_after_workers":std::fs::read_dir("/proc/self/fd").ok().map(|r|r.count()),"threads_after_workers":std::fs::read_dir("/proc/self/task").ok().map(|r|r.count()),"scope":"entire lab process, fixture creation + sender + receiver + relay; not marginal carrier cost"})
}
#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn process_resources() -> serde_json::Value {
    json!({"status":"NOT MEASURED"})
}
fn run() -> Result<(), Error> {
    let disconnect = std::env::var_os("CINE_WAN_LAB_DISCONNECT").is_some();
    let args: Vec<_> = std::env::args().skip(1).collect();
    let (size, bind, sender_ns, receiver_ns, direct): (
        usize,
        SocketAddr,
        Option<String>,
        Option<String>,
        Option<SocketAddr>,
    ) = match args.as_slice() {
        [] => (
            8 * 1024 * 1024,
            "127.0.0.1:0".parse().unwrap(),
            None,
            None,
            None,
        ),
        [size] => (
            size.parse().map_err(|_| Error::Bounds)?,
            "127.0.0.1:0".parse().unwrap(),
            None,
            None,
            None,
        ),
        [size, bind, a, b, direct] => (
            size.parse().map_err(|_| Error::Bounds)?,
            bind.parse().map_err(|_| Error::Bounds)?,
            Some(a.clone()),
            Some(b.clone()),
            Some(direct.parse().map_err(|_| Error::Bounds)?),
        ),
        _ => return Err(Error::Bounds),
    };
    if !(8192..=32 * 1024 * 1024).contains(&size) {
        return Err(Error::Bounds);
    }
    let root = std::env::temp_dir().join(format!("cine-wan-lab-{}", Uuid::new_v4()));
    std::fs::create_dir(&root)?;
    let _workspace = Workspace(root.clone());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))?;
    }
    let source = root.join("synthetic");
    let data: Vec<_> = (0..size).map(|i| (i % 251) as u8).collect();
    std::fs::write(&source, &data)?;
    let manifest = Manifest {
        version: 1,
        transfer_id: Uuid::new_v4(),
        room_id: Uuid::new_v4(),
        room_epoch: Uuid::new_v4(),
        host_id: Uuid::new_v4(),
        authority_revision: 1,
        media_revision: 1,
        media_id: Uuid::new_v4(),
        display_name: "Synthetic fixture".into(),
        size_bytes: size as u64,
        chunk_size: CHUNK_SIZE,
        chunk_count: (size as u64).div_ceil(CHUNK_SIZE as u64) as u32,
        sha256: Sha256::digest(&data).into(),
        duration_ms: 1000,
        block_hash: "sha256".into(),
    };
    drop(data);
    let receiver_id = Uuid::new_v4();
    let host_tls = Identity::generate()?;
    let ctl = Control::default();
    let mut partial = Partial::create(&root, manifest.clone())?;
    // For netns tests, a real private listener exists on the sender. Its NAT
    // admits only established traffic; the advertised gateway has no forwarding.
    let direct_listener = if direct.is_some() {
        if sender_ns.is_none() || receiver_ns.is_none() {
            return Err(Error::Bounds);
        }
        let sender_net = sender_ns.clone();
        Some(
            thread::spawn(move || -> Result<TcpListener, Error> {
                enter(&sender_net)?;
                Ok(TcpListener::bind("0.0.0.0:1730")?)
            })
            .join()
            .map_err(|_| Error::Io)??,
        )
    } else {
        None
    };
    let direct_probe = if let Some(address) = direct {
        let ns = receiver_ns.clone();
        thread::spawn(move || -> Result<_, Error> {
            enter(&ns)?;
            let start = Instant::now();
            let result = TcpStream::connect_timeout(&address, Duration::from_secs(5));
            Ok(json!({"status":if result.is_err(){"EXPECTED_FAILURE"}else{"UNEXPECTED_SUCCESS"},"attempts":1,"duration_ms":start.elapsed().as_secs_f64()*1000.0,"error_kind":result.err().map(|e| format!("{:?}",e.kind()))}))
        }).join().map_err(|_| Error::Io)??
    } else {
        json!({"status":"NOT TESTED"})
    };
    drop(direct_listener);
    if direct_probe["status"] == "UNEXPECTED_SUCCESS" {
        return Err(Error::Unauthorized);
    }
    let attempts = if size > CHUNK_SIZE as usize { 2 } else { 1 };
    let mut observations = Vec::new();
    let mut interrupted_at: Option<Instant> = None;
    let mut grant_ids = Vec::new();
    for attempt in 0..attempts {
        let relay_tls = Identity::generate_names(vec!["cine-relay.local".into()])?;
        let listener = TcpListener::bind(bind)?;
        let address = listener.local_addr()?;
        let a = Credential::new(&manifest, receiver_id)?;
        let b = Credential::new(&manifest, receiver_id)?;
        let peer_grant = Credential::new(&manifest, receiver_id)?;
        grant_ids.push(peer_grant.grant_id);
        let auth_a = Authorization::new(a.clone(), Duration::from_secs(120));
        let auth_b = Authorization::new(b.clone(), Duration::from_secs(120));
        let relay_config = relay_tls.server;
        let relay_worker = thread::spawn(move || {
            relay::serve_pair(
                listener,
                relay_config,
                &auth_a,
                &auth_b,
                &Control::default(),
                relay::Limits::default(),
            )
        });
        let ns = sender_ns.clone();
        let pin = relay_tls.certificate.clone();
        let m = manifest.clone();
        let file = source.clone();
        let config = host_tls.server.clone();
        let grant = peer_grant.clone();
        let sender = thread::spawn(move || -> Result<(), Error> {
            enter(&ns)?;
            let ctl = Control::default();
            let pipe = relay::connect(address, &pin, &a, &ctl, |_| {})?;
            transport::send(
                pipe,
                config,
                &m,
                &mut File::open(file)?,
                &Authorization::new(grant, Duration::from_secs(120)),
                &ctl,
                |_| {},
            )
        });
        let ns = receiver_ns.clone();
        let pin = relay_tls.certificate;
        let peer_pin = host_tls.certificate.clone();
        let ctl_receiver = ctl.clone();
        let receiver = thread::spawn(move || -> Result<_, Error> {
            enter(&ns)?;
            let start = Instant::now();
            let mut active = None;
            let pipe = relay::connect(address, &pin, &b, &ctl_receiver, |s| {
                active = s.try_clone().ok()
            })?;
            let active = active.ok_or(Error::Io)?;
            let outer_ms = start.elapsed().as_secs_f64() * 1000.0;
            let initial = partial.verified;
            let mut auth_ms = None;
            let mut first_ms = None;
            let mut received = 0;
            let result = transport::receive_on_observed(
                pipe,
                &peer_pin,
                &peer_grant,
                &mut partial,
                &ctl_receiver,
                |d| auth_ms = Some(d.as_secs_f64() * 1000.0),
                |p| {
                    if p.received_bytes > 0 && first_ms.is_none() {
                        first_ms = Some(start.elapsed().as_secs_f64() * 1000.0);
                    }
                    received = p.received_bytes;
                    if attempt == 0 && attempts == 2 && p.verified_bytes >= u64::from(CHUNK_SIZE) {
                        if disconnect {
                            let _ = active.shutdown(std::net::Shutdown::Both);
                        } else {
                            ctl_receiver.pause.store(true, Ordering::Release);
                        }
                    }
                },
            );
            let paused = attempt == 0 && attempts == 2;
            if paused && result != Err(if disconnect { Error::Io } else { Error::Paused }) {
                return Err(Error::Io);
            }
            if !paused {
                result?;
            }
            let elapsed = start.elapsed().as_secs_f64();
            let metrics = json!({"attempt":attempt+1,"route":"RELAYED","status":if paused{if disconnect{"DISCONNECTED"}else{"PAUSED"}}else{"COMPLETE_SHA_VERIFIED"},"outer_tls_admission_pair_ms":outer_ms,"inner_tls_grant_auth_ms":auth_ms,"first_chunk_ms":first_ms,"elapsed_seconds":elapsed,"new_useful_bytes":received,"initial_verified_bytes":initial,"verified_bytes":partial.verified,"effective_mib_s_including_admission_auth_sha":received as f64/1048576.0/elapsed});
            Ok((partial, metrics))
        });
        let receiver_result = receiver.join().map_err(|_| Error::Io)?;
        let sender_result = sender.join().map_err(|_| Error::Io)?;
        let stats = relay_worker.join().map_err(|_| Error::Io)?;
        let (returned, mut metrics) = receiver_result?;
        partial = returned;
        if attempt == 0 && attempts == 2 {
            if sender_result.is_ok() {
                return Err(Error::Io);
            }
            ctl.pause.store(false, Ordering::Release);
        } else {
            sender_result?;
        }
        if attempt == 0 && attempts == 2 {
            interrupted_at = Some(Instant::now());
        } else if let Some(interrupted) = interrupted_at {
            metrics["recovery_to_complete_ms"] =
                json!(interrupted.elapsed().as_secs_f64() * 1000.0);
        }
        metrics["relay"] = match stats {
            Ok(stats) => serde_json::to_value(stats).map_err(|_| Error::Io)?,
            Err(Error::Io) if attempt == 0 && disconnect => {
                json!({"status":"ENDED_ON_DISCONNECTION"})
            }
            Err(e) => return Err(e),
        };
        observations.push(metrics);
    }
    let fresh = grant_ids.len() == 1 || grant_ids[0] != grant_ids[1];
    println!(
        "{}",
        json!({"schema_version":1,"resources":process_resources(),"status":"LOCAL_SPIKE_PASS","real_wan":"NOT TESTED","product_wan":"NOT IMPLEMENTED","network":if sender_ns.is_some(){"isolated namespaces, two NATs"}else{"loopback"},"carrier":"TCP outer TLS1.3 / inner peer TLS1.3","bytes":size,"sha_final_match":partial.completed(),"new_grant_after_pause":fresh,"direct_probe":direct_probe,"runs":observations,"gathering_ms":null,"ice_checks_ms":null,"retries":attempts-1,"resume_policy":"explicit lab reauthorization; not automatic product fallback"})
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{{\"status\":\"FAIL\",\"code\":\"{e}\"}}");
        std::process::exit(1);
    }
}
