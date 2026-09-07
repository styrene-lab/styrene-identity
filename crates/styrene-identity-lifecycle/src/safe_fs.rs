//! Descriptor-relative operations for the application-owned local store.
//! This constrains pathname substitution; it is not a sandbox against code
//! running with the same UID, debugger access, or an administrator.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::LifecycleError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DirectoryIdentity {
    device: u64,
    inode: u64,
}

#[cfg(feature = "file-custody")]
pub(crate) type FileIdentity = DirectoryIdentity;

pub(crate) struct Directory {
    file: File,
    path: PathBuf,
}

impl Directory {
    #[cfg(feature = "file-custody")]
    fn open_owned_file(
        &self,
        name: &str,
        create: bool,
        writable: bool,
    ) -> Result<File, LifecycleError> {
        self.check_bound()?;
        self.require_owner(false)?;
        #[cfg(unix)]
        {
            use rustix::fs::{Mode, OFlags};
            let mut flags = (if writable { OFlags::RDWR } else { OFlags::RDONLY })
                | OFlags::NOFOLLOW
                | OFlags::NONBLOCK
                | OFlags::CLOEXEC;
            if create {
                flags |= OFlags::CREATE | OFlags::EXCL;
            }
            let file = File::from(
                rustix::fs::openat(&self.file, name, flags, Mode::RUSR | Mode::WUSR).map_err(
                    |error| {
                        if error == rustix::io::Errno::EXIST {
                            LifecycleError::DestinationConflict
                        } else {
                            map_errno(error)
                        }
                    },
                )?,
            );
            let metadata = file.metadata().map_err(|_| LifecycleError::OperationFailed)?;
            if !metadata.is_file() {
                return Err(LifecycleError::UnsafeStorage);
            }
            require_owner(&metadata, true, true)?;
            Ok(file)
        }
        #[cfg(not(unix))]
        {
            let _ = (name, create, writable);
            Err(LifecycleError::UnsupportedOperation)
        }
    }

    /// Reserve an empty name declared by a durable artifact intent. Existing
    /// nonempty files are never adopted as unproven staging ownership.
    #[cfg(feature = "file-custody")]
    pub(crate) fn reserve_empty(&self, name: &str) -> Result<FileIdentity, LifecycleError> {
        let file = match self.open_owned_file(name, true, true) {
            Ok(file) => file,
            Err(LifecycleError::DestinationConflict) => self.open_owned_file(name, false, true)?,
            Err(error) => return Err(error),
        };
        let metadata = file.metadata().map_err(|_| LifecycleError::OperationFailed)?;
        if metadata.len() != 0 {
            return Err(LifecycleError::DestinationConflict);
        }
        file.sync_all().map_err(|_| LifecycleError::OperationFailed)?;
        self.sync()?;
        self.check_bound()?;
        identity(&metadata)
    }

