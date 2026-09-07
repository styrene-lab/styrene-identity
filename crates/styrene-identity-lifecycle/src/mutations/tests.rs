use super::*;

const PROTECTION: &[u8] = b"disposable-test-protection-marker";

fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("catalog");
    let custody = dir.path().join("identity.key");
    (dir, store, custody)
}

fn creation(path: &Path) -> Mutation {
    Mutation::Create { name: "Original".into(), destination: path.into() }
}

#[test]
fn id04_through_id08_crud_retains_custody_and_rejects_stale_revisions() {
    let (_dir, store, custody) = setup();
    let created = execute(&store, "create", creation(&custody), PROTECTION).unwrap();
    assert_eq!(created.catalog_revision, 1);
    assert_eq!(created.entry_revision, Some(1));
    let original = fs::read(&custody).unwrap();
    let update = Mutation::Update {
        entry_id: created.entry_id.clone(),
        if_revision: 1,
        name: NameChange::Set("Renamed".into()),
    };
    let changed = execute(&store, "update", update.clone(), &[]).unwrap();
    assert_eq!(changed.identity_id, created.identity_id);
    assert_eq!(changed.entry_revision, Some(2));
    assert_eq!(
        execute(&store, "stale", update.clone(), &[]).unwrap_err().error,
        LifecycleError::RevisionConflict
    );
    assert_eq!(execute(&store, "update", update, &[]).unwrap().entry_revision, Some(2));
    assert_eq!(CatalogSnapshot::load(&store).unwrap().list().revision, 2);
    let selection = Mutation::Select { entry_id: created.entry_id.clone(), if_revision: 2 };
    execute(&store, "select", selection, &[]).unwrap();
    assert_eq!(
        CatalogSnapshot::load(&store).unwrap().list().preferred_entry,
        Some(created.entry_id.clone())
    );
    let forgotten = execute(
        &store,
        "forget",
        Mutation::Forget { entry_id: created.entry_id.clone(), if_revision: 2 },
        &[],
    )
    .unwrap();
    assert_eq!(forgotten.entry_revision, None);
    let inventory = CatalogSnapshot::load(&store).unwrap().list();
    assert!(inventory.entries.is_empty());
    assert!(inventory.preferred_entry.is_none());
    assert_eq!(fs::read(&custody).unwrap(), original);
    // Historical request replay returns its original outcome, never recreates an entry.
    assert_eq!(
        execute(&store, "create", creation(&custody), &[]).unwrap().entry_id,
        created.entry_id
    );
    assert!(CatalogSnapshot::load(&store).unwrap().list().entries.is_empty());
    let adopted = execute(
        &store,
        "adopt",
        Mutation::Adopt {
            name: "Adopted".into(),
            custody: custody.clone(),
            expected_identity: created.identity_id.unwrap().parse().unwrap(),
        },
        PROTECTION,
    )
    .unwrap();
    assert_eq!(adopted.effects.custody, CustodyEffect::AuthenticatedUnchanged);
    assert_eq!(fs::read(custody).unwrap(), original);
}

#[test]
fn id04_reconcile_after_each_commit_boundary_preserves_the_original_identity() {
    for phase in [Phase::JournalPrepared, Phase::CustodyCommitted, Phase::CatalogCommitted] {
        let (_dir, store, custody) = setup();
        let error =
            execute_with_hook(&store, "recover", creation(&custody), PROTECTION, &mut |current| {
                if current == phase { Err(LifecycleError::OperationFailed) } else { Ok(()) }
            })
            .unwrap_err();
        assert_eq!(error.operation_id.as_deref(), Some("op-recover"));
        let before = read_journal(&store.join("operations/op-recover.json")).unwrap();
        assert_eq!(before.state, OperationState::Prepared);
        let expected = before.result.identity_id.clone();
        let encrypted = before.encrypted_creation.clone().unwrap();
        let blocked = execute(
            &store,
            "other",
            Mutation::Create {
                name: "Other".into(),
                destination: custody.with_file_name("other.key"),
            },
            PROTECTION,
        )
        .unwrap_err();
        assert_eq!(blocked.error, LifecycleError::ReconciliationRequired);
        // Once the catalog is committed, finalizing observation needs no secret.
        let credential = if phase == Phase::CatalogCommitted { &[][..] } else { PROTECTION };
        let recovered = reconcile(&store, "op-recover", credential).unwrap();
        assert_eq!(recovered.identity_id, expected);
        assert_eq!(fs::read(&custody).unwrap(), encrypted);
        assert_eq!(show_operation(&store, "op-recover").unwrap().state, OperationState::Completed);
        assert_eq!(CatalogSnapshot::load(&store).unwrap().list().entries.len(), 1);
        assert!(
            !fs::read_to_string(store.join("operations/op-recover.json"))
                .unwrap()
                .contains(std::str::from_utf8(PROTECTION).unwrap())
        );
    }
}

