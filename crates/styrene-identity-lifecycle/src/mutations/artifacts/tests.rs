use super::*;

const SOURCE: &[u8] = b"fixture-source-protection";
const BACKUP: &[u8] = b"fixture-backup-protection";
const NEW: &[u8] = b"fixture-new-protection";

fn setup() -> (tempfile::TempDir, PathBuf, PathBuf, IdentityId) {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("store");
    let custody = dir.path().join("custody.key");
    let created = super::super::execute(
        &store,
        "source",
        Mutation::Create { name: "Source".into(), destination: custody.clone() },
        SOURCE,
    )
    .unwrap();
    (dir, store, custody, created.identity_id.unwrap().parse().unwrap())
}

fn export(output: &Path, expected: IdentityId) -> BackupMutation {
    BackupMutation::Export {
        entry_id: "identity-source".into(),
        output: output.into(),
        expected_identity: expected,
    }
}

#[test]
fn id30_id34_export_reprotect_inventory_and_managed_removal() {
    let (dir, store, custody, expected) = setup();
    let original = fs::read(&custody).unwrap();
    let output = dir.path().join("backup.stid");
    let first = execute(&store, "export", export(&output, expected), SOURCE, BACKUP).unwrap();
    assert_eq!(
        crate::backups::verify(&output, Some(expected), BACKUP).unwrap().identity_id,
        expected.to_string()
    );
    assert!(crate::backups::verify(&output, Some(expected), SOURCE).is_err());
    let second_path = dir.path().join("new.stid");
    let second = execute(
        &store,
        "reprotect",
        BackupMutation::Reprotect {
            input: output.clone(),
            output: second_path.clone(),
            expected_identity: expected,
        },
        BACKUP,
        NEW,
    )
    .unwrap();
    assert_ne!(first.sha256, second.sha256);
    assert!(crate::backups::verify(&second_path, Some(expected), NEW).is_ok());
    assert!(output.exists());
    assert_eq!(list(&store, Some(expected)).unwrap().len(), 2);
    execute(
        &store,
        "forget",
        BackupMutation::Forget {
            artifact_id: "backup-export".into(),
            expected_digest: first.sha256,
        },
        &[],
        &[],
    )
    .unwrap();
    assert!(output.exists());
    execute(
        &store,
        "delete",
        BackupMutation::Delete {
            artifact_id: "backup-reprotect".into(),
            expected_digest: second.sha256,
        },
        &[],
        &[],
    )
    .unwrap();
    assert!(!second_path.exists());
    assert_eq!(show(&store, "backup-reprotect").unwrap().status, "deleted");
    assert!(list(&store, None).unwrap().is_empty());
    assert_eq!(fs::read(custody).unwrap(), original);
}

#[test]
fn id33_restore_reprotects_custody_registers_catalog_and_retries_without_overwrite() {
    let (dir, store, _, expected) = setup();
    let backup = dir.path().join("backup.stid");
    execute(&store, "export", export(&backup, expected), SOURCE, BACKUP).unwrap();
    let restored_store = dir.path().join("restored");
    let destination = dir.path().join("restored.key");
    let request = BackupMutation::Restore {
        input: backup.clone(),
        destination: destination.clone(),
        name: "Restored".into(),
        expected_identity: expected,
    };
    let first = execute(&restored_store, "restore", request.clone(), BACKUP, NEW).unwrap();
    let snapshot = CatalogSnapshot::load(&restored_store).unwrap();
    assert_eq!(snapshot.list().entries.len(), 1);
    assert_eq!(
        snapshot.show(first.entry_id.as_ref().unwrap(), Some(expected)).unwrap().entry_id,
        first.entry_id.clone().unwrap()
    );
    let bytes = fs::read(&destination).unwrap();
    let repeated = execute(&restored_store, "restore", request.clone(), &[], &[]).unwrap();
    assert!(repeated.replayed);
    assert_eq!(
        execute(&restored_store, "same-root", request, BACKUP, NEW).unwrap().entry_id,
        first.entry_id
    );
    assert_eq!(fs::read(destination).unwrap(), bytes);
    assert!(backup.exists());
}

#[test]
fn ownership_is_durable_before_payload_and_every_write_boundary_recovers() {
    for cut in [Cut::Intent, Cut::Ownership, Cut::Payload, Cut::Installed] {
        let (dir, store, _, expected) = setup();
        let output = dir.path().join("backup.stid");
        execute_hook(
            &store,
            "interrupted",
            export(&output, expected),
            SOURCE,
            BACKUP,
            &mut |phase| {
                if phase == cut { Err(LifecycleError::OperationFailed) } else { Ok(()) }
            },
        )
        .unwrap_err();
        let directory = Directory::open(&store.join("artifacts")).unwrap();
        let record = load(&directory, "backup-op-interrupted").unwrap();
        let parent = bound_parent(&record).unwrap();
        for stage in &record.stages {
            if let Ok((object, bytes)) = parent.snapshot_owned(&stage.name, 97)
                && !bytes.is_empty()
            {
                assert_eq!(stage.object, Some(object));
            }
        }
        let result = reconcile(&store, "backup-op-interrupted", SOURCE, BACKUP).unwrap();
        assert_eq!(result.identity_id, expected.to_string());
        assert!(crate::backups::verify(&output, Some(expected), BACKUP).is_ok());
        let completed = load(&directory, "backup-op-interrupted").unwrap();
        assert!(completed.stages.is_empty());
    }
}