    #[cfg(feature = "file-custody")]
    pub(crate) fn write_empty_owned(
        &self,
        name: &str,
        expected: FileIdentity,
        bytes: &[u8],
    ) -> Result<(), LifecycleError> {
        use std::io::Write;
        let mut file = self.open_owned_file(name, false, true)?;
        let metadata = file.metadata().map_err(|_| LifecycleError::OperationFailed)?;
        if identity(&metadata)? != expected || metadata.len() != 0 {
            return Err(LifecycleError::DestinationConflict);
        }
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| LifecycleError::OperationFailed)?;
        self.check_bound()
    }

    #[cfg(feature = "file-custody")]
    pub(crate) fn snapshot_owned(
        &self,
        name: &str,
        max: u64,
    ) -> Result<(FileIdentity, Vec<u8>), LifecycleError> {
        let file = self.open_owned_file(name, false, false)?;
        let object = identity(&file.metadata().map_err(|_| LifecycleError::OperationFailed)?)?;
        let bytes = read_bounded(file, max)?;
        self.check_bound()?;
        Ok((object, bytes))
    }

    #[cfg(feature = "file-custody")]
    pub(crate) fn snapshot_expected(
        &self,
        name: &str,
        expected: FileIdentity,
        max: u64,
    ) -> Result<(FileIdentity, Vec<u8>), LifecycleError> {
        let file = self.open_owned_file(name, false, false)?;
        let object = identity(&file.metadata().map_err(|_| LifecycleError::OperationFailed)?)?;
        if object != expected {
            return Err(LifecycleError::DestinationConflict);
        }
        let bytes = read_bounded(file, max)?;
        self.check_bound()?;
        Ok((object, bytes))
    }

    #[cfg(feature = "file-custody")]
    pub(crate) fn rename_exclusive(&self, from: &str, to: &str) -> Result<(), LifecycleError> {
        self.require_owner(false)?;
        self.check_bound()?;
        #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
        {
            rustix::fs::renameat_with(
                &self.file,
                from,
                &self.file,
                to,
                rustix::fs::RenameFlags::NOREPLACE,
            )
            .map_err(|error| {
                if error == rustix::io::Errno::EXIST {
                    LifecycleError::DestinationConflict
                } else {
                    map_errno(error)
                }
            })?;
            self.sync()?;
            self.check_bound()
        }
        #[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
        {
            let _ = (from, to);
            Err(LifecycleError::UnsupportedOperation)
        }
    }

    /// Remove only a verified owned object. Callers quarantine a managed artifact
    /// before this step so a replacement at its public name is never deleted.
    #[cfg(feature = "file-custody")]
    pub(crate) fn remove_owned(
        &self,
        name: &str,
        expected: FileIdentity,
    ) -> Result<(), LifecycleError> {
        let file = self.open_owned_file(name, false, false)?;
        if identity(&file.metadata().map_err(|_| LifecycleError::OperationFailed)?)? != expected {
            return Err(LifecycleError::DestinationConflict);
        }
        #[cfg(unix)]
        {
            rustix::fs::unlinkat(&self.file, name, rustix::fs::AtFlags::empty())
                .map_err(map_errno)?;
            self.sync()?;
            self.check_bound()
        }
        #[cfg(not(unix))]
        {
            Err(LifecycleError::UnsupportedOperation)
        }
    }
    pub(crate) fn open(path: &Path) -> Result<Self, LifecycleError> {
        #[cfg(unix)]
        {
            use rustix::fs::{Mode, OFlags};
            let fd = rustix::fs::open(
                path,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(map_errno)?;
            Ok(Self { file: fd.into(), path: path.to_owned() })
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            Err(LifecycleError::UnsupportedOperation)
        }
    }

    pub(crate) fn identity(&self) -> Result<DirectoryIdentity, LifecycleError> {
        identity(&self.file.metadata().map_err(|_| LifecycleError::OperationFailed)?)
    }

    pub(crate) fn check_bound(&self) -> Result<(), LifecycleError> {
        let current =
            fs::symlink_metadata(&self.path).map_err(|_| LifecycleError::LocationChanged)?;
        if !current.is_dir() || identity(&current)? != self.identity()? {
            return Err(LifecycleError::LocationChanged);
        }
        Ok(())
    }

    #[cfg(feature = "file-custody")]
    pub(crate) fn require_owner(&self, private: bool) -> Result<(), LifecycleError> {
        require_owner(
            &self.file.metadata().map_err(|_| LifecycleError::OperationFailed)?,
            private,
            false,
        )
    }

    #[cfg(feature = "file-custody")]
    pub(crate) fn child(&self, name: &str, create: bool) -> Result<Self, LifecycleError> {
        self.check_bound()?;
        #[cfg(unix)]
        {
            use rustix::fs::{Mode, OFlags};
            if create {
                match rustix::fs::mkdirat(&self.file, name, Mode::RUSR | Mode::WUSR | Mode::XUSR) {
                    Ok(()) => self.sync()?,
                    Err(rustix::io::Errno::EXIST) => {}
                    Err(error) => return Err(map_errno(error)),
                }
            }
            let fd = rustix::fs::openat(
                &self.file,
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(map_errno)?;
            let directory = Self { file: fd.into(), path: self.path.join(name) };
            directory.require_owner(true)?;
            self.check_bound()?;
            Ok(directory)
        }
        #[cfg(not(unix))]
        {
            let _ = (name, create);
            Err(LifecycleError::UnsupportedOperation)
        }
    }

    #[cfg(feature = "file-custody")]
    pub(crate) fn lock_file(&self, name: &str) -> Result<File, LifecycleError> {
        self.check_bound()?;
        #[cfg(unix)]
        {
            use rustix::fs::{Mode, OFlags};
            let fd = rustix::fs::openat(
                &self.file,
                name,
                OFlags::RDWR
                    | OFlags::CREATE
                    | OFlags::NOFOLLOW
                    | OFlags::NONBLOCK
                    | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(map_errno)?;
            let file = File::from(fd);
            let metadata = file.metadata().map_err(|_| LifecycleError::OperationFailed)?;
            if !metadata.is_file() {
                return Err(LifecycleError::UnsafeStorage);
            }
            require_owner(&metadata, true, true)?;
            self.check_bound()?;
            Ok(file)
        }
        #[cfg(not(unix))]
        {
            let _ = name;
            Err(LifecycleError::UnsupportedOperation)
        }
    }

    pub(crate) fn read(
        &self,
        name: &std::ffi::OsStr,
        max: u64,
        private: bool,
    ) -> Result<Vec<u8>, LifecycleError> {
        self.read_with_policy(name, max, private.then_some(true))
    }

    #[cfg(feature = "file-custody")]
    pub(crate) fn read_owned(
        &self,
        name: &std::ffi::OsStr,
        max: u64,
    ) -> Result<Vec<u8>, LifecycleError> {
        self.read_with_policy(name, max, Some(false))
    }

    fn read_with_policy(
        &self,
        name: &std::ffi::OsStr,
        max: u64,
        owner_policy: Option<bool>,
    ) -> Result<Vec<u8>, LifecycleError> {
        self.check_bound()?;
        #[cfg(unix)]
        {
            use rustix::fs::{Mode, OFlags};
            let fd = rustix::fs::openat(
                &self.file,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(map_errno)?;
            let file = File::from(fd);
            let metadata = file.metadata().map_err(|_| LifecycleError::OperationFailed)?;
            if !metadata.is_file() {
                return Err(LifecycleError::OperationFailed);
            }
            if let Some(private) = owner_policy {
                require_owner(&metadata, private, true)?;
            }
            let bytes = read_bounded(file, max)?;
            self.check_bound()?;
            Ok(bytes)
        }
        #[cfg(not(unix))]
        {
            let _ = (name, max, owner_policy);
            Err(LifecycleError::UnsupportedOperation)
        }
    }

    #[cfg(feature = "file-custody")]
    pub(crate) fn names(&self) -> Result<Vec<String>, LifecycleError> {
        self.check_bound()?;
        #[cfg(unix)]
        {
            let directory = rustix::fs::Dir::read_from(&self.file).map_err(map_errno)?;
            let mut names = Vec::new();
            for entry in directory {
                let entry = entry.map_err(map_errno)?;
                let name =
                    entry.file_name().to_str().map_err(|_| LifecycleError::OperationFailed)?;
                if name == "." || name == ".." {
                    continue;
                }
                if names.len() >= 2048 {
                    return Err(LifecycleError::OperationFailed);
                }
                names.push(name.to_owned());
            }
            self.check_bound()?;
            Ok(names)
        }
        #[cfg(not(unix))]
        {
            Err(LifecycleError::UnsupportedOperation)
        }
    }

    #[cfg(feature = "file-custody")]
    pub(crate) fn write(
        &self,
        name: &std::ffi::OsStr,
        bytes: &[u8],
        replace: bool,
    ) -> Result<(), LifecycleError> {
        self.require_owner(false)?;
        self.check_bound()?;
        #[cfg(unix)]
        {
            use rand_core::RngCore;
            use rustix::fs::{AtFlags, Mode, OFlags};
            use std::io::Write;
            let mut suffix = [0; 16];
            rand_core::OsRng.fill_bytes(&mut suffix);
            let temporary = format!(".idctl-{}", hex::encode(suffix));
            let fd = rustix::fs::openat(
                &self.file,
                temporary.as_str(),
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(map_errno)?;
            let mut file = File::from(fd);
            let result = (|| {
                file.write_all(bytes)
                    .and_then(|()| file.sync_all())
                    .map_err(|_| LifecycleError::OperationFailed)?;
                self.check_bound()?;
                if replace {
                    rustix::fs::renameat(&self.file, temporary.as_str(), &self.file, name)
                        .map_err(map_errno)?;
                } else {
                    // An exclusive rename avoids the two-hard-link interval of
                    // link+unlink installation. A crash in that interval would
                    // otherwise violate our single-link authority-file rule.
                    #[cfg(any(
                        target_os = "linux",
                        target_os = "android",
                        target_vendor = "apple"
                    ))]
                    rustix::fs::renameat_with(
                        &self.file,
                        temporary.as_str(),
                        &self.file,
                        name,
                        rustix::fs::RenameFlags::NOREPLACE,
                    )
                    .map_err(|error| {
                        if error == rustix::io::Errno::EXIST {
                            LifecycleError::DestinationConflict
                        } else {
                            map_errno(error)
                        }
                    })?;
                    #[cfg(not(any(
                        target_os = "linux",
                        target_os = "android",
                        target_vendor = "apple"
                    )))]
                    return Err(LifecycleError::UnsupportedOperation);
                }
                Ok(())
            })();
            // Never resolve the temporary path again through mutable ancestors.
            let cleanup = rustix::fs::unlinkat(&self.file, temporary.as_str(), AtFlags::empty());
            let synced = self.sync();
            result?;
            match cleanup {
                Ok(()) | Err(rustix::io::Errno::NOENT) => {}
                Err(error) => return Err(map_errno(error)),
            }
            synced?;
            self.check_bound()
        }
        #[cfg(not(unix))]
        {
            let _ = (name, bytes, replace);
            Err(LifecycleError::UnsupportedOperation)
        }
    }

    #[cfg(feature = "file-custody")]
    pub(crate) fn sync(&self) -> Result<(), LifecycleError> {
        self.file.sync_all().map_err(|_| LifecycleError::OperationFailed)
    }
}

