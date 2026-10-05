//! Device-only filesystem/probe/hash boundary. No paths enter descriptors.
use cine_core::media::{ContentIdentity, MediaDescriptor, SourceType};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, Metadata},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime},
};

pub const CHUNK_BYTES: usize = 1_048_576;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaError {
    NotFound,
    NotRegularFile,
    Empty,
    TooLarge,
    ReadFailed,
    Modified,
    Cancelled,
    ProbeFailed,
    InvalidMetadata,
}
impl std::fmt::Display for MediaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for MediaError {}
#[derive(Clone, Copy, Debug)]
pub struct HashProgress {
    pub read_bytes: u64,
    pub total_bytes: u64,
}
#[derive(Clone, PartialEq, Eq)]
struct Stamp {
    #[cfg(windows)]
    windows: WindowsStamp,
    size: u64,
    modified: Option<SystemTime>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    changed: (i64, i64),
}
impl Stamp {
    fn read(file: &File) -> Result<Self, MediaError> {
        let m = file.metadata().map_err(|_| MediaError::ReadFailed)?;
        Self::from_metadata(&m, file)
    }
    fn from_metadata(m: &Metadata, _file: &File) -> Result<Self, MediaError> {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        Ok(Self {
            #[cfg(windows)]
            windows: WindowsStamp::read(_file)?,
            size: m.len(),
            modified: m.modified().ok(),
            #[cfg(unix)]
            device: m.dev(),
            #[cfg(unix)]
            inode: m.ino(),
            #[cfg(unix)]
            changed: (m.ctime(), m.ctime_nsec()),
        })
    }
}
#[cfg(windows)]
#[derive(Clone, PartialEq, Eq)]
struct WindowsStamp {
    volume: u64,
    id: [u8; 16],
    changed: i64,
}
#[cfg(windows)]
impl WindowsStamp {
    fn read(file: &File) -> Result<Self, MediaError> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_BASIC_INFO, FILE_ID_INFO, FileBasicInfo, FileIdInfo, GetFileInformationByHandleEx,
        };
        let mut basic = FILE_BASIC_INFO::default();
        let mut id = FILE_ID_INFO::default();
        // The borrowed file keeps the HANDLE alive; both output buffers have the
        // exact SDK layout and size. This observes metadata, not content identity.
        let ok = unsafe {
            GetFileInformationByHandleEx(
                file.as_raw_handle(),
                FileBasicInfo,
                (&mut basic as *mut FILE_BASIC_INFO).cast(),
                std::mem::size_of::<FILE_BASIC_INFO>() as u32,
            ) != 0
                && GetFileInformationByHandleEx(
                    file.as_raw_handle(),
                    FileIdInfo,
                    (&mut id as *mut FILE_ID_INFO).cast(),
                    std::mem::size_of::<FILE_ID_INFO>() as u32,
                ) != 0
        };
        if !ok {
            return Err(MediaError::ReadFailed);
        }
        Ok(Self {
            volume: id.VolumeSerialNumber,
            id: id.FileId.Identifier,
            changed: basic.ChangeTime,
        })
    }
}
pub struct LocalHandle {
    path: PathBuf,
    file: File,
    stamp: Stamp,
}
impl LocalHandle {
    pub fn open(path: &Path) -> Result<Self, MediaError> {
        let path = path.canonicalize().map_err(|_| MediaError::NotFound)?;
        let file = File::open(&path).map_err(|_| MediaError::ReadFailed)?;
        let m = file.metadata().map_err(|_| MediaError::ReadFailed)?;
        if !m.is_file() {
            return Err(MediaError::NotRegularFile);
        }
        if m.len() == 0 {
            return Err(MediaError::Empty);
        }
        if m.len() > 1_099_511_627_776 {
            return Err(MediaError::TooLarge);
        }
        Ok(Self {
            path,
            stamp: Stamp::read(&file)?,
            file,
        })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn unchanged(&self) -> Result<(), MediaError> {
        let path = File::open(&self.path).map_err(|_| MediaError::Modified)?;
        if Stamp::read(&self.file)? != self.stamp || Stamp::read(&path)? != self.stamp {
            Err(MediaError::Modified)
        } else {
            Ok(())
        }
    }
    pub fn hash(
        &mut self,
        mut progress: impl FnMut(HashProgress) -> bool,
    ) -> Result<ContentIdentity, MediaError> {
        self.unchanged()?;
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|_| MediaError::ReadFailed)?;
        let identity = hash_reader(&mut self.file, Some(self.stamp.size), &mut progress)?;
        self.unchanged()?;
        Ok(identity)
    }
}
/// Shared bounded-memory hashing for a file or a platform-provided reader.
/// The caller retains the permission/handle and checks source stability separately.
pub fn hash_reader(
    mut reader: impl Read,
    expected_size: Option<u64>,
    mut progress: impl FnMut(HashProgress) -> bool,
) -> Result<ContentIdentity, MediaError> {
    let mut buffer = vec![0; CHUNK_BYTES];
    let mut hash = Sha256::new();
    let mut read = 0_u64;
    let total = expected_size.unwrap_or(0);
    if !progress(HashProgress {
        read_bytes: 0,
        total_bytes: total,
    }) {
        return Err(MediaError::Cancelled);
    }
    loop {
        let n = match reader.read(&mut buffer) {
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            other => other.map_err(|_| MediaError::ReadFailed)?,
        };
        if n == 0 {
            break;
        }
        read = read.checked_add(n as u64).ok_or(MediaError::TooLarge)?;
        if read > 1_099_511_627_776 {
            return Err(MediaError::TooLarge);
        }
        hash.update(&buffer[..n]);
        if !progress(HashProgress {
            read_bytes: read,
            total_bytes: total,
        }) {
            return Err(MediaError::Cancelled);
        }
    }
    if expected_size.is_some_and(|expected| expected != read) {
        return Err(MediaError::Modified);
    }
    if read == 0 {
        return Err(MediaError::Empty);
    }
    Ok(ContentIdentity {
        size_bytes: read,
        sha256: hash.finalize().into(),
    })
}