#[test]
fn id05_wrong_credentials_wrong_identity_and_legacy_adoption_are_non_destructive() {
    let (_dir, store, custody) = setup();
    let root = RootSecret::ephemeral();
    let id = IdentityId::from_public_key(&identity_pubkey(&root));
    let backup = EncryptedIdentityBackup::protect_root_secret(&root, PROTECTION).unwrap();
    // The legacy encrypted-file reader remains supported through the library.
    let legacy = backup.encrypted_bytes()[5..].to_vec();
    fs::write(&custody, &legacy).unwrap();
    let adopt =
        Mutation::Adopt { name: "Legacy".into(), custody: custody.clone(), expected_identity: id };
    assert_eq!(
        execute(&store, "bad-password", adopt.clone(), b"wrong").unwrap_err().error,
        LifecycleError::AuthenticationFailed
    );
    assert_eq!(
        execute(
            &store,
            "bad-id",
            Mutation::Adopt {
                name: "Wrong".into(),
                custody: custody.clone(),
                expected_identity: IdentityId::from_bytes([0; 16]),
            },
            PROTECTION
        )
        .unwrap_err()
        .error,
        LifecycleError::IdentityMismatch
    );
    assert!(!store.join("catalog.json").exists());
    let result = execute(&store, "legacy", adopt.clone(), PROTECTION).unwrap();
    assert_eq!(result.identity_id.as_deref(), Some(id.to_string().as_str()));
    assert_eq!(
        execute(&store, "duplicate", adopt, PROTECTION).unwrap_err().error,
        LifecycleError::DestinationConflict
    );
    assert_eq!(fs::read(custody).unwrap(), legacy);
}

#[test]
fn id04_destination_conflict_and_request_reuse_do_not_replace_custody() {
    let (_dir, store, custody) = setup();
    let created = execute(&store, "one", creation(&custody), PROTECTION).unwrap();
    let original = fs::read(&custody).unwrap();
    assert_eq!(
        execute(&store, "two", creation(&custody), PROTECTION).unwrap_err().error,
        LifecycleError::DestinationConflict
    );
    assert_eq!(
        execute(
            &store,
            "one",
            Mutation::Create { name: "Changed".into(), destination: custody.clone() },
            PROTECTION
        )
        .unwrap_err()
        .error,
        LifecycleError::RequestConflict
    );
    assert_eq!(fs::read(custody).unwrap(), original);
    assert_eq!(CatalogSnapshot::load(&store).unwrap().list().entries[0].entry_id, created.entry_id);
}

#[test]
fn id04_interrupted_create_rejects_occupied_destination_without_regenerating() {
    let (_dir, store, custody) = setup();
    execute_with_hook(&store, "interrupted", creation(&custody), PROTECTION, &mut |phase| {
        if phase == Phase::JournalPrepared { Err(LifecycleError::OperationFailed) } else { Ok(()) }
    })
    .unwrap_err();
    fs::write(&custody, b"unrelated-custody").unwrap();
    assert_eq!(
        reconcile(&store, "op-interrupted", PROTECTION).unwrap_err().error,
        LifecycleError::DestinationConflict
    );
    assert_eq!(fs::read(custody).unwrap(), b"unrelated-custody");
    assert!(!store.join("catalog.json").exists());
}

