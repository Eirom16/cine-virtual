use cine_transfer::{storage::Partial, tls::Identity, transport, *};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Write, net::TcpListener, sync::atomic::Ordering, time::Duration};
use uuid::Uuid;
fn manifest(data: &[u8]) -> Manifest {
    Manifest {
        version: 1,
        transfer_id: Uuid::new_v4(),
        room_id: Uuid::new_v4(),
        room_epoch: Uuid::new_v4(),
        host_id: Uuid::new_v4(),
        authority_revision: 1,
        media_revision: 1,
        media_id: Uuid::new_v4(),
        display_name: "fixture".into(),
        size_bytes: data.len() as u64,
        chunk_size: CHUNK_SIZE,
        chunk_count: (data.len() as u64).div_ceil(CHUNK_SIZE as u64) as u32,
        sha256: Sha256::digest(data).into(),
        duration_ms: 1000,
        block_hash: "sha256".into(),
    }
}
fn root() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("cine-transfer-test-{}", Uuid::new_v4()));
    std::fs::create_dir(&p).unwrap();
    p
}
#[test]
fn manifest_and_bounds_are_strict() {
    let m = manifest(&[0; 13]);
    assert_eq!(m.chunk(0).unwrap(), (0, 13));
    assert_eq!(m.chunk(1), Err(Error::Bounds));
    for bad in ["../x", "x/y", "x\\y", "..", "", "x\n"] {
        let mut b = m.clone();
        b.display_name = bad.into();
        assert_eq!(b.validate(), Err(Error::Manifest));
    }
    let mut b = m.clone();
    b.chunk_size = 0;
    assert_eq!(b.validate(), Err(Error::Manifest));
    b = m.clone();
    b.size_bytes = MAX_SIZE + 1;
    assert_eq!(b.validate(), Err(Error::Manifest));
    b = m;
    b.chunk_count = u32::MAX;
    assert_eq!(b.validate(), Err(Error::Manifest));
}
#[test]
fn blocks_corrupt_truncated_offset_dedup_and_conflicting_duplicates() {
    let path = root();
    let data = b"authorized";
    let m = manifest(data);
    let mut p = Partial::create(&path, m).unwrap();
    let hash = Sha256::digest(data).into();
    assert_eq!(p.accept(0, 1, data, hash), Err(Error::Bounds));
    assert_eq!(p.accept(0, 0, &data[..3], hash), Err(Error::Bounds));
    assert_eq!(p.accept(9, 0, data, hash), Err(Error::Bounds));
    assert_eq!(p.accept(0, 0, data, [0; 32]), Err(Error::Corrupt));
    assert_eq!(p.verified, 0);
    assert!(p.accept(0, 0, data, hash).unwrap());
    assert!(!p.accept(0, 0, data, hash).unwrap());
    assert_eq!(p.verified, data.len() as u64);
    let other = b"badcontent";
    assert_eq!(
        p.accept(0, 0, other, Sha256::digest(other).into()),
        Err(Error::Corrupt)
    );
    p.discard().unwrap();
    std::fs::remove_dir(path).unwrap();
}
#[test]
fn out_of_order_resume_revalidates_disk_and_final_sha() {
    let path = root();
    let data = vec![7; CHUNK_SIZE as usize + 7];
    let m = manifest(&data);
    let id = m.transfer_id;
    let mut p = Partial::create(&path, m).unwrap();
    p.accept(
        1,
        CHUNK_SIZE as u64,
        &data[CHUNK_SIZE as usize..],
        Sha256::digest(&data[CHUNK_SIZE as usize..]).into(),
    )
    .unwrap();
    assert_eq!(p.missing().collect::<Vec<_>>(), vec![0]);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(path.join(format!("cine-{id}/media.part")))
        .unwrap();
    file.write_all(b"corrupt prefix").unwrap();
    p.revalidate(|| Ok(())).unwrap();
    assert_eq!(p.verified, 7);
    p.accept(
        0,
        0,
        &data[..CHUNK_SIZE as usize],
        Sha256::digest(&data[..CHUNK_SIZE as usize]).into(),
    )
    .unwrap();
    let final_path = p.finish(|| Ok(())).unwrap();
    assert_eq!(std::fs::read(final_path).unwrap(), data);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn corrupted_checkpoint_is_missing_again_and_wrong_final_hash_never_commits() {
    let path = root();
    let data = b"fixture";
    let mut m = manifest(data);
    m.sha256 = [0; 32];
    let id = m.transfer_id;
    let mut p = Partial::create(&path, m).unwrap();
    p.accept(0, 0, data, Sha256::digest(data).into()).unwrap();
    assert_eq!(p.finish(|| Ok(())), Err(Error::Identity));
    assert!(!path.join(format!("cine-{id}/media.mp4")).exists());
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(path.join(format!("cine-{id}/media.part")))
        .unwrap();
    file.write_all(b"changed").unwrap();
    p.revalidate(|| Ok(())).unwrap();
    assert_eq!(p.verified, 0);
    p.discard().unwrap();
    std::fs::remove_dir(path).unwrap();
}
#[test]
fn authorization_room_epoch_receiver_revision_secret_expiry_replay_and_revocation() {
    let m = manifest(b"test");
    let c = Credential::new(&m, Uuid::new_v4()).unwrap();
    let auth = Authorization::new(c.clone(), Duration::from_secs(30));
    for field in 0..6 {
        let mut bad = c.clone();
        match field {
            0 => bad.room_id = Uuid::new_v4(),
            1 => bad.room_epoch = Uuid::new_v4(),
            2 => bad.receiver_id = Uuid::new_v4(),
            3 => bad.media_revision += 1,
            4 => bad.authority_revision += 1,
            _ => bad.secret = [0; 32],
        }
        assert_eq!(auth.consume(&bad), Err(Error::Unauthorized));
    }
    auth.consume(&c).unwrap();
    assert_eq!(auth.consume(&c), Err(Error::Replay));
    auth.revoked.store(true, Ordering::Release);
    assert_eq!(auth.check(), Err(Error::Unauthorized));
    let expired = Authorization::new(c.clone(), Duration::ZERO);
    assert_eq!(expired.consume(&c), Err(Error::Expired));
}
#[test]
fn pause_cancel_lifecycle_and_no_overwrite() {
    let c = Control::default();
    c.pause.store(true, Ordering::Release);
    assert_eq!(c.check(), Err(Error::Paused));
    c.cancel.store(true, Ordering::Release);
    assert_eq!(c.check(), Err(Error::Cancelled));
    let path = root();
    let m = manifest(b"safe");
    let id = m.transfer_id;
    let mut p = Partial::create(&path, m).unwrap();
    p.accept(0, 0, b"safe", Sha256::digest(b"safe").into())
        .unwrap();
    std::fs::write(path.join(format!("cine-{id}/media.mp4")), b"existing").unwrap();
    assert_eq!(p.finish(|| Ok(())), Err(Error::Storage));
    assert_eq!(
        std::fs::read(path.join(format!("cine-{id}/media.mp4"))).unwrap(),
        b"existing"
    );
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn real_tls_stream_pause_resume_new_credential_and_constant_memory_window() {
    let path = root();
    let data = vec![19; CHUNK_SIZE as usize * 3 + 31];
    let m = manifest(&data);
    let source = path.join("source");
    std::fs::write(&source, &data).unwrap();
    let tls = Identity::generate().unwrap();
    let mut partial = Partial::create(&path, m.clone()).unwrap();
    let ctl = Control::default();
    for attempt in 0..2 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let credential = Credential::new(&m, Uuid::new_v4()).unwrap();
        let auth = Authorization::new(credential.clone(), Duration::from_secs(10));
        let config = tls.server.clone();
        let source = source.clone();
        let manifest = m.clone();
        let sender = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            transport::send(
                stream,
                config,
                &manifest,
                &mut File::open(source).unwrap(),
                &auth,
                &Control::default(),
                |_| {},
            )
        });
        let result = transport::receive(
            address,
            &tls.certificate,
            &credential,
            &mut partial,
            &ctl,
            |p| {
                if attempt == 0 && p.verified_bytes >= CHUNK_SIZE as u64 {
                    ctl.pause.store(true, Ordering::Release)
                }
            },
        );
        if attempt == 0 {
            assert_eq!(result, Err(Error::Paused));
            assert_eq!(partial.verified, CHUNK_SIZE as u64);
            assert!(sender.join().unwrap().is_err());
            ctl.pause.store(false, Ordering::Release);
        } else {
            let final_path = result.unwrap();
            assert_eq!(std::fs::read(final_path).unwrap(), data);
            sender.join().unwrap().unwrap();
        }
    }
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn untrusted_tls_pin_and_wrong_peer_credential_never_write_a_chunk() {
    for wrong_pin in [true, false] {
        let path = root();
        let data = b"private fixture";
        let m = manifest(data);
        let source = path.join("source");
        std::fs::write(&source, data).unwrap();
        let tls = Identity::generate().unwrap();
        let expected = Credential::new(&m, Uuid::new_v4()).unwrap();
        let mut candidate = expected.clone();
        if !wrong_pin {
            candidate.secret = [0; 32];
        }
        let pin = if wrong_pin {
            Identity::generate().unwrap().certificate
        } else {
            tls.certificate
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let manifest = m.clone();
        let sender = std::thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            transport::send(
                socket,
                tls.server,
                &manifest,
                &mut File::open(source).unwrap(),
                &Authorization::new(expected, Duration::from_secs(10)),
                &Control::default(),
                |_| {},
            )
        });
        let mut partial = Partial::create(&path, m).unwrap();
        assert!(
            transport::receive(
                address,
                &pin,
                &candidate,
                &mut partial,
                &Control::default(),
                |_| {}
            )
            .is_err()
        );
        assert_eq!(partial.verified, 0);
        assert!(sender.join().unwrap().is_err());
        partial.discard().unwrap();
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn real_tls_malicious_chunk_header_and_payload_are_rejected_without_allocation_growth() {
    use std::io::Read;
    for fault in 0..4 {
        let path = root();
        let data = b"good";
        let m = manifest(data);
        let credential = Credential::new(&m, Uuid::new_v4()).unwrap();
        let tls = Identity::generate().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let sender = std::thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut stream = rustls::StreamOwned::new(
                rustls::ServerConnection::new(tls.server).unwrap(),
                socket,
            );
            let mut prefix = [0; 4];
            stream.read_exact(&mut prefix).unwrap();
            let len = u32::from_be_bytes(prefix) as usize;
            assert!(len <= MAX_CONTROL);
            stream.read_exact(&mut vec![0; len]).unwrap();
            stream.write_all(b"CVP1").unwrap();
            stream.flush().unwrap();
            stream.read_exact(&mut prefix).unwrap();
            let index: u32 = if fault == 0 { u32::MAX - 1 } else { 0 };
            let offset: u64 = if fault == 1 { u64::MAX } else { 0 };
            let length: u32 = if fault == 2 { u32::MAX } else { 4 };
            stream.write_all(&index.to_be_bytes()).unwrap();
            stream.write_all(&offset.to_be_bytes()).unwrap();
            stream.write_all(&length.to_be_bytes()).unwrap();
            stream.write_all(&[0; 32]).unwrap();
            stream.write_all(b"evil").unwrap();
            stream.flush().unwrap();
        });
        let mut partial = Partial::create(&path, m).unwrap();
        let result = transport::receive(
            address,
            &tls.certificate,
            &credential,
            &mut partial,
            &Control::default(),
            |_| {},
        );
        assert_eq!(
            result,
            Err(if fault == 3 {
                Error::Corrupt
            } else {
                Error::Bounds
            })
        );
        assert_eq!(partial.verified, 0);
        sender.join().unwrap();
        partial.discard().unwrap();
        std::fs::remove_dir(path).unwrap();
    }
}