#[derive(Clone)]
pub struct Probe {
    pub duration_ms: u64,
    pub mime: Option<String>,
    pub codecs: Vec<String>,
}
pub fn parse_probe(text: &str) -> Result<Probe, MediaError> {
    let v: serde_json::Value =
        serde_json::from_str(text).map_err(|_| MediaError::InvalidMetadata)?;
    let seconds = v["format"]["duration"]
        .as_str()
        .and_then(|v| v.parse::<f64>().ok())
        .ok_or(MediaError::InvalidMetadata)?;
    if !seconds.is_finite() || !(0.001..=604800.0).contains(&seconds) {
        return Err(MediaError::InvalidMetadata);
    }
    let mut codecs = vec![];
    for stream in v["streams"].as_array().ok_or(MediaError::InvalidMetadata)? {
        if let Some(codec) = stream["codec_name"].as_str() {
            if codec.is_empty() || codec.len() > 64 {
                return Err(MediaError::InvalidMetadata);
            }
            if !codecs.iter().any(|v| v == codec) {
                codecs.push(codec.to_owned());
            }
        }
    }
    if codecs.len() > 16 {
        return Err(MediaError::InvalidMetadata);
    }
    let formats = v["format"]["format_name"].as_str().unwrap_or("");
    let mime = if formats.contains("mp4") {
        Some("video/mp4")
    } else if formats.contains("matroska") {
        Some("video/x-matroska")
    } else {
        None
    }
    .map(str::to_owned);
    Ok(Probe {
        duration_ms: (seconds * 1000.0).round() as u64,
        mime,
        codecs,
    })
}
fn probe(path: &Path) -> Result<Probe, MediaError> {
    // Both output streams go to a private temporary pipe; only bounded selected
    // fields are requested. Drain stdout concurrently to avoid pipe deadlock.
    let mut child = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-show_entries",
            "format=duration,format_name:stream=codec_name",
            "-of",
            "json",
        ])
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| MediaError::ProbeFailed)?;
    let stdout = child.stdout.take().ok_or(MediaError::ProbeFailed)?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.take(65_537).read_to_end(&mut bytes).map(|_| bytes)
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if start.elapsed() < Duration::from_secs(10) => {
                std::thread::sleep(Duration::from_millis(10))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let bytes = reader
        .join()
        .map_err(|_| MediaError::ProbeFailed)?
        .map_err(|_| MediaError::ProbeFailed)?;
    if bytes.len() > 65_536 || !status.is_some_and(|s| s.success()) {
        return Err(MediaError::ProbeFailed);
    }
    parse_probe(std::str::from_utf8(&bytes).map_err(|_| MediaError::InvalidMetadata)?)
}
pub struct LocalMedia {
    pub handle: LocalHandle,
    pub identity: ContentIdentity,
    pub probe: Option<Probe>,
    pub hash_ms: f64,
}
impl LocalMedia {
    pub fn inspect(
        path: &Path,
        progress: impl FnMut(HashProgress) -> bool,
    ) -> Result<Self, MediaError> {
        let mut handle = LocalHandle::open(path)?;
        let start = Instant::now();
        let identity = handle.hash(progress)?;
        let hash_ms = start.elapsed().as_secs_f64() * 1000.0;
        let probe = probe(handle.path()).ok();
        handle.unchanged()?;
        Ok(Self {
            handle,
            identity,
            probe,
            hash_ms,
        })
    }
    pub fn descriptor(
        &self,
        media_id: String,
        player_duration: u64,
    ) -> Result<MediaDescriptor, MediaError> {
        self.handle.unchanged()?;
        if self
            .probe
            .as_ref()
            .is_some_and(|p| p.duration_ms.abs_diff(player_duration) > 1000)
        {
            return Err(MediaError::InvalidMetadata);
        }
        let d = MediaDescriptor {
            media_id,
            source_type: SourceType::LocalFile,
            title: None,
            duration_ms: player_duration,
            identity: self.identity.clone(),
            mime: self.probe.as_ref().and_then(|p| p.mime.clone()),
            codecs: self.probe.as_ref().map_or(vec![], |p| p.codecs.clone()),
        };
        if !d.valid_local() {
            return Err(MediaError::InvalidMetadata);
        }
        Ok(d)
    }
}