pub(crate) fn read(path: &Path, max: u64, private: bool) -> Result<Vec<u8>, LifecycleError> {
    #[cfg(unix)]
    {
        let parent =
            path.parent().filter(|path| !path.as_os_str().is_empty()).unwrap_or(Path::new("."));
        Directory::open(parent)?.read(
            path.file_name().ok_or(LifecycleError::InvalidRequest)?,
            max,
            private,
        )
    }
    #[cfg(not(unix))]
    {
        let _ = private;
        let metadata = fs::symlink_metadata(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                LifecycleError::OperationNotFound
            } else {
                LifecycleError::OperationFailed
            }
        })?;
        if !metadata.is_file() {
            return Err(LifecycleError::OperationFailed);
        }
        read_bounded(File::open(path).map_err(|_| LifecycleError::OperationFailed)?, max)
    }
}

fn read_bounded(file: File, max: u64) -> Result<Vec<u8>, LifecycleError> {
    if file.metadata().map_err(|_| LifecycleError::OperationFailed)?.len() > max {
        return Err(LifecycleError::CatalogTooLarge);
    }
    let mut bytes = Vec::new();
    file.take(max + 1).read_to_end(&mut bytes).map_err(|_| LifecycleError::OperationFailed)?;
    if bytes.len() as u64 > max {
        return Err(LifecycleError::CatalogTooLarge);
    }
    Ok(bytes)
}