#[test]
fn id06_clear_name_and_recovery_reject_external_catalog_changes() {
    let (_dir, store, custody) = setup();
    let created = execute(&store, "create", creation(&custody), PROTECTION).unwrap();
    execute(
        &store,
        "clear",
        Mutation::Update {
            entry_id: created.entry_id.clone(),
            if_revision: 1,
            name: NameChange::Clear,
        },
        &[],
    )
    .unwrap();
    assert!(CatalogSnapshot::load(&store).unwrap().list().entries[0].display_name.is_none());
    execute_with_hook(
        &store,
        "pending",
        Mutation::Update {
            entry_id: created.entry_id,
            if_revision: 2,
            name: NameChange::Set("Pending".into()),
        },
        &[],
        &mut |phase| {
            if phase == Phase::JournalPrepared {
                Err(LifecycleError::OperationFailed)
            } else {
                Ok(())
            }
        },
    )
    .unwrap_err();
    let path = store.join("catalog.json");
    let mut bytes = fs::read(&path).unwrap();
    bytes.push(b' ');
    fs::write(&path, &bytes).unwrap();
    assert_eq!(
        reconcile(&store, "op-pending", &[]).unwrap_err().error,
        LifecycleError::RevisionConflict
    );
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn id04_store_lock_serializes_writers_and_releases_on_drop() {
    let (_dir, store, custody) = setup();
    let held = Store::open(&store, true).unwrap();
    assert_eq!(
        execute(&store, "create", creation(&custody), PROTECTION).unwrap_err().error,
        LifecycleError::StoreBusy
    );
    assert!(!custody.exists());
    drop(held);
    execute(&store, "create", creation(&custody), PROTECTION).unwrap();
    use std::os::unix::fs::PermissionsExt;
    for path in [custody, store.join("catalog.json"), store.join("operations/op-create.json")] {
        assert_eq!(fs::metadata(path).unwrap().permissions().mode() & 0o777, 0o600);
    }
}

#[test]
fn id04_filesystem_failure_after_custody_leaves_a_recoverable_operation() {
    let (_dir, store, custody) = setup();
    let path = store.join("catalog.json");
    let error =
        execute_with_hook(&store, "failure", creation(&custody), PROTECTION, &mut |phase| {
            if phase == Phase::CustodyCommitted {
                fs::create_dir(&path).unwrap();
            }
            Ok(())
        })
        .unwrap_err();
    assert_eq!(error.operation_id.as_deref(), Some("op-failure"));
    assert!(custody.is_file());
    assert!(path.is_dir());
    let retained = fs::read(&custody).unwrap();
    assert_eq!(show_operation(&store, "op-failure").unwrap().state, OperationState::Prepared);
    fs::remove_dir(&path).unwrap();
    reconcile(&store, "op-failure", PROTECTION).unwrap();
    assert_eq!(fs::read(custody).unwrap(), retained);
    assert_eq!(CatalogSnapshot::load(&store).unwrap().list().entries.len(), 1);
}

#[test]
fn inconsistent_recovery_receipt_is_rejected_without_catalog_replay() {
    let (_dir, store, custody) = setup();
    execute_with_hook(&store, "pending", creation(&custody), PROTECTION, &mut |phase| {
        if phase == Phase::JournalPrepared { Err(LifecycleError::OperationFailed) } else { Ok(()) }
    })
    .unwrap_err();
    let path = store.join("operations/op-pending.json");
    let mut record: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    record["result"]["identity_id"] = serde_json::json!("00".repeat(16));
    fs::write(path, serde_json::to_vec(&record).unwrap()).unwrap();
    assert_eq!(
        reconcile(&store, "op-pending", PROTECTION).unwrap_err().error,
        LifecycleError::OperationFailed
    );
    assert!(!store.join("catalog.json").exists());
    assert!(!custody.exists());
}

#[test]
fn adversarial_recovery_rejects_parent_directory_substitution() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("store");
    let parent = dir.path().join("original");
    let attacker = dir.path().join("replacement");
    fs::create_dir(&parent).unwrap();
    fs::create_dir(&attacker).unwrap();
    let custody = parent.join("identity.key");
    execute_with_hook(&store, "redirect", creation(&custody), PROTECTION, &mut |phase| {
        if phase == Phase::JournalPrepared { Err(LifecycleError::OperationFailed) } else { Ok(()) }
    })
    .unwrap_err();
    fs::rename(&parent, dir.path().join("retained")).unwrap();
    std::os::unix::fs::symlink(&attacker, &parent).unwrap();
    assert!(reconcile(&store, "op-redirect", PROTECTION).is_err());
    assert!(!attacker.join("identity.key").exists());
}

#[test]
fn adversarial_recovery_rejects_unrelated_catalog_changes_in_journal() {
    let (_dir, store, custody) = setup();
    execute(&store, "create", creation(&custody), PROTECTION).unwrap();
    execute_with_hook(
        &store,
        "rename",
        Mutation::Update {
            entry_id: "identity-create".into(),
            if_revision: 1,
            name: NameChange::Set("Renamed".into()),
        },
        &[],
        &mut |phase| {
            if phase == Phase::JournalPrepared {
                Err(LifecycleError::OperationFailed)
            } else {
                Ok(())
            }
        },
    )
    .unwrap_err();
    let catalog = fs::read(store.join("catalog.json")).unwrap();
    let path = store.join("operations/op-rename.json");
    let mut journal: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    journal["after"]["preferred_entry"] = serde_json::json!("identity-create");
    fs::write(path, serde_json::to_vec(&journal).unwrap()).unwrap();
    assert!(reconcile(&store, "op-rename", &[]).is_err());
    assert_eq!(fs::read(store.join("catalog.json")).unwrap(), catalog);
}

