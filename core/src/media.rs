#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    LocalFile,
    P2pFile,
    Http,
    Hls,
    Dash,
    Jellyfin,
    Provider,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentIdentity {
    pub size_bytes: u64,
    pub sha256: [u8; 32],
}

impl ContentIdentity {
    pub fn matches(&self, other: &Self) -> bool {
        self == other
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaDescriptor {
    pub media_id: String,
    pub source_type: SourceType,
    pub title: Option<String>,
    pub duration_ms: u64,
    pub identity: ContentIdentity,
    pub mime: Option<String>,
    pub codecs: Vec<String>,
}

impl MediaDescriptor {
    /// A domain check, not a replacement for wire validation or UUID parsing.
    pub fn valid_local(&self) -> bool {
        self.source_type == SourceType::LocalFile
            && !self.media_id.is_empty()
            && self.duration_ms > 0
            && self.duration_ms <= 604_800_000
            && self.identity.size_bytes > 0
            && self.identity.size_bytes <= 1_099_511_627_776
            && self.title.as_ref().is_none_or(|v| v.len() <= 256)
            && self.mime.as_ref().is_none_or(|v| v.len() <= 128)
            && self.codecs.len() <= 16
            && self.codecs.iter().all(|v| !v.is_empty() && v.len() <= 64)
    }
}