#[cfg(unix)]
fn require_owner(
    metadata: &fs::Metadata,
    private: bool,
    single_link: bool,
) -> Result<(), LifecycleError> {
    use std::os::unix::fs::MetadataExt;
    let forbidden = if private { 0o077 } else { 0o022 };
    if metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & forbidden != 0
        || (single_link && metadata.nlink() != 1)
    {
        return Err(LifecycleError::UnsafeStorage);
    }
    Ok(())
}

#[cfg(all(not(unix), feature = "file-custody"))]
fn require_owner(_: &fs::Metadata, _: bool, _: bool) -> Result<(), LifecycleError> {
    Err(LifecycleError::UnsupportedOperation)
}

fn identity(metadata: &fs::Metadata) -> Result<DirectoryIdentity, LifecycleError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(DirectoryIdentity { device: metadata.dev(), inode: metadata.ino() })
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        Err(LifecycleError::UnsupportedOperation)
    }
}

#[cfg(unix)]
fn map_errno(error: rustix::io::Errno) -> LifecycleError {
    match error {
        rustix::io::Errno::NOENT => LifecycleError::OperationNotFound,
        rustix::io::Errno::LOOP | rustix::io::Errno::NOTDIR => LifecycleError::LocationChanged,
        _ => LifecycleError::OperationFailed,
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn special_files_and_symlinks_are_rejected_without_blocking() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = Directory::open(temporary.path()).unwrap();
        let _socket =
            std::os::unix::net::UnixListener::bind(temporary.path().join("socket")).unwrap();
        assert!(directory.read(std::ffi::OsStr::new("socket"), 97, false).is_err());
        fs::write(temporary.path().join("file"), b"public").unwrap();
        std::os::unix::fs::symlink("file", temporary.path().join("alias")).unwrap();
        assert!(directory.read(std::ffi::OsStr::new("alias"), 97, false).is_err());
    }

    #[cfg(feature = "file-custody")]
    #[test]
    fn held_directory_never_redirects_writes_to_a_replacement() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("original");
        fs::create_dir(&path).unwrap();
        let directory = Directory::open(&path).unwrap();
        fs::rename(&path, temporary.path().join("retained")).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(
            directory.write(std::ffi::OsStr::new("identity.key"), b"ciphertext", false).is_err()
        );
        assert!(!path.join("identity.key").exists());
        assert!(!temporary.path().join("retained/identity.key").exists());
    }

    #[cfg(all(
        feature = "file-custody",
        any(target_os = "linux", target_os = "android", target_vendor = "apple")
    ))]
    #[test]
    fn exclusive_install_preserves_destination_and_single_link_authority() {
        use std::os::unix::fs::MetadataExt;
        let temporary = tempfile::tempdir().unwrap();
        let directory = Directory::open(temporary.path()).unwrap();
        let name = std::ffi::OsStr::new("receipt.json");
        directory.write(name, b"original", false).unwrap();
        assert_eq!(fs::metadata(temporary.path().join(name)).unwrap().nlink(), 1);
        assert_eq!(
            directory.write(name, b"replacement", false),
            Err(LifecycleError::DestinationConflict)
        );
        assert_eq!(fs::read(temporary.path().join(name)).unwrap(), b"original");
        assert_eq!(directory.names().unwrap(), ["receipt.json"]);
    }
}
