//! Serialized file-custody/catalog mutations with durable retry records.
//!
//! Every method is blocking. Hosts must run it on their blocking worker. The
//! private journal includes public metadata, custody locators and, for creation,
//! the encrypted STID recovery artifact. It never contains plaintext credentials
//! or roots. Completed records are retained; replay returns the historical result.

use std::fs::{self, File};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use styrene_identity::IdentityId;
use styrene_identity::identity::identity_pubkey;
use styrene_identity::signer::RootSecret;
use styrene_identity::vault::EncryptedIdentityBackup;

use crate::safe_fs::{self, Directory, DirectoryIdentity};
use crate::{
    CatalogEntry, CatalogFile, CatalogPublicIdentity, CatalogSnapshot, LifecycleError,
    MAX_CATALOG_BYTES, valid_id,
};

const MAX_JOURNAL_BYTES: u64 = MAX_CATALOG_BYTES + 16_384;
const MAX_OPERATIONS: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Mutation {
    Create {
        name: String,
        destination: PathBuf,
    },
    Adopt {
        name: String,
        custody: PathBuf,
        #[serde(with = "identity_text")]
        expected_identity: IdentityId,
    },
    Update {
        entry_id: String,
        if_revision: u64,
        name: NameChange,
    },
    Select {
        entry_id: String,
        if_revision: u64,
    },
    Forget {
        entry_id: String,
        if_revision: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", content = "value", rename_all = "snake_case")]
pub enum NameChange {
    Set(String),
    Clear,
}

impl Mutation {
    pub fn needs_credentials(&self) -> bool {
        matches!(self, Self::Create { .. } | Self::Adopt { .. })
    }

    pub fn command(&self) -> &'static str {
        match self {
            Self::Create { .. } => "identity.create",
            Self::Adopt { .. } => "identity.adopt",
            Self::Update { .. } => "identity.update",
            Self::Select { .. } => "identity.select",
            Self::Forget { .. } => "identity.forget",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationState {
    Prepared,
    Completed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogEffect {
    Unchanged,
    Committed,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustodyEffect {
    NotAccessed,
    Unchanged,
    Created,
    AuthenticatedUnchanged,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Effects {
    pub catalog: CatalogEffect,
    pub custody: CustodyEffect,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MutationResult {
    pub operation_id: String,
    pub entry_id: String,
    pub identity_id: Option<String>,
    pub catalog_revision: u64,
    pub entry_revision: Option<u64>,
    pub effects: Effects,
    /// True only when returning an already completed historical receipt.
    #[serde(default)]
    pub replayed: bool,
}

#[derive(Debug)]
pub struct MutationFailure {
    pub error: LifecycleError,
    pub operation_id: Option<String>,
    pub effects: Effects,
}

impl From<LifecycleError> for MutationFailure {
    fn from(error: LifecycleError) -> Self {
        Self {
            error,
            operation_id: None,
            effects: Effects {
                catalog: CatalogEffect::Unchanged,
                custody: CustodyEffect::Unchanged,
            },
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct OperationView {
    pub operation_id: String,
    pub command: String,
    pub state: OperationState,
    pub effects: Effects,
    /// Present only after the operation's durable completion was recorded.
    pub result: Option<MutationResult>,
    pub journal_version: u32,
    pub retains_recovery_material: bool,
}

// Deliberately no Debug: locators and encrypted recovery material stay in the
// private store, not generic application diagnostics or operation-show output.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    schema_version: u32,
    operation_id: String,
    mutation: Mutation,
    before_digest: Option<String>,
    after: Option<CatalogFile>,
    custody_digest: Option<String>,
    encrypted_creation: Option<Vec<u8>>,
    state: OperationState,
    result: MutationResult,
    #[serde(default)]
    parent_identity: Option<DirectoryIdentity>,
}

impl Journal {
    fn pending_effects(&self) -> Effects {
        Effects {
            catalog: CatalogEffect::Unknown,
            custody: match self.mutation {
                Mutation::Create { .. } => CustodyEffect::Unknown,
                Mutation::Adopt { .. } => CustodyEffect::AuthenticatedUnchanged,
                _ => CustodyEffect::NotAccessed,
            },
        }
    }

    fn view(&self) -> OperationView {
        OperationView {
            operation_id: self.operation_id.clone(),
            command: self.mutation.command().into(),
            state: self.state,
            effects: if self.state == OperationState::Completed {
                self.result.effects.clone()
            } else {
                self.pending_effects()
            },
            result: (self.state == OperationState::Completed).then(|| self.result.clone()),
            journal_version: self.schema_version,
            retains_recovery_material: self.encrypted_creation.is_some(),
        }
    }
}

struct Store {
    root: PathBuf,
    directory: Directory,
    operations: Directory,
    _lock: File,
}

impl Store {
    fn open(root: &Path, initialize: bool) -> Result<Self, LifecycleError> {
        if !cfg!(any(target_os = "linux", target_os = "android", target_vendor = "apple")) {
            return Err(LifecycleError::UnsupportedOperation);
        }
        let mut created = false;
        if initialize {
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(root) {
                Ok(()) => created = true,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => return Err(LifecycleError::CatalogUnavailable),
            }
        }
        let root = root.canonicalize().map_err(|_| LifecycleError::CatalogUninitialized)?;
        let directory = Directory::open(&root)?;
        directory.require_owner(true)?;
        if created {
            sync_directory(&root)?;
            sync_directory(root.parent().ok_or(LifecycleError::CatalogUnavailable)?)?;
        }
        let lock = directory.lock_file(".mutation-lock")?;
        lock.try_lock().map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => LifecycleError::StoreBusy,
            _ => LifecycleError::CatalogUnavailable,
        })?;
        let operations = directory.child("operations", true)?;
        Ok(Self { root, directory, operations, _lock: lock })
    }

    fn journal_path(&self, id: &str) -> PathBuf {
        self.root.join("operations").join(format!("{id}.json"))
    }

    fn read_journal(&self, id: &str) -> Result<Journal, LifecycleError> {
        self.directory.check_bound()?;
        parse_journal(&self.operations.read(
            std::ffi::OsStr::new(&format!("{id}.json")),
            MAX_JOURNAL_BYTES,
            true,
        )?)
    }

    fn write_journal(&self, journal: &Journal, replace: bool) -> Result<(), LifecycleError> {
        self.directory.check_bound()?;
        self.operations.write(
            std::ffi::OsStr::new(&format!("{}.json", journal.operation_id)),
            &journal_bytes(journal)?,
            replace,
        )
    }

    fn catalog(&self) -> Result<(CatalogFile, Option<String>), LifecycleError> {
        match self.directory.read_owned(std::ffi::OsStr::new("catalog.json"), MAX_CATALOG_BYTES) {
            Ok(bytes) => {
                CatalogSnapshot::from_bytes(&bytes)?;
                let catalog =
                    serde_json::from_slice(&bytes).map_err(|_| LifecycleError::InvalidCatalog)?;
                Ok((catalog, Some(digest(&bytes))))
            }
            Err(LifecycleError::OperationNotFound) => Ok((
                CatalogFile {
                    schema_version: 1,
                    revision: 0,
                    preferred_entry: None,
                    entries: vec![],
                },
                None,
            )),
            Err(LifecycleError::UnsafeStorage) => Err(LifecycleError::UnsafeStorage),
            Err(_) => Err(LifecycleError::CatalogUnavailable),
        }
    }

    fn check_pending(&self, current: &str) -> Result<(), LifecycleError> {
        let mut count = 0;
        for name in self.operations.names()? {
            let Some(id) = name.strip_suffix(".json") else {
                continue;
            };
            count += 1;
            if count > MAX_OPERATIONS {
                return Err(LifecycleError::OperationFailed);
            }
            let journal = self.read_journal(id)?;
            if journal.operation_id != id {
                return Err(LifecycleError::OperationFailed);
            }
            if journal.state != OperationState::Completed && journal.operation_id != current {
                return Err(LifecycleError::ReconciliationRequired);
            }
        }
        if count == MAX_OPERATIONS && !self.journal_path(current).exists() {
            return Err(LifecycleError::OperationFailed);
        }
        Ok(())
    }
}

/// Execute or resume one mutation. The request ID must be stable across retries.
/// Credentials are borrowed for this call and are never placed in the journal.
pub fn execute(
    store: &Path,
    request_id: &str,
    mutation: Mutation,
    passphrase: &[u8],
) -> Result<MutationResult, MutationFailure> {
    execute_with_hook(store, request_id, mutation, passphrase, &mut |_| Ok(()))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    JournalPrepared,
    CustodyCommitted,
    CatalogCommitted,
}

fn execute_with_hook(
    store: &Path,
    request_id: &str,
    mutation: Mutation,
    passphrase: &[u8],
    hook: &mut dyn FnMut(Phase) -> Result<(), LifecycleError>,
) -> Result<MutationResult, MutationFailure> {
    if !valid_id(request_id) || request_id.len() > 48 {
        return Err(LifecycleError::InvalidRequest.into());
    }
    validate_request(&mutation)?;
    let mutation = normalize(mutation)?;
    let operation_id = format!("op-{request_id}");
    if mutation.needs_credentials()
        && passphrase.is_empty()
        && !store.join("operations").join(format!("{operation_id}.json")).exists()
    {
        return Err(LifecycleError::AuthenticationRequired.into());
    }
    let store = Store::open(store, mutation.needs_credentials())?;
    validate_location(&store.root, &mutation)?;
    let mut journal = match store.read_journal(&operation_id) {
        Ok(journal) => {
            if journal.operation_id != operation_id || journal.mutation != mutation {
                return Err(LifecycleError::RequestConflict.into());
            }
            if journal.state == OperationState::Completed {
                return Ok(replay(journal.result));
            }
            journal
        }
        Err(LifecycleError::OperationNotFound) => {
            store.check_pending(&operation_id)?;
            let journal = prepare(&store, operation_id.clone(), request_id, mutation, passphrase)?;
            store.write_journal(&journal, false).map_err(|error| MutationFailure {
                error,
                operation_id: Some(operation_id.clone()),
                effects: Effects {
                    catalog: CatalogEffect::Unchanged,
                    custody: if matches!(journal.mutation, Mutation::Adopt { .. }) {
                        CustodyEffect::AuthenticatedUnchanged
                    } else {
                        CustodyEffect::NotAccessed
                    },
                },
            })?;
            journal
        }
        Err(error) => return Err(error.into()),
    };
    finish(&store, &mut journal, passphrase, hook).map_err(|error| MutationFailure {
        error,
        operation_id: Some(operation_id),
        effects: journal.pending_effects(),
    })
}

/// Inspect the recorded outcome, not inferred filesystem state. This does not
/// acquire custody or a mutation lock and never initializes a store.
pub fn show_operation(store: &Path, operation_id: &str) -> Result<OperationView, LifecycleError> {
    validate_operation_id(operation_id)?;
    let journal = read_journal(&store.join("operations").join(format!("{operation_id}.json")))?;
    if journal.operation_id != operation_id {
        return Err(LifecycleError::OperationFailed);
    }
    Ok(journal.view())
}

/// Reconcile an admitted operation using its retained request. Creation/adoption
/// may need fresh credential input. Completed operations are never replayed.
pub fn reconcile(
    store: &Path,
    operation_id: &str,
    passphrase: &[u8],
) -> Result<MutationResult, MutationFailure> {
    validate_operation_id(operation_id)?;
    let locked = Store::open(store, false)?;
    let mut journal = locked.read_journal(operation_id)?;
    if journal.operation_id != operation_id {
        return Err(LifecycleError::OperationFailed.into());
    }
    if journal.state == OperationState::Completed {
        return Ok(replay(journal.result));
    }
    validate_request(&journal.mutation)?;
    validate_location(&locked.root, &journal.mutation)?;
    finish(&locked, &mut journal, passphrase, &mut |_| Ok(())).map_err(|error| MutationFailure {
        error,
        operation_id: Some(operation_id.into()),
        effects: journal.pending_effects(),
    })
}

fn prepare(
    store: &Store,
    operation_id: String,
    request_id: &str,
    mutation: Mutation,
    passphrase: &[u8],
) -> Result<Journal, LifecycleError> {
    let (mut catalog, before_digest) = store.catalog()?;
    let parent = parent_for(&mutation)?;
    let parent_identity = parent.as_ref().map(|parent| parent.identity()).transpose()?;
    if before_digest.is_none() && !mutation.needs_credentials() {
        return Err(LifecycleError::CatalogUninitialized);
    }
    let mut encrypted_creation = None;
    let mut custody_digest = None;
    let entry_id;
    let identity_id;
    let entry_revision;
    let custody_effect;
    match &mutation {
        Mutation::Create { name, destination }
        | Mutation::Adopt { name, custody: destination, .. } => {
            check_passphrase(passphrase)?;
            let root;
            match &mutation {
                Mutation::Create { .. } => {
                    let name = destination.file_name().ok_or(LifecycleError::InvalidRequest)?;
                    if !matches!(
                        parent
                            .as_ref()
                            .ok_or(LifecycleError::OperationFailed)?
                            .read(name, 97, false),
                        Err(LifecycleError::OperationNotFound)
                    ) {
                        return Err(LifecycleError::DestinationConflict);
                    }
                    root = RootSecret::ephemeral();
                    let backup = EncryptedIdentityBackup::protect_root_secret(&root, passphrase)
                        .map_err(|_| LifecycleError::OperationFailed)?;
                    encrypted_creation = Some(backup.encrypted_bytes().to_vec());
                    custody_digest = encrypted_creation.as_deref().map(digest);
                    custody_effect = CustodyEffect::Created;
                }
                Mutation::Adopt { expected_identity, .. } => {
                    let bytes = parent.as_ref().ok_or(LifecycleError::OperationFailed)?.read(
                        destination.file_name().ok_or(LifecycleError::InvalidRequest)?,
                        97,
                        false,
                    )?;
                    root = decrypt(&bytes, passphrase)?;
                    if IdentityId::from_public_key(&identity_pubkey(&root)) != *expected_identity {
                        return Err(LifecycleError::IdentityMismatch);
                    }
                    custody_digest = Some(digest(&bytes));
                    custody_effect = CustodyEffect::AuthenticatedUnchanged;
                }
                _ => return Err(LifecycleError::InvalidRequest),
            }
            let key = identity_pubkey(&root);
            let canonical = IdentityId::from_public_key(&key).to_string();
            entry_id = format!("identity-{request_id}");
            if catalog.entries.iter().any(|entry| {
                entry.entry_id == entry_id
                    || entry
                        .public_identity
                        .as_ref()
                        .is_some_and(|identity| identity.identity_id == canonical)
            }) {
                return Err(LifecycleError::DestinationConflict);
            }
            catalog.entries.push(CatalogEntry {
                entry_id: entry_id.clone(),
                revision: 1,
                display_name: Some(name.clone()),
                public_identity: Some(CatalogPublicIdentity {
                    identity_id: canonical.clone(),
                    public_key: hex::encode(key),
                }),
                custody_refs: vec![format!("custody-{request_id}")],
            });
            identity_id = Some(canonical);
            entry_revision = Some(1);
        }
        Mutation::Update { entry_id: target, if_revision, name } => {
            let entry = catalog
                .entries
                .iter_mut()
                .find(|entry| entry.entry_id == *target)
                .ok_or(LifecycleError::EntryNotFound)?;
            if entry.revision != *if_revision {
                return Err(LifecycleError::RevisionConflict);
            }
            entry.display_name = match name {
                NameChange::Set(name) => Some(name.clone()),
                NameChange::Clear => None,
            };
            entry.revision = increment(entry.revision)?;
            entry_id = target.clone();
            identity_id =
                entry.public_identity.as_ref().map(|identity| identity.identity_id.clone());
            entry_revision = Some(entry.revision);
            custody_effect = CustodyEffect::NotAccessed;
        }
        Mutation::Select { entry_id: target, if_revision } => {
            if catalog.revision != *if_revision {
                return Err(LifecycleError::RevisionConflict);
            }
            let entry = catalog
                .entries
                .iter()
                .find(|entry| entry.entry_id == *target)
                .ok_or(LifecycleError::EntryNotFound)?;
            entry_id = target.clone();
            identity_id =
                entry.public_identity.as_ref().map(|identity| identity.identity_id.clone());
            entry_revision = Some(entry.revision);
            catalog.preferred_entry = Some(target.clone());
            custody_effect = CustodyEffect::NotAccessed;
        }
        Mutation::Forget { entry_id: target, if_revision } => {
            let index = catalog
                .entries
                .iter()
                .position(|entry| entry.entry_id == *target)
                .ok_or(LifecycleError::EntryNotFound)?;
            if catalog.entries[index].revision != *if_revision {
                return Err(LifecycleError::RevisionConflict);
            }
            let entry = catalog.entries.remove(index);
            entry_id = target.clone();
            identity_id = entry.public_identity.map(|identity| identity.identity_id);
            entry_revision = None;
            if catalog.preferred_entry.as_deref() == Some(target) {
                catalog.preferred_entry = None;
            }
            custody_effect = CustodyEffect::NotAccessed;
        }
    }
    catalog.revision = increment(catalog.revision)?;
    catalog.entries.sort_by(|a, b| a.entry_id.cmp(&b.entry_id));
    validate_catalog(&catalog)?;
    let result = MutationResult {
        operation_id: operation_id.clone(),
        entry_id,
        identity_id,
        catalog_revision: catalog.revision,
        entry_revision,
        effects: Effects { catalog: CatalogEffect::Committed, custody: custody_effect },
        replayed: false,
    };
    if let Some(parent) = &parent {
        parent.check_bound()?;
    }
    Ok(Journal {
        schema_version: 2,
        operation_id,
        mutation,
        before_digest,
        after: Some(catalog),
        custody_digest,
        encrypted_creation,
        state: OperationState::Prepared,
        result,
        parent_identity,
    })
}

fn finish(
    store: &Store,
    journal: &mut Journal,
    passphrase: &[u8],
    hook: &mut dyn FnMut(Phase) -> Result<(), LifecycleError>,
) -> Result<MutationResult, LifecycleError> {
    store.check_pending(&journal.operation_id)?;
    let (before, current) = store.catalog()?;
    let after = validate_catalog(journal.after.as_ref().ok_or(LifecycleError::OperationFailed)?)?;
    let after_digest = digest(&after);
    if current.as_deref() == Some(&after_digest) {
        // Catalog already committed before interruption. Do not reapply or unlock.
        complete(journal)?;
        store.write_journal(journal, true)?;
        return Ok(journal.result.clone());
    }
    if current != journal.before_digest {
        return Err(LifecycleError::RevisionConflict);
    }
    validate_transition(&before, journal)?;
    let parent = parent_for(&journal.mutation)?;
    if let Some(parent) = &parent
        && journal.parent_identity != Some(parent.identity()?)
    {
        return Err(LifecycleError::LocationChanged);
    }
    hook(Phase::JournalPrepared)?;
    match &journal.mutation {
        Mutation::Create { destination, .. } => {
            let encrypted =
                journal.encrypted_creation.as_ref().ok_or(LifecycleError::OperationFailed)?;
            let root = decrypt(encrypted, passphrase)?;
            check_root(&root, journal)?;
            let parent = parent.as_ref().ok_or(LifecycleError::OperationFailed)?;
            let name = destination.file_name().ok_or(LifecycleError::InvalidRequest)?;
            match parent.read(name, 97, false) {
                Ok(existing) if existing == *encrypted => {}
                Ok(_) => return Err(LifecycleError::DestinationConflict),
                Err(LifecycleError::OperationNotFound) => parent.write(name, encrypted, false)?,
                Err(_) => return Err(LifecycleError::CustodyUnavailable),
            }
        }
        Mutation::Adopt { custody, .. } => {
            let bytes = parent.as_ref().ok_or(LifecycleError::OperationFailed)?.read(
                custody.file_name().ok_or(LifecycleError::InvalidRequest)?,
                97,
                false,
            )?;
            if journal.custody_digest.as_deref() != Some(&digest(&bytes)) {
                return Err(LifecycleError::DestinationConflict);
            }
            check_root(&decrypt(&bytes, passphrase)?, journal)?;
        }
        _ => {}
    }
    hook(Phase::CustodyCommitted)?;
    if let Some(parent) = &parent {
        parent.check_bound()?;
    }
    if store.catalog()?.1 != journal.before_digest {
        return Err(LifecycleError::RevisionConflict);
    }
    if let Mutation::Create { destination: path, .. } | Mutation::Adopt { custody: path, .. } =
        &journal.mutation
        && journal.custody_digest.as_deref() != Some(&digest(&read_custody(path)?))
    {
        return Err(LifecycleError::DestinationConflict);
    }
    // Cooperating writers hold the same permanent OS file lock throughout.
    store.directory.write(
        std::ffi::OsStr::new("catalog.json"),
        &after,
        journal.before_digest.is_some(),
    )?;
    hook(Phase::CatalogCommitted)?;
    complete(journal)?;
    store.write_journal(journal, true)?;
    Ok(journal.result.clone())
}

fn check_root(root: &RootSecret, journal: &Journal) -> Result<(), LifecycleError> {
    let actual = IdentityId::from_public_key(&identity_pubkey(root)).to_string();
    if journal.result.identity_id.as_deref() != Some(&actual) {
        return Err(LifecycleError::IdentityMismatch);
    }
    Ok(())
}

fn validate_request(mutation: &Mutation) -> Result<(), LifecycleError> {
    let name = match mutation {
        Mutation::Create { name, .. } | Mutation::Adopt { name, .. } => Some(name),
        Mutation::Update { name: NameChange::Set(name), .. } => Some(name),
        _ => None,
    };
    if name.is_some_and(|name| {
        name.trim().is_empty() || name.len() > 128 || name.chars().any(char::is_control)
    }) {
        return Err(LifecycleError::InvalidRequest);
    }
    match mutation {
        Mutation::Update { entry_id, .. }
        | Mutation::Select { entry_id, .. }
        | Mutation::Forget { entry_id, .. }
            if !valid_id(entry_id) =>
        {
            Err(LifecycleError::InvalidRequest)
        }
        _ => Ok(()),
    }
}

fn normalize(mut mutation: Mutation) -> Result<Mutation, LifecycleError> {
    let path = match &mut mutation {
        Mutation::Create { destination, .. } => destination,
        Mutation::Adopt { custody, .. } => custody,
        _ => return Ok(mutation),
    };
    let name = path.file_name().ok_or(LifecycleError::InvalidRequest)?.to_owned();
    let parent =
        path.parent().filter(|path| !path.as_os_str().is_empty()).unwrap_or(Path::new("."));
    *path = parent.canonicalize().map_err(|_| LifecycleError::CustodyUnavailable)?.join(name);
    Ok(mutation)
}

fn validate_location(store: &Path, mutation: &Mutation) -> Result<(), LifecycleError> {
    if let Mutation::Create { destination: path, .. } | Mutation::Adopt { custody: path, .. } =
        mutation
        && (!path.is_absolute() || path.starts_with(store))
    {
        return Err(LifecycleError::InvalidRequest);
    }
    Ok(())
}

fn check_passphrase(passphrase: &[u8]) -> Result<(), LifecycleError> {
    if passphrase.is_empty() {
        return Err(LifecycleError::AuthenticationRequired);
    }
    if passphrase.len() > 4096 {
        return Err(LifecycleError::InvalidRequest);
    }
    Ok(())
}

fn decrypt(bytes: &[u8], passphrase: &[u8]) -> Result<RootSecret, LifecycleError> {
    check_passphrase(passphrase)?;
    EncryptedIdentityBackup::from_encrypted_bytes(bytes.to_vec())
        .map_err(|_| LifecycleError::CustodyUnavailable)?
        .decrypt_root_secret(passphrase)
        .map_err(|_| LifecycleError::AuthenticationFailed)
}

fn read_custody(path: &Path) -> Result<Vec<u8>, LifecycleError> {
    read_regular(path, 97).map_err(|_| LifecycleError::CustodyUnavailable)
}

fn read_regular(path: &Path, max: u64) -> Result<Vec<u8>, LifecycleError> {
    safe_fs::read(path, max, false)
}

fn sync_directory(path: &Path) -> Result<(), LifecycleError> {
    File::open(path).and_then(|file| file.sync_all()).map_err(|_| LifecycleError::OperationFailed)
}

fn validate_catalog(catalog: &CatalogFile) -> Result<Vec<u8>, LifecycleError> {
    let bytes = serde_json::to_vec(catalog).map_err(|_| LifecycleError::InvalidCatalog)?;
    CatalogSnapshot::from_bytes(&bytes)?;
    Ok(bytes)
}

fn journal_bytes(journal: &Journal) -> Result<Vec<u8>, LifecycleError> {
    let bytes = serde_json::to_vec(journal).map_err(|_| LifecycleError::OperationFailed)?;
    if bytes.len() as u64 > MAX_JOURNAL_BYTES {
        return Err(LifecycleError::OperationFailed);
    }
    Ok(bytes)
}

fn read_journal(path: &Path) -> Result<Journal, LifecycleError> {
    parse_journal(&safe_fs::read(path, MAX_JOURNAL_BYTES, true)?)
}

fn parse_journal(bytes: &[u8]) -> Result<Journal, LifecycleError> {
    #[derive(Deserialize)]
    struct Header {
        schema_version: u32,
    }
    let header: Header =
        serde_json::from_slice(bytes).map_err(|_| LifecycleError::OperationFailed)?;
    if !matches!(header.schema_version, 1 | 2) {
        return Err(LifecycleError::UnsupportedSchema);
    }
    let journal: Journal =
        serde_json::from_slice(bytes).map_err(|_| LifecycleError::OperationFailed)?;
    validate_operation_id(&journal.operation_id)?;
    validate_request(&journal.mutation)?;
    if journal.result.operation_id != journal.operation_id
        || journal.result.catalog_revision == 0
        || journal.result.replayed
        || journal.result.effects.catalog != CatalogEffect::Committed
        || !valid_id(&journal.result.entry_id)
        || journal.result.identity_id.as_ref().is_some_and(|id| id.parse::<IdentityId>().is_err())
        || journal.before_digest.as_ref().is_some_and(|value| !valid_digest(value))
    {
        return Err(LifecycleError::OperationFailed);
    }
    let expected_target = match &journal.mutation {
        Mutation::Create { .. } | Mutation::Adopt { .. } => {
            format!("identity-{}", &journal.operation_id[3..])
        }
        Mutation::Update { entry_id, .. }
        | Mutation::Select { entry_id, .. }
        | Mutation::Forget { entry_id, .. } => entry_id.clone(),
    };
    if expected_target != journal.result.entry_id {
        return Err(LifecycleError::OperationFailed);
    }
    if journal.schema_version == 2 && journal.state == OperationState::Completed {
        if journal.after.is_some()
            || journal.encrypted_creation.is_some()
            || journal.before_digest.is_some()
        {
            return Err(LifecycleError::OperationFailed);
        }
        let effect = match &journal.mutation {
            Mutation::Create { .. } => {
                if journal.result.identity_id.is_none() || journal.result.entry_revision != Some(1)
                {
                    return Err(LifecycleError::OperationFailed);
                }
                CustodyEffect::Created
            }
            Mutation::Adopt { expected_identity, .. } => {
                if journal.result.identity_id.as_deref() != Some(&expected_identity.to_string())
                    || journal.result.entry_revision != Some(1)
                {
                    return Err(LifecycleError::OperationFailed);
                }
                CustodyEffect::AuthenticatedUnchanged
            }
            Mutation::Forget { .. } => {
                if journal.result.entry_revision.is_some() {
                    return Err(LifecycleError::OperationFailed);
                }
                CustodyEffect::NotAccessed
            }
            _ => CustodyEffect::NotAccessed,
        };
        if journal.result.effects.custody != effect {
            return Err(LifecycleError::OperationFailed);
        }
        return Ok(journal);
    }
    let after = journal.after.as_ref().ok_or(LifecycleError::OperationFailed)?;
    validate_catalog(after)?;
    if journal.result.catalog_revision != after.revision {
        return Err(LifecycleError::OperationFailed);
    }
    let entry = after.entries.iter().find(|entry| entry.entry_id == expected_target);
    if matches!(journal.mutation, Mutation::Forget { .. }) {
        if entry.is_some() || journal.result.entry_revision.is_some() {
            return Err(LifecycleError::OperationFailed);
        }
    } else {
        let entry = entry.ok_or(LifecycleError::OperationFailed)?;
        if journal.result.entry_revision != Some(entry.revision)
            || journal.result.identity_id
                != entry.public_identity.as_ref().map(|identity| identity.identity_id.clone())
        {
            return Err(LifecycleError::OperationFailed);
        }
    }
    let expected_custody_effect = match &journal.mutation {
        Mutation::Create { .. } => {
            let bytes =
                journal.encrypted_creation.as_ref().ok_or(LifecycleError::OperationFailed)?;
            EncryptedIdentityBackup::from_encrypted_bytes(bytes.clone())
                .map_err(|_| LifecycleError::OperationFailed)?;
            if journal.custody_digest.as_deref() != Some(&digest(bytes)) {
                return Err(LifecycleError::OperationFailed);
            }
            CustodyEffect::Created
        }
        Mutation::Adopt { expected_identity, .. } => {
            if journal.encrypted_creation.is_some()
                || journal.custody_digest.as_ref().is_none_or(|value| !valid_digest(value))
                || journal.result.identity_id.as_deref() != Some(&expected_identity.to_string())
            {
                return Err(LifecycleError::OperationFailed);
            }
            CustodyEffect::AuthenticatedUnchanged
        }
        _ => {
            if journal.encrypted_creation.is_some() || journal.custody_digest.is_some() {
                return Err(LifecycleError::OperationFailed);
            }
            CustodyEffect::NotAccessed
        }
    };
    if journal.result.effects.custody != expected_custody_effect {
        return Err(LifecycleError::OperationFailed);
    }
    Ok(journal)
}

fn parent_for(mutation: &Mutation) -> Result<Option<Directory>, LifecycleError> {
    let path = match mutation {
        Mutation::Create { destination, .. } => destination,
        Mutation::Adopt { custody, .. } => custody,
        _ => return Ok(None),
    };
    let directory = Directory::open(path.parent().ok_or(LifecycleError::InvalidRequest)?)?;
    directory.require_owner(false)?;
    Ok(Some(directory))
}

fn replay(mut result: MutationResult) -> MutationResult {
    result.replayed = true;
    result
}

fn complete(journal: &mut Journal) -> Result<(), LifecycleError> {
    // Catalog commit alone is insufficient justification to discard a recovery
    // copy: custody may have disappeared between commit and reconciliation.
    // Checking ciphertext needs no unlock and does not claim key possession.
    if journal.schema_version == 2
        && let Some(encrypted) = &journal.encrypted_creation
    {
        let parent = parent_for(&journal.mutation)?.ok_or(LifecycleError::OperationFailed)?;
        if journal.parent_identity != Some(parent.identity()?) {
            return Err(LifecycleError::LocationChanged);
        }
        let path = match &journal.mutation {
            Mutation::Create { destination, .. } => destination,
            _ => return Err(LifecycleError::OperationFailed),
        };
        let existing = parent
            .read(path.file_name().ok_or(LifecycleError::InvalidRequest)?, 97, false)
            .map_err(|_| LifecycleError::CustodyUnavailable)?;
        if existing != *encrypted {
            return Err(LifecycleError::DestinationConflict);
        }
    }
    journal.state = OperationState::Completed;
    if journal.schema_version == 2 {
        journal.encrypted_creation = None;
        journal.after = None;
        journal.before_digest = None;
    }
    Ok(())
}

/// Recompute the only permitted transition from current state and the declared
/// request. A structurally valid arbitrary after-snapshot is not authorization.
fn validate_transition(before: &CatalogFile, journal: &Journal) -> Result<(), LifecycleError> {
    let after = journal.after.as_ref().ok_or(LifecycleError::OperationFailed)?;
    let mut expected = before.clone();
    match &journal.mutation {
        Mutation::Create { name, .. } | Mutation::Adopt { name, .. } => {
            let entry = after
                .entries
                .iter()
                .find(|entry| entry.entry_id == journal.result.entry_id)
                .ok_or(LifecycleError::OperationFailed)?;
            if entry.revision != 1
                || entry.display_name.as_deref() != Some(name)
                || entry.public_identity.is_none()
                || entry.custody_refs != [format!("custody-{}", &journal.operation_id[3..])]
                || before.entries.iter().any(|old| {
                    old.entry_id == entry.entry_id
                        || old.public_identity.as_ref().is_some_and(|old_identity| {
                            Some(&old_identity.identity_id) == journal.result.identity_id.as_ref()
                        })
                })
            {
                return Err(LifecycleError::OperationFailed);
            }
            expected.entries.push(entry.clone());
        }
        Mutation::Update { entry_id, if_revision, name } => {
            let entry = expected
                .entries
                .iter_mut()
                .find(|entry| entry.entry_id == *entry_id)
                .ok_or(LifecycleError::OperationFailed)?;
            if entry.revision != *if_revision {
                return Err(LifecycleError::RevisionConflict);
            }
            entry.revision = increment(entry.revision)?;
            entry.display_name = match name {
                NameChange::Set(name) => Some(name.clone()),
                NameChange::Clear => None,
            };
        }
        Mutation::Select { entry_id, if_revision } => {
            if expected.revision != *if_revision {
                return Err(LifecycleError::RevisionConflict);
            }
            expected.preferred_entry = Some(entry_id.clone());
        }
        Mutation::Forget { entry_id, if_revision } => {
            let index = expected
                .entries
                .iter()
                .position(|entry| entry.entry_id == *entry_id)
                .ok_or(LifecycleError::OperationFailed)?;
            let entry = expected.entries.remove(index);
            if entry.revision != *if_revision {
                return Err(LifecycleError::RevisionConflict);
            }
            if entry.public_identity.as_ref().map(|identity| &identity.identity_id)
                != journal.result.identity_id.as_ref()
            {
                return Err(LifecycleError::OperationFailed);
            }
            if expected.preferred_entry.as_ref() == Some(entry_id) {
                expected.preferred_entry = None;
            }
        }
    }
    expected.revision = increment(expected.revision)?;
    expected.entries.sort_by(|a, b| a.entry_id.cmp(&b.entry_id));
    if validate_catalog(&expected)? != validate_catalog(after)? {
        return Err(LifecycleError::OperationFailed);
    }
    Ok(())
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn validate_operation_id(id: &str) -> Result<(), LifecycleError> {
    if id.strip_prefix("op-").is_none_or(|request| !valid_id(request) || request.len() > 48) {
        return Err(LifecycleError::InvalidRequest);
    }
    Ok(())
}

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn increment(revision: u64) -> Result<u64, LifecycleError> {
    revision.checked_add(1).ok_or(LifecycleError::RevisionConflict)
}

mod identity_text {
    use super::*;
    pub fn serialize<S: serde::Serializer>(
        id: &IdentityId,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&id.to_string())
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<IdentityId, D::Error> {
        String::deserialize(deserializer)?.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
mod tests;
