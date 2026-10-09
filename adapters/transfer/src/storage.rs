use crate::{Error, Manifest};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
// In-memory checkpoints survive socket/room-session reconnect, never process death.
pub struct Partial {
    pub manifest: Manifest,
    file: File,
    directory: PathBuf,
    hashes: Vec<Option<[u8; 32]>>,
    pub verified: u64,
    completed: bool,
}
impl Partial {
    pub fn create(root: &Path, manifest: Manifest) -> Result<Self, Error> {
        manifest.validate()?;
        let root = root.canonicalize().map_err(|_| Error::Storage)?;
        if let Some(free) = available(&root)?
            && free < manifest.size_bytes.saturating_add(64 * 1024 * 1024)
        {
            return Err(Error::Space);
        }
        let directory = root.join(format!("cine-{}", manifest.transfer_id));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&directory).map_err(|_| Error::Storage)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = match options.open(directory.join("media.part")) {
            Ok(f) => f,
            Err(_) => {
                let _ = std::fs::remove_dir(&directory);
                return Err(Error::Storage);
            }
        };
        // Sparse length is bounded. Actual free-space checks belong to platform/Application.
        let hashes = vec![None; manifest.chunk_count as usize];
        Ok(Self {
            manifest,
            file,
            directory,
            hashes,
            verified: 0,
            completed: false,
        })
    }
    pub fn missing(&self) -> impl Iterator<Item = u32> + '_ {
        self.hashes
            .iter()
            .enumerate()
            .filter(|(_, h)| h.is_none())
            .map(|(i, _)| i as u32)
    }
    pub fn accept(
        &mut self,
        index: u32,
        offset: u64,
        data: &[u8],
        hash: [u8; 32],
    ) -> Result<bool, Error> {
        let (expected, len) = self.manifest.chunk(index)?;
        if offset != expected || data.len() != len {
            return Err(Error::Bounds);
        }
        let actual: [u8; 32] = Sha256::digest(data).into();
        if actual != hash {
            return Err(Error::Corrupt);
        }
        if let Some(old) = self.hashes[index as usize] {
            return if old == hash {
                Ok(false)
            } else {
                Err(Error::Corrupt)
            };
        }
        self.file.seek(SeekFrom::Start(expected))?;
        self.file.write_all(data)?;
        self.hashes[index as usize] = Some(hash);
        self.verified += data.len() as u64;
        Ok(true)
    }
    pub fn revalidate(
        &mut self,
        mut active: impl FnMut() -> Result<(), Error>,
    ) -> Result<(), Error> {
        let mut data = vec![0; self.manifest.chunk_size as usize];
        self.verified = 0;
        for index in 0..self.manifest.chunk_count {
            active()?;
            if let Some(old) = self.hashes[index as usize] {
                let (offset, len) = self.manifest.chunk(index)?;
                self.file.seek(SeekFrom::Start(offset))?;
                if self.file.read_exact(&mut data[..len]).is_err()
                    || <[u8; 32]>::from(Sha256::digest(&data[..len])) != old
                {
                    self.hashes[index as usize] = None;
                } else {
                    self.verified += len as u64;
                }
            }
        }
        Ok(())
    }
    pub fn finish(
        &mut self,
        mut active: impl FnMut() -> Result<(), Error>,
    ) -> Result<PathBuf, Error> {
        if self.missing().next().is_some() {
            return Err(Error::Identity);
        }
        self.file.set_len(self.manifest.size_bytes)?;
        self.file.sync_all()?;
        self.file.seek(SeekFrom::Start(0))?;
        let identity =
            cine_local_media::hash_reader(&mut self.file, Some(self.manifest.size_bytes), |_| {
                active().is_ok()
            })
            .map_err(|e| {
                if e == cine_local_media::MediaError::Cancelled {
                    active().err().unwrap_or(Error::Cancelled)
                } else {
                    Error::Io
                }
            })?;
        active()?;
        if identity != self.manifest.identity() {
            return Err(Error::Identity);
        }
        let complete = self.directory.join("media.mp4");
        // No replacement of an existing final file, even if a user creates a conflict.
        commit(&self.directory.join("media.part"), &complete)?;
        #[cfg(unix)]
        File::open(&self.directory)?.sync_all()?;
        self.completed = true;
        Ok(complete)
    }
    pub fn discard(&mut self) -> Result<(), Error> {
        if self.completed {
            return Err(Error::Storage);
        }
        std::fs::remove_file(self.directory.join("media.part"))?;
        std::fs::remove_dir(&self.directory)?;
        Ok(())
    }
}

fn commit(part: &Path, complete: &Path) -> Result<(), Error> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};
        let from = CString::new(part.as_os_str().as_bytes()).map_err(|_| Error::Storage)?;
        let to = CString::new(complete.as_os_str().as_bytes()).map_err(|_| Error::Storage)?;
        // Both C strings live through this syscall. RENAME_NOREPLACE is atomic and
        // cannot overwrite another file or follow a destination symlink. Fail closed
        // on a kernel/filesystem that does not support it.
        let result = unsafe {
            libc::syscall(
                libc::SYS_renameat2,
                libc::AT_FDCWD,
                from.as_ptr(),
                libc::AT_FDCWD,
                to.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        if result != 0 {
            return Err(Error::Storage);
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        std::fs::hard_link(part, complete).map_err(|_| Error::Storage)?;
        std::fs::remove_file(part)?;
    }
    Ok(())
}

pub fn available(path: &Path) -> Result<Option<u64>, Error> {
    #[cfg(unix)]
    {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};
        let path = CString::new(path.as_os_str().as_bytes()).map_err(|_| Error::Storage)?;
        let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
        // CString and output buffer remain valid for the call. Read only on success.
        if unsafe { libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) } != 0 {
            return Err(Error::Storage);
        }
        let stat = unsafe { stat.assume_init() };
        Ok(Some(
            u64::try_from(u128::from(stat.f_bavail) * u128::from(stat.f_frsize))
                .unwrap_or(u64::MAX),
        ))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(None)
    }
}
