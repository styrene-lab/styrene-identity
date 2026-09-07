#![cfg(feature = "file-custody")]

use std::fs;
use styrene_identity::signer::RootSecret;
use styrene_identity::vault::EncryptedIdentityBackup;
use styrene_identity::{IdentityId, identity_pubkey};
use styrene_identity_lifecycle::{LifecycleError, backups};

#[test]
fn id31_id32_inspection_authentication_and_header_evidence_are_distinct() {
    let dir = tempfile::tempdir().unwrap();
    let root = RootSecret::ephemeral();
    let expected = IdentityId::from_public_key(&identity_pubkey(&root));
    let backup = EncryptedIdentityBackup::protect_root_secret(&root, b"test-protection").unwrap();
    let path = dir.path().join("backup.stid");
    fs::write(&path, backup.encrypted_bytes()).unwrap();
    assert!(!backups::inspect(&path).unwrap().payload_authenticated);
    let verified = backups::verify(&path, Some(expected), b"test-protection").unwrap();
    assert!(verified.artifact.payload_authenticated);
    assert!(!verified.artifact.format_header_authenticated);
    assert_eq!(verified.expected_identity_matched, Some(true));
    let legacy = dir.path().join("legacy.key");
    fs::write(&legacy, &backup.encrypted_bytes()[5..]).unwrap();
    let verified_legacy = backups::verify(&legacy, Some(expected), b"test-protection").unwrap();
    assert_eq!(verified.identity_id, verified_legacy.identity_id);
    assert_ne!(verified.artifact.sha256, verified_legacy.artifact.sha256);
    assert_eq!(fs::read(path).unwrap(), backup.encrypted_bytes());
}

#[test]
fn id32_wrong_identity_wrong_password_and_tampered_payload_fail() {
    let dir = tempfile::tempdir().unwrap();
    let root = RootSecret::ephemeral();
    let backup = EncryptedIdentityBackup::protect_root_secret(&root, b"test-protection").unwrap();
    let path = dir.path().join("backup.stid");
    fs::write(&path, backup.encrypted_bytes()).unwrap();
    assert!(matches!(
        backups::verify(&path, Some(IdentityId::from_bytes([0; 16])), b"test-protection"),
        Err(LifecycleError::IdentityMismatch)
    ));
    assert!(matches!(
        backups::verify(&path, None, b"wrong"),
        Err(LifecycleError::AuthenticationFailed)
    ));
    assert!(matches!(
        backups::verify(&path, None, &[]),
        Err(LifecycleError::AuthenticationRequired)
    ));
    let mut corrupted = backup.encrypted_bytes().to_vec();
    corrupted[96] ^= 1;
    fs::write(&path, &corrupted).unwrap();
    assert!(!backups::inspect(&path).unwrap().payload_authenticated);
    assert!(matches!(
        backups::verify(&path, None, b"test-protection"),
        Err(LifecycleError::AuthenticationFailed)
    ));
    fs::write(&path, vec![0; 98]).unwrap();
    assert!(matches!(backups::inspect(&path), Err(LifecycleError::InvalidBackup)));
}
