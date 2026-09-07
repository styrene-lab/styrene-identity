//! Managed encrypted backup transactions. Ciphertext is written only after a
//! durable intent owns a named staging inode. All operations share the catalog
//! store lock; receipts are historical and contain no credentials or root bytes.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use rand_core::RngCore;
use serde::{Deserialize, Serialize};

use super::*;
use crate::safe_fs::FileIdentity;

const LIMIT: u64 = 65_536;
const MAX_STAGES: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BackupMutation {
    Export {
        entry_id: String,
        output: PathBuf,
        #[serde(with = "identity_text")]
        expected_identity: IdentityId,
    },
    Reprotect {
        input: PathBuf,
        output: PathBuf,
        #[serde(with = "identity_text")]
        expected_identity: IdentityId,
    },
    Restore {
        input: PathBuf,
        destination: PathBuf,
        name: String,
        #[serde(with = "identity_text")]
        expected_identity: IdentityId,
    },
    MigrateRecovery {
        source_operation: String,
        output: PathBuf,
        #[serde(with = "identity_text")]
        expected_identity: IdentityId,
    },
    Forget {
        artifact_id: String,
        expected_digest: String,
    },
    Delete {
        artifact_id: String,
        expected_digest: String,
    },
}

impl BackupMutation {
    pub fn command(&self) -> &'static str {
        match self {
            Self::Export { .. } => "backup.export",
            Self::Reprotect { .. } => "backup.reprotect",
            Self::Restore { .. } => "backup.restore",
            Self::MigrateRecovery { .. } => "backup.migrate-recovery",
            Self::Forget { .. } => "backup.forget",
            Self::Delete { .. } => "backup.delete",
        }
    }
    fn expected(&self) -> Option<IdentityId> {
        match self {
            Self::Export { expected_identity, .. }
            | Self::Reprotect { expected_identity, .. }
            | Self::Restore { expected_identity, .. }
            | Self::MigrateRecovery { expected_identity, .. } => Some(*expected_identity),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupPhase {
    Intent,
    Staging,
    Installed,
    Quarantined,
    Completed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackupResult {
    pub operation_id: String,
    pub artifact_id: Option<String>,
    pub identity_id: String,
    pub sha256: String,
    pub encrypted_size: u64,
    pub entry_id: Option<String>,
    pub effect: String,
    pub replayed: bool,
}

#[derive(Debug)]
pub struct BackupFailure {
    pub error: LifecycleError,
    pub operation_id: Option<String>,
    pub phase: Option<BackupPhase>,
}

impl From<LifecycleError> for BackupFailure {
    fn from(error: LifecycleError) -> Self {
        Self { error, operation_id: None, phase: None }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct BackupOperationView {
    pub operation_id: String,
    pub command: String,
    pub phase: BackupPhase,
    pub result: Option<BackupResult>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ManagedBackup {
    pub artifact_id: String,
    pub identity_id: String,
    pub sha256: String,
    pub encrypted_size: u64,
    /// Administrative local metadata, never part of the public overview client.
    pub location: PathBuf,
    pub status: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stage {
    name: String,
    object: Option<FileIdentity>,
    digest: String,
}

// No Debug: private paths and complete request history stay out of generic logs.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema_version: u32,
    id: String,
    request: BackupMutation,
    destination: PathBuf,
    directory: DirectoryIdentity,
    stages: Vec<Stage>,
    installed: Option<FileIdentity>,
    quarantine: Option<String>,
    phase: BackupPhase,
    result: BackupResult,
}

impl Record {
    fn view(&self) -> BackupOperationView {
        BackupOperationView {
            operation_id: self.id.clone(),
            command: self.request.command().into(),
            phase: self.phase,
            result: (self.phase == BackupPhase::Completed).then(|| self.result.clone()),
        }
    }
    fn creates_artifact(&self) -> bool {
        matches!(
            self.request,
            BackupMutation::Export { .. }
                | BackupMutation::Reprotect { .. }
                | BackupMutation::MigrateRecovery { .. }
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cut {
    Intent,
    Ownership,
    Payload,
    Installed,
    Catalog,
    Quarantined,
    BeforeQuarantine,
}

pub fn execute(
    store: &Path,
    request_id: &str,
    request: BackupMutation,
    source_protection: &[u8],
    destination_protection: &[u8],
) -> Result<BackupResult, BackupFailure> {
    execute_hook(store, request_id, request, source_protection, destination_protection, &mut |_| {
        Ok(())
    })
}

fn execute_hook(
    store: &Path,
    request_id: &str,
    request: BackupMutation,
    source: &[u8],
    target: &[u8],
    hook: &mut dyn FnMut(Cut) -> Result<(), LifecycleError>,
) -> Result<BackupResult, BackupFailure> {
    validate_key(request_id)?;
    let request = normalize_request(request)?;
    let id = format!("backup-op-{request_id}");
    let exists = store.join("artifacts").join(format!("{id}.json")).exists();
    if !exists && request.expected().is_some() {
        check_passphrase(source)?;
        check_passphrase(target)?;
    }
    let mut locked = Store::open(store, matches!(request, BackupMutation::Restore { .. }))?;
    locked.artifact_context = Some(id.clone());
    if let BackupMutation::MigrateRecovery { source_operation, .. } = &request {
        locked.legacy_context = Some(source_operation.clone());
    }
    if locked.journal_path(&format!("op-{request_id}")).exists() {
        return Err(LifecycleError::RequestConflict.into());
    }
    let directory = locked.directory.child("artifacts", true)?;
    let mut prepared_payload = None;
    let mut record = match load(&directory, &id) {
        Ok(record) => {
            if record.request != request {
                return Err(LifecycleError::RequestConflict.into());
            }
            if record.phase == BackupPhase::Completed {
                return Ok(replayed(record.result));
            }
            record
        }
        Err(LifecycleError::OperationNotFound) => {
            locked.check_pending("")?;
            let (record, payload) = prepare_artifact(&locked, &id, request, source, target)?;
            install_store_guard(&locked)?;
            persist(&directory, &record, false).map_err(|error| BackupFailure {
                error,
                operation_id: Some(id.clone()),
                phase: Some(BackupPhase::Intent),
            })?;
            prepared_payload = payload;
            record
        }
        Err(error) => return Err(error.into()),
    };
    finish_artifact(&mut locked, &directory, &mut record, source, target, prepared_payload, hook)
        .map_err(|error| BackupFailure { error, operation_id: Some(id), phase: Some(record.phase) })
}

pub fn reconcile(
    store: &Path,
    operation_id: &str,
    source: &[u8],
    target: &[u8],
) -> Result<BackupResult, BackupFailure> {
    let key = operation_key(operation_id)?;
    let directory = Directory::open(&store.join("artifacts"))?;
    let record = load(&directory, operation_id)?;
    execute(store, key, record.request, source, target)
}

pub fn show_operation(
    store: &Path,
    operation_id: &str,
) -> Result<BackupOperationView, LifecycleError> {
    operation_key(operation_id)?;
    let directory = Directory::open(&store.join("artifacts"))?;
    Ok(load(&directory, operation_id)?.view())
}

pub(super) fn operation_views(store: &Path) -> Result<Vec<BackupOperationView>, LifecycleError> {
    let root = Directory::open(store)?;
    match root.child("artifacts", false) {
        Ok(directory) => Ok(records(&directory)?.into_iter().map(|record| record.view()).collect()),
        Err(LifecycleError::OperationNotFound) => Ok(vec![]),
        Err(error) => Err(error),
    }
}

pub fn verify_recovery(
    store: &Path,
    operation_id: &str,
    expected: Option<IdentityId>,
    protection: &[u8],
) -> Result<crate::backups::VerifiedBackup, LifecycleError> {
    validate_operation_id(operation_id)?;
    let record = read_journal(&store.join("operations").join(format!("{operation_id}.json")))?;
    let backup = EncryptedIdentityBackup::from_encrypted_bytes(
        record.encrypted_creation.ok_or(LifecycleError::CustodyUnavailable)?,
    )
    .map_err(|_| LifecycleError::InvalidBackup)?;
    let verified = crate::backups::verify_artifact(backup, expected, protection)?;
    if record.result.identity_id.as_deref() != Some(&verified.identity_id) {
        return Err(LifecycleError::IdentityMismatch);
    }
    Ok(verified)
}

pub fn list(
    store: &Path,
    identity: Option<IdentityId>,
) -> Result<Vec<ManagedBackup>, LifecycleError> {
    let root = Directory::open(store)?;
    let directory = match root.child("artifacts", false) {
        Ok(directory) => directory,
        Err(LifecycleError::OperationNotFound) => return Ok(vec![]),
        Err(error) => return Err(error),
    };
    let records = records(&directory)?;
    let mut result = Vec::new();
    for record in &records {
        if !record.creates_artifact() || record.phase != BackupPhase::Completed {
            continue;
        }
        if identity.is_some_and(|id| record.result.identity_id != id.to_string()) {
            continue;
        }
        let artifact = record.result.artifact_id.as_ref().ok_or(LifecycleError::OperationFailed)?;
        if retired(&records, artifact).is_some() {
            continue;
        }
        result.push(managed(record, "active"));
    }
    result.sort_by(|a, b| a.artifact_id.cmp(&b.artifact_id));
    Ok(result)
}

pub fn show(store: &Path, artifact_id: &str) -> Result<ManagedBackup, LifecycleError> {
    let directory = Directory::open(&store.join("artifacts"))?;
    let all = records(&directory)?;
    let record = original(&all, artifact_id)?;
    let status = retired(&all, artifact_id).unwrap_or("active");
    let mut view = managed(record, status);
    if status == "active" {
        view.status =
            match bound_parent(record).and_then(|parent| snapshot_target(&parent, record)) {
                Ok((object, bytes))
                    if Some(object) == record.installed
                        && digest(&bytes) == record.result.sha256 =>
                {
                    "present_matching"
                }
                Err(LifecycleError::OperationNotFound) => "missing",
                Ok(_) => "changed",
                Err(_) => "unavailable",
            }
            .into();
    }
    Ok(view)
}

fn managed(record: &Record, status: &str) -> ManagedBackup {
    ManagedBackup {
        artifact_id: record.result.artifact_id.clone().unwrap_or_default(),
        identity_id: record.result.identity_id.clone(),
        sha256: record.result.sha256.clone(),
        encrypted_size: record.result.encrypted_size,
        location: record.destination.clone(),
        status: status.into(),
    }
}

fn prepare_artifact(
    store: &Store,
    id: &str,
    request: BackupMutation,
    source: &[u8],
    target: &[u8],
) -> Result<(Record, Option<Vec<u8>>), LifecycleError> {
    let key = operation_key(id)?;
    if let BackupMutation::Forget { artifact_id, expected_digest }
    | BackupMutation::Delete { artifact_id, expected_digest } = &request
    {
        let directory = store.directory.child("artifacts", false)?;
        let all = records(&directory)?;
        let origin = original(&all, artifact_id)?;
        if retired(&all, artifact_id).is_some() {
            return Err(LifecycleError::ArtifactNotFound);
        }
        if origin.result.sha256 != *expected_digest {
            return Err(LifecycleError::DestinationConflict);
        }
        if matches!(request, BackupMutation::Delete { .. }) {
            ensure_not_custody(store, &origin.destination)?;
            let parent = bound_parent(origin)?;
            match snapshot_target(&parent, origin) {
                Ok((object, bytes))
                    if Some(object) == origin.installed && digest(&bytes) == *expected_digest => {}
                Err(LifecycleError::OperationNotFound) => {}
                Ok(_) => return Err(LifecycleError::DestinationConflict),
                Err(error) => return Err(error),
            }
        }
        let deleting = matches!(request, BackupMutation::Delete { .. });
        let artifact_id = artifact_id.clone();
        return Ok((
            Record {
                schema_version: 1,
                id: id.into(),
                request,
                destination: origin.destination.clone(),
                directory: origin.directory,
                stages: vec![],
                installed: origin.installed,
                quarantine: deleting.then(|| stage_name(key, 0)),
                phase: BackupPhase::Intent,
                result: BackupResult {
                    operation_id: id.into(),
                    artifact_id: Some(artifact_id),
                    identity_id: origin.result.identity_id.clone(),
                    sha256: origin.result.sha256.clone(),
                    encrypted_size: origin.result.encrypted_size,
                    entry_id: None,
                    effect: if deleting { "deleted" } else { "forgotten" }.into(),
                    replayed: false,
                },
            },
            None,
        ));
    }
    let expected = request.expected().ok_or(LifecycleError::InvalidRequest)?;
    let destination = output(&request).ok_or(LifecycleError::InvalidRequest)?.to_owned();
    if destination.starts_with(&store.root) {
        return Err(LifecycleError::InvalidRequest);
    }
    let parent = Directory::open(destination.parent().ok_or(LifecycleError::InvalidRequest)?)?;
    parent.require_owner(false)?;
    let mut installed = None;
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(LifecycleError::InvalidRequest)?;
    let existing = match parent.snapshot_owned(name, 97) {
        Ok(snapshot) if matches!(request, BackupMutation::Restore { .. }) => Some(snapshot),
        Err(LifecycleError::OperationNotFound) => None,
        _ if !matches!(request, BackupMutation::Restore { .. }) => {
            return Err(LifecycleError::DestinationConflict);
        }
        Ok(_) => return Err(LifecycleError::DestinationConflict),
        Err(error) => return Err(error),
    };
    let root = source_root(store, &request, source)?;
    check_identity(&root, expected)?;
    check_passphrase(target)?;
    let mut payload = EncryptedIdentityBackup::protect_root_secret(&root, target)
        .map_err(|_| LifecycleError::OperationFailed)?
        .encrypted_bytes()
        .to_vec();
    if let Some((object, bytes)) = existing {
        check_identity(&decrypt(&bytes, target)?, expected)?;
        payload = bytes;
        installed = Some(object);
    }
    let entry_id = if matches!(request, BackupMutation::Restore { .. }) {
        existing_restore_entry(store, expected, &destination)?
    } else {
        None
    };
    let is_restore = matches!(request, BackupMutation::Restore { .. });
    if is_restore && entry_id.is_none() && store.journal_path(&format!("op-restore-{key}")).exists()
    {
        return Err(LifecycleError::RequestConflict);
    }
    let record = Record {
        schema_version: 1,
        id: id.into(),
        request,
        destination,
        directory: parent.identity()?,
        stages: if installed.is_some() {
            vec![]
        } else {
            vec![Stage { name: stage_name(key, 0), object: None, digest: digest(&payload) }]
        },
        installed,
        quarantine: None,
        phase: if installed.is_some() { BackupPhase::Installed } else { BackupPhase::Intent },
        result: BackupResult {
            operation_id: id.into(),
            artifact_id: (!is_restore).then(|| format!("backup-{key}")),
            identity_id: expected.to_string(),
            sha256: digest(&payload),
            encrypted_size: payload.len() as u64,
            entry_id,
            effect: if is_restore { "restored" } else { "exported" }.into(),
            replayed: false,
        },
    };
    Ok((record, Some(payload)))
}

fn finish_artifact(
    store: &mut Store,
    ledger: &Directory,
    record: &mut Record,
    source: &[u8],
    target: &[u8],
    mut payload: Option<Vec<u8>>,
    hook: &mut dyn FnMut(Cut) -> Result<(), LifecycleError>,
) -> Result<BackupResult, LifecycleError> {
    store.check_pending("")?;
    hook(Cut::Intent)?;
    if let BackupMutation::Forget { artifact_id, .. } | BackupMutation::Delete { artifact_id, .. } =
        &record.request
    {
        let all = records(ledger)?;
        let origin = original(&all, artifact_id)?;
        if origin.destination != record.destination
            || origin.directory != record.directory
            || origin.installed != record.installed
            || origin.result.sha256 != record.result.sha256
            || origin.result.identity_id != record.result.identity_id
        {
            return Err(LifecycleError::OperationFailed);
        }
    }
    if matches!(record.request, BackupMutation::Forget { .. }) {
        record.phase = BackupPhase::Completed;
        persist(ledger, record, true)?;
        return Ok(record.result.clone());
    }
    let parent = bound_parent(record)?;
    if matches!(record.request, BackupMutation::Delete { .. }) {
        return finish_delete(ledger, &parent, record, hook);
    }
    let target_name = file_name(&record.destination)?.to_owned();
    let mut authenticated_output = false;
    if record.phase == BackupPhase::Installed
        && matches!(parent.snapshot_owned(&target_name, 97), Err(LifecycleError::OperationNotFound))
    {
        let cipher = regenerated_payload(store, record, source, target)?;
        append_stage(record, &cipher)?;
        record.phase = BackupPhase::Intent;
        record.installed = None;
        payload = Some(cipher);
        persist(ledger, record, true)?;
    }
    if record.phase != BackupPhase::Installed {
        // A no-replace rename may have completed before its receipt was saved.
        if let Ok((object, bytes)) = parent.snapshot_owned(&target_name, 97) {
            if record
                .stages
                .iter()
                .any(|stage| stage.object == Some(object) && stage.digest == digest(&bytes))
            {
                record.installed = Some(object);
                record.result.sha256 = digest(&bytes);
                record.result.encrypted_size = bytes.len() as u64;
                record.phase = BackupPhase::Installed;
                persist(ledger, record, true)?;
            } else {
                return Err(LifecycleError::DestinationConflict);
            }
        }
    }
    while record.phase != BackupPhase::Installed {
        let index = record.stages.len().checked_sub(1).ok_or(LifecycleError::OperationFailed)?;
        if record.stages[index].object.is_none() {
            let object = parent.reserve_empty(&record.stages[index].name)?;
            record.stages[index].object = Some(object);
            record.phase = BackupPhase::Staging;
            persist(ledger, record, true)?; // ownership durable BEFORE ciphertext
            hook(Cut::Ownership)?;
        }
        let stage = record.stages[index].clone();
        let (object, bytes) = match parent.snapshot_owned(&stage.name, 97) {
            Ok(snapshot) => snapshot,
            Err(LifecycleError::OperationNotFound) => {
                let cipher = regenerated_payload(store, record, source, target)?;
                append_stage(record, &cipher)?;
                payload = Some(cipher);
                persist(ledger, record, true)?;
                continue;
            }
            Err(error) => return Err(error),
        };
        if Some(object) != stage.object {
            return Err(LifecycleError::DestinationConflict);
        }
        if !bytes.is_empty() && digest(&bytes) == stage.digest {
            verify_output(record, &bytes, target)?;
            authenticated_output = true;
            parent.rename_exclusive(&stage.name, &target_name)?;
            record.installed = Some(object);
            record.result.sha256 = stage.digest;
            record.result.encrypted_size = bytes.len() as u64;
            record.phase = BackupPhase::Installed;
            persist(ledger, record, true)?;
            break;
        }
        if payload.is_none() {
            payload = Some(regenerated_payload(store, record, source, target)?);
        }
        let cipher = payload.take().ok_or(LifecycleError::OperationFailed)?;
        if !bytes.is_empty() {
            if record.stages.len() >= MAX_STAGES {
                return Err(LifecycleError::ReconciliationRequired);
            }
            // Never overwrite a possible recovery copy, even when its digest is
            // unexpected. Allocate a new tracked generation and clean old owned
            // staging only after the final artifact is verified in place.
            append_stage(record, &cipher)?;
            payload = Some(cipher);
            persist(ledger, record, true)?;
            continue;
        }
        record.stages[index].digest = digest(&cipher);
        record.result.sha256 = digest(&cipher);
        persist(ledger, record, true)?;
        parent.write_empty_owned(&stage.name, object, &cipher)?;
        hook(Cut::Payload)?;
        payload = None;
    }
    hook(Cut::Installed)?;
    verify_installed(&parent, record)?;
    if !authenticated_output {
        let (_, bytes) = snapshot_target(&parent, record)?;
        verify_output(record, &bytes, target)?;
    }
    if let BackupMutation::Restore { name, expected_identity, .. } = &record.request {
        if record.result.entry_id.is_none() {
            if let Some(entry) =
                existing_restore_entry(store, *expected_identity, &record.destination)?
            {
                record.result.entry_id = Some(entry);
            } else {
                check_passphrase(target)?;
                let child_key = format!("restore-{}", operation_key(&record.id)?);
                let mutation = Mutation::Adopt {
                    name: name.clone(),
                    custody: record.destination.clone(),
                    expected_identity: *expected_identity,
                };
                let child = execute_locked(
                    store,
                    &child_key,
                    format!("op-{child_key}"),
                    mutation,
                    target,
                    &mut |_| Ok(()),
                )
                .map_err(|failure| failure.error)?;
                record.result.entry_id = Some(child.entry_id);
            }
            persist(ledger, record, true)?;
        }
        hook(Cut::Catalog)?;
    }
    if let BackupMutation::MigrateRecovery { source_operation, .. } = &record.request {
        let mut old = store.read_journal(source_operation)?;
        if old.result.identity_id.as_deref() != Some(&record.result.identity_id) {
            return Err(LifecycleError::IdentityMismatch);
        }
        verify_installed(&parent, record)?;
        let catalog_committed =
            old.after.as_ref().map(validate_catalog).transpose()?.is_some_and(|bytes| {
                store
                    .catalog()
                    .is_ok_and(|(_, current)| current.as_deref() == Some(&digest(&bytes)))
            });
        old.state = if old.state == OperationState::Completed || catalog_committed {
            OperationState::Completed
        } else {
            OperationState::Superseded
        };
        old.schema_version = 3;
        old.encrypted_creation = None;
        old.after = None;
        old.before_digest = None;
        old.recovery_migrated_to = record.result.artifact_id.clone();
        store.write_journal(&old, true)?;
    }
    // Recheck the destination before every cleanup: it may now be the last copy.
    for stage in &record.stages {
        if let Some(object) = stage.object {
            verify_installed(&parent, record)?;
            match parent.snapshot_owned(&stage.name, 97) {
                Ok((actual, _)) if actual == object => parent.remove_owned(&stage.name, object)?,
                Err(LifecycleError::OperationNotFound) => {}
                Ok(_) => return Err(LifecycleError::DestinationConflict),
                Err(error) => return Err(error),
            }
        }
    }
    record.stages.clear();
    record.phase = BackupPhase::Completed;
    persist(ledger, record, true)?;
    Ok(record.result.clone())
}

fn finish_delete(
    ledger: &Directory,
    parent: &Directory,
    record: &mut Record,
    hook: &mut dyn FnMut(Cut) -> Result<(), LifecycleError>,
) -> Result<BackupResult, LifecycleError> {
    let target = file_name(&record.destination)?;
    let quarantine = record.quarantine.as_deref().ok_or(LifecycleError::OperationFailed)?;
    let expected = record.installed.ok_or(LifecycleError::OperationFailed)?;
    match parent.snapshot_expected(quarantine, expected, 97) {
        Ok((object, bytes)) if object == expected && digest(&bytes) == record.result.sha256 => {}
        Ok(_) => {
            // Restore a raced-in unrelated object if its old name is still free.
            // Otherwise retain it in quarantine and report the unresolved conflict.
            let _ = parent.rename_exclusive(quarantine, target);
            return Err(LifecycleError::DestinationConflict);
        }
        Err(LifecycleError::OperationNotFound) if record.phase == BackupPhase::Quarantined => {
            record.phase = BackupPhase::Completed;
            persist(ledger, record, true)?;
            return Ok(record.result.clone());
        }
        Err(LifecycleError::OperationNotFound) => {
            let (object, bytes) = match parent.snapshot_expected(target, expected, 97) {
                Ok(snapshot) => snapshot,
                Err(LifecycleError::OperationNotFound) => {
                    record.result.effect = "already_absent".into();
                    record.phase = BackupPhase::Completed;
                    persist(ledger, record, true)?;
                    return Ok(record.result.clone());
                }
                Err(error) => return Err(error),
            };
            if object != expected || digest(&bytes) != record.result.sha256 {
                return Err(LifecycleError::DestinationConflict);
            }
            hook(Cut::BeforeQuarantine)?;
            parent.rename_exclusive(target, quarantine)?;
            let (object, bytes) = match parent.snapshot_expected(quarantine, expected, 97) {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    let _ = parent.rename_exclusive(quarantine, target);
                    return Err(error);
                }
            };
            if object != expected || digest(&bytes) != record.result.sha256 {
                let _ = parent.rename_exclusive(quarantine, target);
                return Err(LifecycleError::DestinationConflict);
            }
        }
        Err(error) => {
            let _ = parent.rename_exclusive(quarantine, target);
            return Err(error);
        }
    }
    record.phase = BackupPhase::Quarantined;
    persist(ledger, record, true)?;
    hook(Cut::Quarantined)?;
    parent.remove_owned(quarantine, expected)?;
    record.phase = BackupPhase::Completed;
    persist(ledger, record, true)?;
    Ok(record.result.clone())
}

fn source_root(
    store: &Store,
    request: &BackupMutation,
    protection: &[u8],
) -> Result<RootSecret, LifecycleError> {
    check_passphrase(protection)?;
    let expected = request.expected().ok_or(LifecycleError::InvalidRequest)?;
    let bytes = match request {
        BackupMutation::Export { entry_id, .. } => {
            read_custody(&custody_path(store, entry_id, expected)?)?
        }
        BackupMutation::Reprotect { input, .. } | BackupMutation::Restore { input, .. } => {
            safe_fs::read(input, 97, false)?
        }
        BackupMutation::MigrateRecovery { source_operation, .. } => {
            let record = store.read_journal(source_operation)?;
            if record.result.identity_id.as_deref() != Some(&expected.to_string()) {
                return Err(LifecycleError::IdentityMismatch);
            }
            record.encrypted_creation.ok_or(LifecycleError::CustodyUnavailable)?
        }
        _ => return Err(LifecycleError::InvalidRequest),
    };
    let root = decrypt(&bytes, protection)?;
    check_identity(&root, expected)?;
    Ok(root)
}

fn custody_path(
    store: &Store,
    entry_id: &str,
    expected: IdentityId,
) -> Result<PathBuf, LifecycleError> {
    let snapshot = CatalogSnapshot::load(&store.root)?;
    let entry = snapshot.show(entry_id, Some(expected))?;
    if entry.entry_id != entry_id || entry.custody_refs.len() != 1 {
        return Err(LifecycleError::CustodyUnavailable);
    }
    let key =
        entry.custody_refs[0].strip_prefix("custody-").ok_or(LifecycleError::CustodyUnavailable)?;
    let record = store.read_journal(&format!("op-{key}"))?;
    if record.state != OperationState::Completed
        || record.result.entry_id != entry_id
        || record.result.identity_id.as_deref() != Some(&expected.to_string())
    {
        return Err(LifecycleError::CustodyUnavailable);
    }
    match record.mutation {
        Mutation::Create { destination, .. } => Ok(destination),
        Mutation::Adopt { custody, .. } => Ok(custody),
        _ => Err(LifecycleError::CustodyUnavailable),
    }
}

fn existing_restore_entry(
    store: &Store,
    expected: IdentityId,
    destination: &Path,
) -> Result<Option<String>, LifecycleError> {
    let (catalog, _) = store.catalog()?;
    let Some(entry) = catalog.entries.iter().find(|entry| {
        entry
            .public_identity
            .as_ref()
            .is_some_and(|public| public.identity_id == expected.to_string())
    }) else {
        return Ok(None);
    };
    if custody_path(store, &entry.entry_id, expected)? != destination {
        return Err(LifecycleError::DestinationConflict);
    }
    Ok(Some(entry.entry_id.clone()))
}

fn check_identity(root: &RootSecret, expected: IdentityId) -> Result<(), LifecycleError> {
    if !expected.matches_public_key(&identity_pubkey(root)) {
        return Err(LifecycleError::IdentityMismatch);
    }
    Ok(())
}

fn bound_parent(record: &Record) -> Result<Directory, LifecycleError> {
    let parent =
        Directory::open(record.destination.parent().ok_or(LifecycleError::InvalidRequest)?)?;
    parent.require_owner(false)?;
    if parent.identity()? != record.directory {
        return Err(LifecycleError::LocationChanged);
    }
    Ok(parent)
}

fn snapshot_target(
    parent: &Directory,
    record: &Record,
) -> Result<(FileIdentity, Vec<u8>), LifecycleError> {
    if let Some(object) = record.installed {
        parent.snapshot_expected(file_name(&record.destination)?, object, 97)
    } else {
        parent.snapshot_owned(file_name(&record.destination)?, 97)
    }
}

fn verify_installed(parent: &Directory, record: &Record) -> Result<(), LifecycleError> {
    let (object, bytes) = snapshot_target(parent, record)?;
    if Some(object) != record.installed || digest(&bytes) != record.result.sha256 {
        return Err(LifecycleError::DestinationConflict);
    }
    Ok(())
}

fn verify_output(record: &Record, bytes: &[u8], protection: &[u8]) -> Result<(), LifecycleError> {
    let backup = EncryptedIdentityBackup::from_encrypted_bytes(bytes.to_vec())
        .map_err(|_| LifecycleError::InvalidBackup)?;
    crate::backups::verify_artifact(backup, record.request.expected(), protection)?;
    Ok(())
}

fn regenerated_payload(
    store: &Store,
    record: &Record,
    source: &[u8],
    target: &[u8],
) -> Result<Vec<u8>, LifecycleError> {
    let root = source_root(store, &record.request, source)?;
    check_passphrase(target)?;
    Ok(EncryptedIdentityBackup::protect_root_secret(&root, target)
        .map_err(|_| LifecycleError::OperationFailed)?
        .encrypted_bytes()
        .to_vec())
}

fn append_stage(record: &mut Record, cipher: &[u8]) -> Result<(), LifecycleError> {
    if record.stages.len() >= MAX_STAGES {
        return Err(LifecycleError::ReconciliationRequired);
    }
    record.stages.push(Stage {
        name: stage_name(operation_key(&record.id)?, record.stages.len()),
        object: None,
        digest: digest(cipher),
    });
    record.result.sha256 = digest(cipher);
    record.result.encrypted_size = cipher.len() as u64;
    Ok(())
}

fn normalize_request(mut request: BackupMutation) -> Result<BackupMutation, LifecycleError> {
    match &mut request {
        BackupMutation::Export { entry_id, output, .. } => {
            if !valid_id(entry_id) {
                return Err(LifecycleError::InvalidRequest);
            }
            normalize_path(output)?;
        }
        BackupMutation::Reprotect { input, output, .. } => {
            normalize_path(input)?;
            normalize_path(output)?;
            if input == output {
                return Err(LifecycleError::DestinationConflict);
            }
        }
        BackupMutation::Restore { input, destination, name, .. } => {
            normalize_path(input)?;
            normalize_path(destination)?;
            if input == destination {
                return Err(LifecycleError::DestinationConflict);
            }
            if name.trim().is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
                return Err(LifecycleError::InvalidRequest);
            }
        }
        BackupMutation::MigrateRecovery { source_operation, output, .. } => {
            validate_operation_id(source_operation)?;
            normalize_path(output)?;
        }
        BackupMutation::Forget { artifact_id, expected_digest }
        | BackupMutation::Delete { artifact_id, expected_digest } => {
            artifact_key(artifact_id)?;
            if !valid_digest(expected_digest) {
                return Err(LifecycleError::InvalidRequest);
            }
        }
    }
    Ok(request)
}

fn normalize_path(path: &mut PathBuf) -> Result<(), LifecycleError> {
    let name = path.file_name().ok_or(LifecycleError::InvalidRequest)?.to_owned();
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    *path = parent.canonicalize().map_err(|_| LifecycleError::CustodyUnavailable)?.join(name);
    Ok(())
}

fn output(request: &BackupMutation) -> Option<&Path> {
    match request {
        BackupMutation::Export { output, .. }
        | BackupMutation::Reprotect { output, .. }
        | BackupMutation::MigrateRecovery { output, .. } => Some(output),
        BackupMutation::Restore { destination, .. } => Some(destination),
        _ => None,
    }
}

fn file_name(path: &Path) -> Result<&str, LifecycleError> {
    path.file_name().and_then(|n| n.to_str()).ok_or(LifecycleError::InvalidRequest)
}
fn validate_key(key: &str) -> Result<(), LifecycleError> {
    if !valid_id(key) || key.len() > 32 { Err(LifecycleError::InvalidRequest) } else { Ok(()) }
}
fn operation_key(id: &str) -> Result<&str, LifecycleError> {
    let key = id.strip_prefix("backup-op-").ok_or(LifecycleError::InvalidRequest)?;
    validate_key(key)?;
    Ok(key)
}
fn artifact_key(id: &str) -> Result<&str, LifecycleError> {
    let key = id.strip_prefix("backup-").ok_or(LifecycleError::InvalidRequest)?;
    validate_key(key)?;
    Ok(key)
}
fn stage_name(key: &str, generation: usize) -> String {
    let mut nonce = [0u8; 16];
    rand_core::OsRng.fill_bytes(&mut nonce);
    format!(".idctl-artifact-{key}-{generation}-{}", hex::encode(nonce))
}
fn replayed(mut result: BackupResult) -> BackupResult {
    result.replayed = true;
    result
}

fn persist(directory: &Directory, record: &Record, replace: bool) -> Result<(), LifecycleError> {
    let bytes = serde_json::to_vec(record).map_err(|_| LifecycleError::OperationFailed)?;
    if bytes.len() as u64 > LIMIT {
        return Err(LifecycleError::OperationFailed);
    }
    directory.write(OsStr::new(&format!("{}.json", record.id)), &bytes, replace)
}

fn load(directory: &Directory, id: &str) -> Result<Record, LifecycleError> {
    operation_key(id)?;
    let bytes = directory.read(OsStr::new(&format!("{id}.json")), LIMIT, true)?;
    let record: Record =
        serde_json::from_slice(&bytes).map_err(|_| LifecycleError::OperationFailed)?;
    if record.schema_version != 1 {
        return Err(LifecycleError::UnsupportedSchema);
    }
    if record.id != id
        || record.result.operation_id != id
        || record.result.replayed
        || !record.destination.is_absolute()
        || record.stages.len() > MAX_STAGES
        || !valid_digest(&record.result.sha256)
        || record.result.identity_id.parse::<IdentityId>().is_err()
    {
        return Err(LifecycleError::OperationFailed);
    }
    for stage in &record.stages {
        if !stage.name.starts_with(&format!(".idctl-artifact-{}-", operation_key(id)?))
            || stage.name.contains('/')
            || !valid_digest(&stage.digest)
        {
            return Err(LifecycleError::OperationFailed);
        }
    }
    if record
        .quarantine
        .as_ref()
        .is_some_and(|name| !name.starts_with(".idctl-artifact-") || name.contains('/'))
    {
        return Err(LifecycleError::OperationFailed);
    }
    if record
        .request
        .expected()
        .is_some_and(|expected| record.result.identity_id != expected.to_string())
    {
        return Err(LifecycleError::IdentityMismatch);
    }
    match &record.request {
        BackupMutation::Forget { artifact_id, expected_digest }
        | BackupMutation::Delete { artifact_id, expected_digest } => {
            if record.result.artifact_id.as_ref() != Some(artifact_id)
                || record.result.sha256 != *expected_digest
                || !record.stages.is_empty()
            {
                return Err(LifecycleError::OperationFailed);
            }
            if record.result.entry_id.is_some()
                || (matches!(record.request, BackupMutation::Forget { .. })
                    && record.result.effect != "forgotten")
                || (matches!(record.request, BackupMutation::Delete { .. })
                    && !matches!(record.result.effect.as_str(), "deleted" | "already_absent"))
            {
                return Err(LifecycleError::OperationFailed);
            }
        }
        _ => {
            if output(&record.request) != Some(record.destination.as_path())
                || !matches!(record.result.encrypted_size, 92 | 97)
            {
                return Err(LifecycleError::OperationFailed);
            }
            let expected_artifact = if matches!(record.request, BackupMutation::Restore { .. }) {
                None
            } else {
                Some(format!("backup-{}", operation_key(id)?))
            };
            if record.result.artifact_id != expected_artifact {
                return Err(LifecycleError::OperationFailed);
            }
            if matches!(record.phase, BackupPhase::Installed | BackupPhase::Completed)
                && record.installed.is_none()
            {
                return Err(LifecycleError::OperationFailed);
            }
            if record.phase == BackupPhase::Completed && !record.stages.is_empty() {
                return Err(LifecycleError::OperationFailed);
            }
            if matches!(record.request, BackupMutation::Restore { .. }) {
                if record.result.effect != "restored"
                    || (record.phase == BackupPhase::Completed && record.result.entry_id.is_none())
                {
                    return Err(LifecycleError::OperationFailed);
                }
            } else if record.result.effect != "exported" || record.result.entry_id.is_some() {
                return Err(LifecycleError::OperationFailed);
            }
        }
    }
    Ok(record)
}

fn records(directory: &Directory) -> Result<Vec<Record>, LifecycleError> {
    let mut result = Vec::new();
    for name in directory.names()? {
        if let Some(id) = name.strip_suffix(".json") {
            if result.len() >= MAX_OPERATIONS {
                return Err(LifecycleError::OperationFailed);
            }
            result.push(load(directory, id)?);
        }
    }
    Ok(result)
}

fn original<'a>(records: &'a [Record], id: &str) -> Result<&'a Record, LifecycleError> {
    artifact_key(id)?;
    records
        .iter()
        .find(|record| {
            record.creates_artifact()
                && record.phase == BackupPhase::Completed
                && record.result.artifact_id.as_deref() == Some(id)
        })
        .ok_or(LifecycleError::ArtifactNotFound)
}

fn retired<'a>(records: &'a [Record], id: &str) -> Option<&'a str> {
    records.iter().find_map(|record| {
        if record.phase == BackupPhase::Completed {
            match &record.request {
                BackupMutation::Delete { artifact_id, .. } if artifact_id == id => Some("deleted"),
                BackupMutation::Forget { artifact_id, .. } if artifact_id == id => {
                    Some("forgotten")
                }
                _ => None,
            }
        } else {
            None
        }
    })
}

pub(super) fn request_exists(store: &Store, key: &str) -> Result<bool, LifecycleError> {
    let directory = match store.directory.child("artifacts", false) {
        Ok(directory) => directory,
        Err(LifecycleError::OperationNotFound) => return Ok(false),
        Err(error) => return Err(error),
    };
    match directory.read(OsStr::new(&format!("backup-op-{key}.json")), LIMIT, true) {
        Ok(_) => Ok(true),
        Err(LifecycleError::OperationNotFound) => Ok(false),
        Err(error) => Err(error),
    }
}

pub(super) fn check_pending(store: &Store, allowed: Option<&str>) -> Result<(), LifecycleError> {
    let directory = match store.directory.child("artifacts", false) {
        Ok(directory) => directory,
        Err(LifecycleError::OperationNotFound) => return Ok(()),
        Err(error) => return Err(error),
    };
    let records = records(&directory)?;
    if records.len() >= MAX_OPERATIONS
        && allowed.is_none_or(|id| !records.iter().any(|r| r.id == id))
    {
        return Err(LifecycleError::OperationFailed);
    }
    if records
        .iter()
        .any(|record| record.phase != BackupPhase::Completed && allowed != Some(record.id.as_str()))
    {
        return Err(LifecycleError::ReconciliationRequired);
    }
    Ok(())
}

const STORE_GUARD: &[u8] = b"{\"schema_version\":3,\"kind\":\"artifact-capable-store\"}";

pub(super) fn validate_store_guard(store: &Store) -> Result<(), LifecycleError> {
    if store.operations.read(OsStr::new(".artifact-store-v3.json"), 256, true)? != STORE_GUARD {
        return Err(LifecycleError::UnsupportedSchema);
    }
    Ok(())
}

fn install_store_guard(store: &Store) -> Result<(), LifecycleError> {
    match store.operations.read(OsStr::new(".artifact-store-v3.json"), 256, true) {
        Ok(bytes) if bytes == STORE_GUARD => Ok(()),
        Ok(_) => Err(LifecycleError::UnsupportedSchema),
        Err(LifecycleError::OperationNotFound) => {
            store.operations.write(OsStr::new(".artifact-store-v3.json"), STORE_GUARD, false)
        }
        Err(error) => Err(error),
    }
}

pub(super) fn ensure_not_managed_backup(store: &Store, path: &Path) -> Result<(), LifecycleError> {
    let directory = match store.directory.child("artifacts", false) {
        Ok(directory) => directory,
        Err(LifecycleError::OperationNotFound) => return Ok(()),
        Err(error) => return Err(error),
    };
    let all = records(&directory)?;
    if all.iter().any(|record| {
        record.creates_artifact()
            && record.phase == BackupPhase::Completed
            && record.destination == path
            && record.result.artifact_id.as_ref().is_some_and(|id| retired(&all, id).is_none())
    }) {
        return Err(LifecycleError::DestinationConflict);
    }
    Ok(())
}

fn ensure_not_custody(store: &Store, path: &Path) -> Result<(), LifecycleError> {
    let (catalog, _) = store.catalog()?;
    for entry in catalog.entries {
        for custody in entry.custody_refs {
            let Some(key) = custody.strip_prefix("custody-") else { continue };
            let record = store.read_journal(&format!("op-{key}"))?;
            let location = match record.mutation {
                Mutation::Create { destination, .. } => destination,
                Mutation::Adopt { custody, .. } => custody,
                _ => continue,
            };
            if location == path {
                return Err(LifecycleError::DestinationConflict);
            }
        }
    }
    Ok(())
}

#[cfg(all(test, any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
mod tests;