#[test]
fn completed_v2_receipt_drops_recovery_ciphertext_and_catalog_history() {
    let (_dir, store, custody) = setup();
    execute(&store, "compact", creation(&custody), PROTECTION).unwrap();
    let record = read_journal(&store.join("operations/op-compact.json")).unwrap();
    assert_eq!(record.schema_version, 2);
    assert!(record.encrypted_creation.is_none());
    assert!(record.after.is_none());
    assert!(record.before_digest.is_none());
    assert!(!show_operation(&store, "op-compact").unwrap().retains_recovery_material);
    assert!(execute(&store, "compact", creation(&custody), &[]).unwrap().replayed);
}

#[test]
fn legacy_completed_records_are_read_without_destroying_retained_recovery() {
    let (_dir, store, custody) = setup();
    execute_with_hook(&store, "legacy", creation(&custody), PROTECTION, &mut |phase| {
        if phase == Phase::JournalPrepared { Err(LifecycleError::OperationFailed) } else { Ok(()) }
    })
    .unwrap_err();
    let path = store.join("operations/op-legacy.json");
    let mut legacy = read_journal(&path).unwrap();
    reconcile(&store, "op-legacy", PROTECTION).unwrap();
    legacy.schema_version = 1;
    legacy.state = OperationState::Completed;
    legacy.parent_identity = None;
    let bytes = journal_bytes(&legacy).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert!(show_operation(&store, "op-legacy").unwrap().retains_recovery_material);
    assert!(execute(&store, "legacy", creation(&custody), &[]).unwrap().replayed);
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn legacy_pending_custody_without_directory_binding_requires_migration() {
    let (_dir, store, custody) = setup();
    execute_with_hook(&store, "legacy", creation(&custody), PROTECTION, &mut |phase| {
        if phase == Phase::JournalPrepared { Err(LifecycleError::OperationFailed) } else { Ok(()) }
    })
    .unwrap_err();
    let path = store.join("operations/op-legacy.json");
    let mut legacy = read_journal(&path).unwrap();
    legacy.schema_version = 1;
    legacy.parent_identity = None;
    fs::write(path, journal_bytes(&legacy).unwrap()).unwrap();
    assert_eq!(
        reconcile(&store, "op-legacy", PROTECTION).unwrap_err().error,
        LifecycleError::LocationChanged
    );
    assert!(!custody.exists());
}

#[test]
fn writable_authority_files_and_shared_private_journals_fail_closed() {
    use std::os::unix::fs::PermissionsExt;
    let (_dir, store, custody) = setup();
    execute(&store, "create", creation(&custody), PROTECTION).unwrap();
    let catalog = store.join("catalog.json");
    fs::set_permissions(&catalog, fs::Permissions::from_mode(0o666)).unwrap();
    let request = Mutation::Update {
        entry_id: "identity-create".into(),
        if_revision: 1,
        name: NameChange::Clear,
    };
    assert_eq!(
        execute(&store, "update", request.clone(), &[]).unwrap_err().error,
        LifecycleError::UnsafeStorage
    );
    fs::set_permissions(&catalog, fs::Permissions::from_mode(0o600)).unwrap();
    let record = store.join("operations/op-create.json");
    fs::set_permissions(&record, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        execute(&store, "update", request, &[]).unwrap_err().error,
        LifecycleError::UnsafeStorage
    );
}

#[test]
fn recovery_material_is_retained_if_custody_disappears_after_catalog_commit() {
    let (_dir, store, custody) = setup();
    execute_with_hook(&store, "missing", creation(&custody), PROTECTION, &mut |phase| {
        if phase == Phase::CatalogCommitted { Err(LifecycleError::OperationFailed) } else { Ok(()) }
    })
    .unwrap_err();
    fs::remove_file(&custody).unwrap();
    assert_eq!(
        reconcile(&store, "op-missing", &[]).unwrap_err().error,
        LifecycleError::CustodyUnavailable
    );
    let journal = read_journal(&store.join("operations/op-missing.json")).unwrap();
    assert_eq!(journal.state, OperationState::Prepared);
    assert!(journal.encrypted_creation.is_some());
}
