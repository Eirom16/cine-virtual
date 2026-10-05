use cine_local_media::{CHUNK_BYTES, LocalHandle, LocalMedia, MediaError, parse_probe};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(bytes: &[u8]) -> Self {
        let path = std::env::temp_dir().join(format!(
            "cine-media-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, bytes).unwrap();
        Self(path)
    }
    fn hash(&self) -> cine_core::media::ContentIdentity {
        LocalHandle::open(&self.0).unwrap().hash(|_| true).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
#[test]
fn known_sha256_vector_and_exact_identity() {
    let a = Fixture::new(b"abc");
    let b = Fixture::new(b"abc");
    let c = Fixture::new(b"abd");
    let identity = a.hash();
    assert_eq!(
        identity.sha256,
        [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
            0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
            0xf2, 0x00, 0x15, 0xad
        ]
    );
    assert_eq!(identity.size_bytes, 3);
    assert!(identity.matches(&b.hash()));
    assert!(!identity.matches(&c.hash()));
}
#[test]
fn streaming_progress_and_cancellation() {
    let f = Fixture::new(&vec![7; CHUNK_BYTES * 2 + 23]);
    let mut handle = LocalHandle::open(&f.0).unwrap();
    let mut counts = vec![];
    handle
        .hash(|p| {
            counts.push(p.read_bytes);
            assert_eq!(p.total_bytes, (CHUNK_BYTES * 2 + 23) as u64);
            true
        })
        .unwrap();
    assert_eq!(
        counts,
        vec![
            0,
            CHUNK_BYTES as u64,
            (CHUNK_BYTES * 2) as u64,
            (CHUNK_BYTES * 2 + 23) as u64
        ]
    );
    assert_eq!(
        handle.hash(|p| p.read_bytes == 0).unwrap_err(),
        MediaError::Cancelled
    );
}
#[test]
fn modification_during_hash_never_returns_identity() {
    let f = Fixture::new(&vec![7; CHUNK_BYTES * 2]);
    let mut handle = LocalHandle::open(&f.0).unwrap();
    let mut changed = false;
    let result = handle.hash(|p| {
        if p.read_bytes > 0 && !changed {
            fs::OpenOptions::new()
                .append(true)
                .open(&f.0)
                .unwrap()
                .write_all(b"mutation")
                .unwrap();
            changed = true;
        }
        true
    });
    assert_eq!(result.unwrap_err(), MediaError::Modified);
}
#[test]
fn same_size_modification_and_path_replacement_are_detected() {
    let f = Fixture::new(b"abc");
    let handle = LocalHandle::open(&f.0).unwrap();
    fs::write(&f.0, b"abd").unwrap();
    // Do not depend on filesystem timestamp resolution or deferred NTFS updates.
    // Give the metadata guard an observable change; the digest tests separately
    // verify that differing bytes can never match content identity.
    let modified =
        fs::metadata(&f.0).unwrap().modified().unwrap() + std::time::Duration::from_secs(60);
    fs::File::options()
        .write(true)
        .open(&f.0)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(modified))
        .unwrap();
    assert_eq!(handle.unchanged(), Err(MediaError::Modified));
    let handle = LocalHandle::open(&f.0).unwrap();
    let replacement = Fixture::new(b"abd");
    fs::rename(&replacement.0, &f.0).unwrap();
    assert_eq!(handle.unchanged(), Err(MediaError::Modified));
}
#[test]
fn invalid_metadata_and_missing_empty_media() {
    for duration in ["NaN", "-1", "0", "9999999"] {
        let v = serde_json::json!({"format":{"duration":duration},"streams":[]});
        assert!(matches!(
            parse_probe(&v.to_string()),
            Err(MediaError::InvalidMetadata)
        ));
    }
    assert!(matches!(
        parse_probe("{}"),
        Err(MediaError::InvalidMetadata)
    ));
    let valid=parse_probe(r#"{"format":{"duration":"30.1","format_name":"mov,mp4"},"streams":[{"codec_name":"h264"},{"codec_name":"aac"}]}"#).unwrap();
    assert_eq!(valid.duration_ms, 30100);
    assert_eq!(valid.codecs, vec!["h264", "aac"]);
    let f = Fixture::new(b"");
    assert!(matches!(LocalHandle::open(&f.0), Err(MediaError::Empty)));
    fs::remove_file(&f.0).unwrap();
    assert!(matches!(LocalHandle::open(&f.0), Err(MediaError::NotFound)));
}
#[test]
fn player_duration_fallback_and_portable_descriptor() {
    let f = Fixture::new(b"abc");
    let handle = LocalHandle::open(&f.0).unwrap();
    let identity = f.hash();
    let local = LocalMedia {
        handle,
        identity,
        probe: None,
        hash_ms: 0.0,
    };
    let descriptor = local.descriptor("portable-test".into(), 30000).unwrap();
    assert_eq!(descriptor.title, None);
    assert_eq!(descriptor.mime, None);
    assert!(descriptor.codecs.is_empty());
    assert!(descriptor.valid_local());
    assert_eq!(
        local.descriptor("portable-test".into(), 0).unwrap_err(),
        MediaError::InvalidMetadata
    );
}

#[test]
fn platform_reader_handles_unknown_size_and_read_failure() {
    use cine_local_media::{MediaError, hash_reader};
    let a = hash_reader(std::io::Cursor::new(b"abc"), None, |p| {
        assert_eq!(p.total_bytes, 0);
        true
    })
    .unwrap();
    assert_eq!(a.size_bytes, 3);
    assert!(matches!(
        hash_reader(std::io::Cursor::new(b"abc"), Some(4), |_| true),
        Err(MediaError::Modified)
    ));
    struct Broken;
    impl std::io::Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::PermissionDenied.into())
        }
    }
    assert!(matches!(
        hash_reader(Broken, None, |_| true),
        Err(MediaError::ReadFailed)
    ));
}