#[test]
fn partial_owned_ciphertext_is_preserved_until_new_generation_is_installed() {
    let (dir, store, _, expected) = setup();
    let output = dir.path().join("backup.stid");
    execute_hook(&store, "partial", export(&output, expected), SOURCE, BACKUP, &mut |phase| {
        if phase == Cut::Ownership { Err(LifecycleError::OperationFailed) } else { Ok(()) }
    })
    .unwrap_err();
    let directory = Directory::open(&store.join("artifacts")).unwrap();
    let record = load(&directory, "backup-op-partial").unwrap();
    let parent = bound_parent(&record).unwrap();
    let stage = &record.stages[0];
    parent.write_empty_owned(&stage.name, stage.object.unwrap(), b"partial").unwrap();
    execute_hook(&store, "partial", export(&output, expected), SOURCE, NEW, &mut |phase| {
        if phase == Cut::Payload {
            assert_eq!(parent.read(OsStr::new(&stage.name), 97, false).unwrap(), b"partial");
            Err(LifecycleError::OperationFailed)
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    reconcile(&store, "backup-op-partial", SOURCE, NEW).unwrap();
    assert!(crate::backups::verify(&output, Some(expected), NEW).is_ok());
    assert!(parent.read(OsStr::new(&stage.name), 97, false).is_err());
}

#[test]
fn replacement_objects_are_not_deleted_and_quarantined_delete_recovers() {
    let (dir, store, _, expected) = setup();
    let output = dir.path().join("backup.stid");
    let result = execute(&store, "export", export(&output, expected), SOURCE, BACKUP).unwrap();
    let bytes = fs::read(&output).unwrap();
    let held = File::open(&output).unwrap();
    fs::remove_file(&output).unwrap();
    fs::write(&output, &bytes).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&output, fs::Permissions::from_mode(0o600)).unwrap();
    let delete = BackupMutation::Delete {
        artifact_id: "backup-export".into(),
        expected_digest: result.sha256,
    };
    assert_eq!(
        execute(&store, "delete", delete, SOURCE, BACKUP).unwrap_err().error,
        LifecycleError::DestinationConflict
    );
    assert_eq!(fs::read(&output).unwrap(), bytes);
    drop(held);
    let second = dir.path().join("second.stid");
    let result = execute(&store, "second", export(&second, expected), SOURCE, BACKUP).unwrap();
    execute_hook(
        &store,
        "delete-second",
        BackupMutation::Delete {
            artifact_id: "backup-second".into(),
            expected_digest: result.sha256,
        },
        &[],
        &[],
        &mut |phase| {
            if phase == Cut::Quarantined { Err(LifecycleError::OperationFailed) } else { Ok(()) }
        },
    )
    .unwrap_err();
    assert!(!second.exists());
    reconcile(&store, "backup-op-delete-second", &[], &[]).unwrap();
    assert_eq!(show(&store, "backup-second").unwrap().status, "deleted");
}

#[test]
fn legacy_pending_recovery_can_migrate_without_overwriting_original_intent() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("store");
    let original = dir.path().join("original.key");
    super::super::execute_with_hook(
        &store,
        "legacy",
        Mutation::Create { name: "Legacy".into(), destination: original.clone() },
        SOURCE,
        &mut |phase| {
            if phase == Phase::JournalPrepared {
                Err(LifecycleError::OperationFailed)
            } else {
                Ok(())
            }
        },
    )
    .unwrap_err();
    let path = store.join("operations/op-legacy.json");
    let mut record = read_journal(&path).unwrap();
    record.schema_version = 1;
    record.parent_identity = None;
    let expected = record.result.identity_id.clone().unwrap().parse().unwrap();
    fs::write(&path, journal_bytes(&record).unwrap()).unwrap();
    let output = dir.path().join("recovered.stid");
    execute(
        &store,
        "migrate",
        BackupMutation::MigrateRecovery {
            source_operation: "op-legacy".into(),
            output: output.clone(),
            expected_identity: expected,
        },
        SOURCE,
        BACKUP,
    )
    .unwrap();
    assert!(!original.exists());
    assert!(crate::backups::verify(&output, Some(expected), BACKUP).is_ok());
    let migrated = read_journal(&path).unwrap();
    assert_eq!(migrated.state, OperationState::Superseded);
    assert!(migrated.encrypted_creation.is_none());
    assert_eq!(migrated.recovery_migrated_to.as_deref(), Some("backup-migrate"));
    assert_eq!(
        super::super::execute(
            &store,
            "legacy",
            Mutation::Create { name: "Legacy".into(), destination: original },
            SOURCE
        )
        .unwrap_err()
        .error,
        LifecycleError::OperationSuperseded
    );
}

#[test]
fn restore_catalog_cut_recovers_and_restore_cannot_consume_its_source() {
    let (dir, store, _, expected) = setup();
    let backup = dir.path().join("backup.stid");
    execute(&store, "export", export(&backup, expected), SOURCE, BACKUP).unwrap();
    assert_eq!(
        execute(
            &store,
            "bad-restore",
            BackupMutation::Restore {
                input: backup.clone(),
                destination: backup.clone(),
                name: "Bad".into(),
                expected_identity: expected
            },
            BACKUP,
            BACKUP
        )
        .unwrap_err()
        .error,
        LifecycleError::DestinationConflict
    );
    assert_eq!(
        super::super::execute(
            &store,
            "bad-adopt",
            Mutation::Adopt {
                name: "Bad".into(),
                custody: backup.clone(),
                expected_identity: expected
            },
            BACKUP
        )
        .unwrap_err()
        .error,
        LifecycleError::DestinationConflict
    );
    let target_store = dir.path().join("new-store");
    let target = dir.path().join("restored.key");
    execute_hook(
        &target_store,
        "restore",
        BackupMutation::Restore {
            input: backup,
            destination: target.clone(),
            name: "Restored".into(),
            expected_identity: expected,
        },
        BACKUP,
        NEW,
        &mut |phase| {
            if phase == Cut::Catalog { Err(LifecycleError::OperationFailed) } else { Ok(()) }
        },
    )
    .unwrap_err();
    let receipt = reconcile(&target_store, "backup-op-restore", &[], NEW).unwrap();
    assert_eq!(CatalogSnapshot::load(&target_store).unwrap().list().entries.len(), 1);
    assert!(receipt.entry_id.is_some());
    assert!(target.exists());
}

#[test]
fn ready_ciphertext_with_forged_digest_still_requires_expected_identity() {
    let (dir, store, _, expected) = setup();
    let output = dir.path().join("backup.stid");
    execute_hook(&store, "tamper", export(&output, expected), SOURCE, BACKUP, &mut |phase| {
        if phase == Cut::Payload { Err(LifecycleError::OperationFailed) } else { Ok(()) }
    })
    .unwrap_err();
    let ledger = Directory::open(&store.join("artifacts")).unwrap();
    let mut record = load(&ledger, "backup-op-tamper").unwrap();
    let attacker = RootSecret::ephemeral();
    let changed = EncryptedIdentityBackup::protect_root_secret(&attacker, BACKUP).unwrap();
    fs::write(output.parent().unwrap().join(&record.stages[0].name), changed.encrypted_bytes())
        .unwrap();
    record.stages[0].digest = digest(changed.encrypted_bytes());
    record.result.sha256 = record.stages[0].digest.clone();
    persist(&ledger, &record, true).unwrap();
    assert_eq!(
        reconcile(&store, "backup-op-tamper", SOURCE, BACKUP).unwrap_err().error,
        LifecycleError::IdentityMismatch
    );
    assert!(!output.exists());
}

#[test]
fn raced_quarantine_restores_unrelated_objects_without_deleting_them() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, store, _, expected) = setup();
    let output = dir.path().join("backup.stid");
    let exported = execute(&store, "export", export(&output, expected), SOURCE, BACKUP).unwrap();
    let preserved = dir.path().join("original.stid");
    let unrelated = vec![b'x'; 2048];
    execute_hook(
        &store,
        "delete",
        BackupMutation::Delete {
            artifact_id: "backup-export".into(),
            expected_digest: exported.sha256,
        },
        &[],
        &[],
        &mut |phase| {
            if phase == Cut::BeforeQuarantine {
                fs::rename(&output, &preserved).unwrap();
                fs::write(&output, &unrelated).unwrap();
                fs::set_permissions(&output, fs::Permissions::from_mode(0o600)).unwrap();
            }
            Ok(())
        },
    )
    .unwrap_err();
    assert_eq!(fs::read(output).unwrap(), unrelated);
    assert!(preserved.exists());
}

#[test]
fn completed_staging_can_recover_after_original_custody_disappears() {
    let (dir, store, custody, expected) = setup();
    let output = dir.path().join("backup.stid");
    execute_hook(&store, "last-copy", export(&output, expected), SOURCE, BACKUP, &mut |phase| {
        if phase == Cut::Payload { Err(LifecycleError::OperationFailed) } else { Ok(()) }
    })
    .unwrap_err();
    fs::remove_file(custody).unwrap();
    reconcile(&store, "backup-op-last-copy", &[], BACKUP).unwrap();
    assert!(crate::backups::verify(&output, Some(expected), BACKUP).is_ok());
}
