//! Bounded, non-mutating backup inspection and authenticated identity discovery.
//! Authentication proves the encrypted payload, not external ownership or the
//! unauthenticated STID format header. No credential or plaintext root is returned.

use std::path::Path;

use serde::Serialize;
use sha2::{Digest, Sha256};
use styrene_identity::IdentityId;
use styrene_identity::identity::identity_pubkey;
use styrene_identity::vault::{EncryptedIdentityBackup, EncryptedIdentityBackupFormat};

use crate::{LifecycleError, safe_fs};

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupFormat {
    LegacyV0,
    StidV1,
}

#[derive(Clone, Debug, Serialize)]
pub struct BackupInspection {
    pub format: BackupFormat,
    pub encrypted_size: u64,
    pub sha256: String,
    pub payload_authenticated: bool,
    pub format_header_authenticated: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct VerifiedBackup {
    pub artifact: BackupInspection,
    pub identity_id: String,
    pub public_key: String,
    pub expected_identity_matched: Option<bool>,
    pub root_exposure: &'static str,
}

/// Parse at most 97 bytes. Neither a digest nor parsed metadata authenticates a backup.
pub fn inspect(path: &Path) -> Result<BackupInspection, LifecycleError> {
    let backup = read_backup(path)?;
    Ok(metadata(&backup, false))
}

/// Authenticate once and bind the same decrypted root to an optional expected ID.
/// This blocking KDF belongs on a bounded worker when called by an async UI host.
pub fn verify(
    path: &Path,
    expected: Option<IdentityId>,
    protection: &[u8],
) -> Result<VerifiedBackup, LifecycleError> {
    if protection.is_empty() {
        return Err(LifecycleError::AuthenticationRequired);
    }
    if protection.len() > 4096 {
        return Err(LifecycleError::InvalidRequest);
    }
    let backup = read_backup(path)?;
    verify_artifact(backup, expected, protection)
}

pub(crate) fn verify_artifact(
    backup: EncryptedIdentityBackup,
    expected: Option<IdentityId>,
    protection: &[u8],
) -> Result<VerifiedBackup, LifecycleError> {
    if protection.is_empty() {
        return Err(LifecycleError::AuthenticationRequired);
    }
    if protection.len() > 4096 {
        return Err(LifecycleError::InvalidRequest);
    }
    let root =
        backup.decrypt_root_secret(protection).map_err(|_| LifecycleError::AuthenticationFailed)?;
    let public_key = identity_pubkey(&root);
    if expected.is_some_and(|id| !id.matches_public_key(&public_key)) {
        return Err(LifecycleError::IdentityMismatch);
    }
    Ok(VerifiedBackup {
        artifact: metadata(&backup, true),
        identity_id: IdentityId::from_public_key(&public_key).to_string(),
        public_key: hex::encode(public_key),
        expected_identity_matched: expected.map(|_| true),
        root_exposure: "host_memory",
    })
}

fn read_backup(path: &Path) -> Result<EncryptedIdentityBackup, LifecycleError> {
    let bytes = safe_fs::read(path, 97, false).map_err(|error| match error {
        LifecycleError::CatalogTooLarge => LifecycleError::InvalidBackup,
        other => other,
    })?;
    EncryptedIdentityBackup::from_encrypted_bytes(bytes).map_err(|_| LifecycleError::InvalidBackup)
}

fn metadata(backup: &EncryptedIdentityBackup, authenticated: bool) -> BackupInspection {
    let metadata = backup.metadata();
    BackupInspection {
        format: match metadata.format {
            EncryptedIdentityBackupFormat::LegacyV0 => BackupFormat::LegacyV0,
            EncryptedIdentityBackupFormat::StidV1 => BackupFormat::StidV1,
        },
        encrypted_size: metadata.encrypted_size,
        sha256: hex::encode(Sha256::digest(backup.encrypted_bytes())),
        payload_authenticated: authenticated,
        // Legacy compatibility permits stripping the STID header without changing
        // the authenticated payload. A future header-bound format needs versioning.
        format_header_authenticated: false,
    }
}
