use cine_transfer::{relay, storage::Partial, tls::Identity, transport, *};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Write, net::TcpListener, sync::atomic::Ordering, thread, time::Duration};
use uuid::Uuid;

struct Fixture {
    root: std::path::PathBuf,
    manifest: Manifest,
    source: std::path::PathBuf,
}
impl Fixture {
    fn new() -> Self {
        Self::sized(CHUNK_SIZE as usize * 3 + 31)
    }
    fn sized(size: usize) -> Self {
        let root = std::env::temp_dir().join(format!("cine-relay-test-{}", Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let data = vec![37; size];
        let source = root.join("source");
        std::fs::write(&source, &data).unwrap();
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
            size_bytes: data.len() as u64,
            chunk_size: CHUNK_SIZE,
            chunk_count: (size as u64).div_ceil(CHUNK_SIZE as u64) as u32,
            sha256: Sha256::digest(&data).into(),
            duration_ms: 1000,
            block_hash: "sha256".into(),
        };
        Self {
            root,
            manifest,
            source,
        }
    }
    fn ticket(&self) -> Credential {
        Credential::new(&self.manifest, Uuid::new_v4()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn two_outbound_peers_keep_inner_tls_chunks_sha_and_resume_with_new_grants() {
    let f = Fixture::new();
    let host_tls = Identity::generate().unwrap();
    let ctl = Control::default();
    let mut partial = Partial::create(&f.root, f.manifest.clone()).unwrap();
    let mut grants = Vec::new();
    for attempt in 0..2 {
        let relay_tls = Identity::generate_names(vec!["cine-relay.local".into()]).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let a = f.ticket();
        let b = f.ticket();
        let peer_grant = f.ticket();
        assert_ne!(a.secret, peer_grant.secret);
        assert_ne!(b.secret, peer_grant.secret);
        grants.push(peer_grant.grant_id);
        let auth_a = Authorization::new(a.clone(), Duration::from_secs(20));
        let auth_b = Authorization::new(b.clone(), Duration::from_secs(20));
        let relay_config = relay_tls.server;
        let relay = thread::spawn(move || {
            relay::serve_pair(
                listener,
                relay_config,
                &auth_a,
                &auth_b,
                &Control::default(),
                relay::Limits::default(),
            )
        });
        let source = f.source.clone();
        let m = f.manifest.clone();
        let pin = relay_tls.certificate.clone();
        let config = host_tls.server.clone();
        let grant = peer_grant.clone();
        let sender = thread::spawn(move || {
            let ctl = Control::default();
            let pipe = relay::connect(address, &pin, &a, &ctl, |_| {}).unwrap();
            transport::send(
                pipe,
                config,
                &m,
                &mut File::open(source).unwrap(),
                &Authorization::new(grant, Duration::from_secs(20)),
                &ctl,
                |_| {},
            )
        });
        let pipe = relay::connect(address, &relay_tls.certificate, &b, &ctl, |_| {}).unwrap();
        let initial = partial.verified;
        let mut received = 0;
        let result = transport::receive_on(
            pipe,
            &host_tls.certificate,
            &peer_grant,
            &mut partial,
            &ctl,
            |p| {
                received = p.received_bytes;
                if attempt == 0 && p.verified_bytes >= u64::from(CHUNK_SIZE) {
                    ctl.pause.store(true, Ordering::Release);
                }
            },
        );
        if attempt == 0 {
            assert_eq!(result, Err(Error::Paused));
            assert_eq!(partial.verified, u64::from(CHUNK_SIZE));
            assert!(sender.join().unwrap().is_err());
            ctl.pause.store(false, Ordering::Release);
        } else {
            let path = result.unwrap();
            assert_eq!(received, f.manifest.size_bytes - initial);
            assert_eq!(
                std::fs::read(path).unwrap(),
                std::fs::read(&f.source).unwrap()
            );
            sender.join().unwrap().unwrap();
        }
        let stats = relay.join().unwrap().unwrap();
        assert!(stats.forwarded_ciphertext_bytes > received);
    }
    assert_ne!(grants[0], grants[1]);
}

#[test]
fn outer_pin_wrong_ticket_scope_replay_and_oversized_admission_fail_closed() {
    for fault in 0..5 {
        let f = Fixture::new();
        let tls = Identity::generate_names(vec!["cine-relay.local".into()]).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let a = f.ticket();
        let b = f.ticket();
        let auth_a = Authorization::new(a.clone(), Duration::from_secs(10));
        let auth_b = Authorization::new(b.clone(), Duration::from_secs(10));
        if fault == 3 {
            auth_a.consume(&a).unwrap();
        }
        let mut candidate = a;
        if fault == 1 {
            candidate.secret = [0; 32];
        }
        if fault == 2 {
            candidate.room_epoch = Uuid::new_v4();
        }
        let pin = if fault == 0 {
            Identity::generate().unwrap().certificate
        } else {
            tls.certificate
        };
        let server = thread::spawn(move || {
            relay::serve_pair(
                listener,
                tls.server,
                &auth_a,
                &auth_b,
                &Control::default(),
                relay::Limits::default(),
            )
        });
        if fault == 4 {
            use cine_transfer::carrier::Carrier;
            let sock = std::net::TcpStream::connect(addr).unwrap();
            sock.configure(Duration::from_secs(2)).unwrap();
            let mut outer = rustls::StreamOwned::new(
                rustls::ClientConnection::new(
                    cine_transfer::tls::client(&pin).unwrap(),
                    "cine-relay.local".try_into().unwrap(),
                )
                .unwrap(),
                sock,
            );
            outer
                .write_all(&((MAX_CONTROL + 1) as u32).to_be_bytes())
                .unwrap();
            outer.flush().unwrap();
        } else {
            assert!(relay::connect(addr, &pin, &candidate, &Control::default(), |_| {}).is_err());
        }
        assert!(server.join().unwrap().is_err());
        assert!(
            !f.root
                .join(format!("cine-{}/media.part", f.manifest.transfer_id))
                .exists()
        );
    }
}

#[test]
fn relay_cannot_substitute_peer_identity_and_never_receives_inner_secret() {
    let f = Fixture::new();
    let relay_tls = Identity::generate_names(vec!["cine-relay.local".into()]).unwrap();
    let expected_host = Identity::generate().unwrap();
    let impostor = Identity::generate().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let a = f.ticket();
    let b = f.ticket();
    let grant = f.ticket();
    let auth_a = Authorization::new(a.clone(), Duration::from_secs(10));
    let auth_b = Authorization::new(b.clone(), Duration::from_secs(10));
    let server = thread::spawn(move || {
        relay::serve_pair(
            listener,
            relay_tls.server,
            &auth_a,
            &auth_b,
            &Control::default(),
            relay::Limits::default(),
        )
    });
    let pin = relay_tls.certificate.clone();
    let source = f.source.clone();
    let m = f.manifest.clone();
    let g = grant.clone();
    let sender = thread::spawn(move || {
        let ctl = Control::default();
        let pipe = relay::connect(addr, &pin, &a, &ctl, |_| {}).unwrap();
        transport::send(
            pipe,
            impostor.server,
            &m,
            &mut File::open(source).unwrap(),
            &Authorization::new(g, Duration::from_secs(10)),
            &ctl,
            |_| {},
        )
    });
    let ctl = Control::default();
    let pipe = relay::connect(addr, &relay_tls.certificate, &b, &ctl, |_| {}).unwrap();
    let mut partial = Partial::create(&f.root, f.manifest.clone()).unwrap();
    assert!(
        transport::receive_on(
            pipe,
            &expected_host.certificate,
            &grant,
            &mut partial,
            &ctl,
            |_| {}
        )
        .is_err()
    );
    assert_eq!(partial.verified, 0);
    assert!(sender.join().unwrap().is_err());
    let _ = server.join().unwrap();
}

#[test]
fn revoked_expired_cancelled_and_invalid_resource_policy_never_accept() {
    let f = Fixture::new();
    for fault in 0..4 {
        let tls = Identity::generate().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let auth_a = Authorization::new(
            f.ticket(),
            if fault == 1 {
                Duration::ZERO
            } else {
                Duration::from_secs(10)
            },
        );
        let auth_b = Authorization::new(f.ticket(), Duration::from_secs(10));
        let ctl = Control::default();
        let mut limits = relay::Limits::default();
        if fault == 0 {
            auth_a.revoked.store(true, Ordering::Release);
        }
        if fault == 2 {
            ctl.cancel.store(true, Ordering::Release);
        }
        if fault == 3 {
            limits.bytes = 0;
        }
        let expected = [
            Error::Unauthorized,
            Error::Expired,
            Error::Cancelled,
            Error::Bounds,
        ][fault];
        assert!(
            matches!(relay::serve_pair(listener, tls.server, &auth_a, &auth_b, &ctl, limits), Err(e) if e == expected)
        );
    }
}

#[test]
fn relay_byte_budget_is_enforced_before_forwarding() {
    let f = Fixture::new();
    let tls = Identity::generate_names(vec!["cine-relay.local".into()]).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let a = f.ticket();
    let b = f.ticket();
    let auth_a = Authorization::new(a.clone(), Duration::from_secs(10));
    let auth_b = Authorization::new(b.clone(), Duration::from_secs(10));
    let server = thread::spawn(move || {
        relay::serve_pair(
            listener,
            tls.server,
            &auth_a,
            &auth_b,
            &Control::default(),
            relay::Limits {
                bytes: 1,
                ..relay::Limits::default()
            },
        )
    });
    let pin = tls.certificate.clone();
    let sender = thread::spawn(move || {
        let mut pipe = relay::connect(addr, &pin, &a, &Control::default(), |_| {}).unwrap();
        pipe.write_all(b"two").unwrap();
        pipe.flush().unwrap();
        thread::sleep(Duration::from_millis(20));
    });
    let _peer = relay::connect(addr, &tls.certificate, &b, &Control::default(), |_| {}).unwrap();
    assert!(matches!(server.join().unwrap(), Err(Error::Bounds)));
    sender.join().unwrap();
}

#[test]
fn cancellation_during_outer_handshake_joins_the_relay() {
    let f = Fixture::new();
    let tls = Identity::generate_names(vec!["cine-relay.local".into()]).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let a = f.ticket();
    let b = f.ticket();
    let auth_a = Authorization::new(a.clone(), Duration::from_secs(10));
    let auth_b = Authorization::new(b, Duration::from_secs(10));
    let ctl = Control::default();
    let server = thread::spawn(move || {
        relay::serve_pair(
            listener,
            tls.server,
            &auth_a,
            &auth_b,
            &Control::default(),
            relay::Limits::default(),
        )
    });
    assert!(
        relay::connect(addr, &tls.certificate, &a, &ctl, |_| {
            ctl.cancel.store(true, Ordering::Release);
        })
        .is_err()
    );
    assert!(server.join().unwrap().is_err());
}

#[test]
fn small_relay_transfer_drains_tls_termination_on_ipv4_and_ipv6() {
    for bind in ["127.0.0.1:0", "[::1]:0"] {
        let f = Fixture::sized(8192);
        let tls = Identity::generate_names(vec!["cine-relay.local".into()]).unwrap();
        let host = Identity::generate().unwrap();
        let listener = TcpListener::bind(bind).unwrap();
        let addr = listener.local_addr().unwrap();
        let a = f.ticket();
        let b = f.ticket();
        let g = f.ticket();
        let auth_a = Authorization::new(a.clone(), Duration::from_secs(10));
        let auth_b = Authorization::new(b.clone(), Duration::from_secs(10));
        let server = thread::spawn(move || {
            relay::serve_pair(
                listener,
                tls.server,
                &auth_a,
                &auth_b,
                &Control::default(),
                relay::Limits::default(),
            )
        });
        let pin = tls.certificate.clone();
        let source = f.source.clone();
        let m = f.manifest.clone();
        let grant = g.clone();
        let sender = thread::spawn(move || {
            let ctl = Control::default();
            let pipe = relay::connect(addr, &pin, &a, &ctl, |_| {}).unwrap();
            transport::send(
                pipe,
                host.server,
                &m,
                &mut File::open(source).unwrap(),
                &Authorization::new(grant, Duration::from_secs(10)),
                &ctl,
                |_| {},
            )
        });
        let ctl = Control::default();
        let pipe = relay::connect(addr, &tls.certificate, &b, &ctl, |_| {}).unwrap();
        let mut partial = Partial::create(&f.root, f.manifest.clone()).unwrap();
        transport::receive_on(pipe, &host.certificate, &g, &mut partial, &ctl, |_| {}).unwrap();
        assert!(partial.completed());
        sender.join().unwrap().unwrap();
        server.join().unwrap().unwrap();
    }
}

#[test]
fn stale_allocation_ticket_does_not_authorize_a_new_generation() {
    let f = Fixture::new();
    let old = f.ticket();
    let new = f.ticket();
    let auth = Authorization::new(new.clone(), Duration::from_secs(10));
    assert_eq!(auth.consume(&old), Err(Error::Unauthorized));
    auth.consume(&new).unwrap();
    assert_eq!(auth.consume(&new), Err(Error::Replay));
}
