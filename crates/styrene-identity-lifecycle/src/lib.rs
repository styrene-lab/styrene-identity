//! Shared lifecycle services. The initial surface reads public catalog snapshots.
//!
//! Loading performs bounded blocking filesystem I/O; asynchronous hosts must use
//! their blocking worker. Once loaded, the immutable snapshot can serve public
//! overviews without I/O, credentials, or custody access. Catalog metadata is not
//! evidence of current custody availability or a running session's identity.

use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};
#[cfg(feature = "file-custody")]
pub mod backups;
#[cfg(feature = "file-custody")]
pub mod mutations;
mod safe_fs;
use styrene_identity::IdentityId;
use styrene_identity::overview::{
    CapabilitySummary, IdentityCapability, IdentityOverview, IdentityOverviewSource,
    IdentitySelection, OverviewError, ProviderAvailability, PublicIdentityStatus,
    PublicIdentitySummary, RootExposure, UnavailableReason,
};

pub const CATALOG_SCHEMA_VERSION: u32 = 1;
pub const MAX_CATALOG_BYTES: u64 = 1_048_576;
pub const MAX_CATALOG_ENTRIES: usize = 1024;

/// Stable machine-readable service failures, without raw filesystem/parser text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum LifecycleError {
    #[error("identity catalog has not been initialized")]
    CatalogUninitialized,
    #[error("identity catalog cannot be read")]
    CatalogUnavailable,
    #[error("identity catalog exceeds its size limit")]
    CatalogTooLarge,
    #[error("identity catalog is invalid")]
    InvalidCatalog,
    #[error("identity catalog schema is unsupported")]
    UnsupportedSchema,
    #[error("identity entry was not found")]
    EntryNotFound,
    #[error("identity name matches multiple entries; use an entry ID")]
    AmbiguousName,
    #[error("canonical identity does not match the expected identity")]
    IdentityMismatch,
    #[error("canonical identity is not known")]
    IdentityUnavailable,
    #[error("invalid mutation request")]
    InvalidRequest,
    #[error("protected credential input is required")]
    AuthenticationRequired,
    #[error("custody authentication failed")]
    AuthenticationFailed,
    #[error("custody is invalid or unavailable")]
    CustodyUnavailable,
    #[error("destination or identity is already registered")]
    DestinationConflict,
    #[error("catalog or entry revision has changed")]
    RevisionConflict,
    #[error("another mutation holds the store lock")]
    StoreBusy,
    #[error("request ID was already used for a different request")]
    RequestConflict,
    #[error("an incomplete operation requires reconciliation")]
    ReconciliationRequired,
    #[error("operation was not found")]
    OperationNotFound,
    #[error("operation state could not be read or persisted")]
    OperationFailed,
    #[error("file mutations are unsupported on this target")]
    UnsupportedOperation,
    #[error("storage ownership or permissions are unsafe")]
    UnsafeStorage,
    #[error("a bound storage directory has changed")]
    LocationChanged,
    #[error("encrypted backup format or size is invalid")]
    InvalidBackup,
}

/// Application operations, rather than custody-provider capabilities.
#[derive(Clone, Debug, Serialize)]
pub struct ServiceCapabilities {
    pub catalog_schema_version: u32,
    pub supported: &'static [&'static str],
    pub mutations_supported: bool,
}

pub fn capabilities() -> ServiceCapabilities {
    ServiceCapabilities {
        catalog_schema_version: CATALOG_SCHEMA_VERSION,
        #[cfg(all(
            feature = "file-custody",
            any(target_os = "linux", target_os = "android", target_vendor = "apple")
        ))]
        supported: &[
            "capabilities",
            "identity.list",
            "identity.show",
            "identity.create",
            "identity.adopt",
            "identity.update",
            "identity.select",
            "identity.forget",
            "operation.show",
            "operation.reconcile",
            "backup.inspect",
            "backup.verify",
        ],
        #[cfg(all(
            feature = "file-custody",
            not(any(target_os = "linux", target_os = "android", target_vendor = "apple"))
        ))]
        supported: &[
            "capabilities",
            "identity.list",
            "identity.show",
            "backup.inspect",
            "backup.verify",
        ],
        #[cfg(not(feature = "file-custody"))]
        supported: &["capabilities", "identity.list", "identity.show"],
        mutations_supported: cfg!(all(
            feature = "file-custody",
            any(target_os = "linux", target_os = "android", target_vendor = "apple")
        )),
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogFile {
    schema_version: u32,
    revision: u64,
    preferred_entry: Option<String>,
    entries: Vec<CatalogEntry>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogEntry {
    entry_id: String,
    revision: u64,
    display_name: Option<String>,
    public_identity: Option<CatalogPublicIdentity>,
    custody_refs: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogPublicIdentity {
    identity_id: String,
    public_key: String,
}

/// Wire projection for CLI/UI application results, not a custody attestation.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PublicIdentityView {
    HashConsistent { identity_id: String, public_key: String },
    Unavailable { reason: &'static str },
}

#[derive(Clone, Debug, Serialize)]
pub struct IdentityEntryView {
    pub entry_id: String,
    pub revision: u64,
    pub display_name: Option<String>,
    pub identity: PublicIdentityView,
    pub custody_refs: Vec<String>,
    pub provider_availability: &'static str,
    pub root_exposure: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct IdentityInventory {
    pub revision: u64,
    pub preferred_entry: Option<String>,
    pub entries: Vec<IdentityEntryView>,
}

struct Entry {
    metadata: CatalogEntry,
    identity: Option<PublicIdentitySummary>,
}

impl Entry {
    fn view(&self) -> IdentityEntryView {
        IdentityEntryView {
            entry_id: self.metadata.entry_id.clone(),
            revision: self.metadata.revision,
            display_name: self.metadata.display_name.clone(),
            identity: match &self.identity {
                Some(identity) => PublicIdentityView::HashConsistent {
                    identity_id: identity.identity_id().to_string(),
                    public_key: hex::encode(identity.public_key()),
                },
                None => PublicIdentityView::Unavailable { reason: "not_known" },
            },
            custody_refs: self.metadata.custody_refs.clone(),
            provider_availability: "unknown",
            root_exposure: "unknown",
        }
    }
}

/// An immutable, validated public snapshot. Selections are local to this instance.
pub struct CatalogSnapshot {
    revision: u64,
    preferred_entry: Option<String>,
    entries: Vec<Entry>,
}

impl CatalogSnapshot {
    /// Read `<store>/catalog.json` without creating any files or accessing custody.
    pub fn load(store: &Path) -> Result<Self, LifecycleError> {
        let path = store.join("catalog.json");
        let bytes =
            safe_fs::read(&path, MAX_CATALOG_BYTES, false).map_err(|error| match error {
                LifecycleError::OperationNotFound => LifecycleError::CatalogUninitialized,
                LifecycleError::CatalogTooLarge => LifecycleError::CatalogTooLarge,
                _ => LifecycleError::CatalogUnavailable,
            })?;
        Self::from_bytes(&bytes)
    }

    /// Validate a bounded catalog supplied by an application storage adapter.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, LifecycleError> {
        if bytes.len() as u64 > MAX_CATALOG_BYTES {
            return Err(LifecycleError::CatalogTooLarge);
        }
        // Read just the schema header first so newer valid schemas have a distinct
        // failure. Deserialize the original bytes again to reject duplicate fields.
        #[derive(Deserialize)]
        struct Header {
            schema_version: u32,
        }
        let header: Header =
            serde_json::from_slice(bytes).map_err(|_| LifecycleError::InvalidCatalog)?;
        if header.schema_version != CATALOG_SCHEMA_VERSION {
            return Err(LifecycleError::UnsupportedSchema);
        }
        let catalog: CatalogFile =
            serde_json::from_slice(bytes).map_err(|_| LifecycleError::InvalidCatalog)?;
        if catalog.schema_version != CATALOG_SCHEMA_VERSION
            || catalog.entries.len() > MAX_CATALOG_ENTRIES
        {
            return Err(LifecycleError::InvalidCatalog);
        }
        let mut ids = HashSet::new();
        let mut entries = Vec::with_capacity(catalog.entries.len());
        for metadata in catalog.entries {
            if !valid_id(&metadata.entry_id)
                || !ids.insert(metadata.entry_id.clone())
                || metadata.display_name.as_ref().is_some_and(|name| {
                    name.trim().is_empty() || name.len() > 128 || name.chars().any(char::is_control)
                })
                || metadata.custody_refs.len() > 32
            {
                return Err(LifecycleError::InvalidCatalog);
            }
            let mut custody_ids = HashSet::new();
            if metadata.custody_refs.iter().any(|id| !valid_id(id) || !custody_ids.insert(id)) {
                return Err(LifecycleError::InvalidCatalog);
            }
            let identity = metadata
                .public_identity
                .as_ref()
                .map(|public| {
                    let id =
                        public.identity_id.parse().map_err(|_| LifecycleError::InvalidCatalog)?;
                    if public.public_key.len() != 64
                        || !public
                            .public_key
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                    {
                        return Err(LifecycleError::InvalidCatalog);
                    }
                    let mut key = [0; 32];
                    hex::decode_to_slice(&public.public_key, &mut key)
                        .map_err(|_| LifecycleError::InvalidCatalog)?;
                    PublicIdentitySummary::from_claimed_identity(id, key)
                        .map_err(|_| LifecycleError::InvalidCatalog)
                })
                .transpose()?;
            entries.push(Entry { metadata, identity });
        }
        if catalog.preferred_entry.as_ref().is_some_and(|id| !ids.contains(id)) {
            return Err(LifecycleError::InvalidCatalog);
        }
        entries.sort_by(|a, b| a.metadata.entry_id.cmp(&b.metadata.entry_id));
        Ok(Self { revision: catalog.revision, preferred_entry: catalog.preferred_entry, entries })
    }

    pub fn list(&self) -> IdentityInventory {
        IdentityInventory {
            revision: self.revision,
            preferred_entry: self.preferred_entry.clone(),
            entries: self.entries.iter().map(Entry::view).collect(),
        }
    }

    /// Exact entry IDs take precedence over display names. Ambiguous names fail.
    pub fn selection(&self, target: &str) -> Result<IdentitySelection, LifecycleError> {
        if let Some(index) = self.entries.iter().position(|e| e.metadata.entry_id == target) {
            return Ok(IdentitySelection(index as u64));
        }
        let mut matches = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.metadata.display_name.as_deref() == Some(target));
        let (index, _) = matches.next().ok_or(LifecycleError::EntryNotFound)?;
        if matches.next().is_some() {
            return Err(LifecycleError::AmbiguousName);
        }
        Ok(IdentitySelection(index as u64))
    }

    pub fn show(
        &self,
        target: &str,
        expected_identity: Option<IdentityId>,
    ) -> Result<IdentityEntryView, LifecycleError> {
        let selection = self.selection(target)?;
        let entry = self.entry(selection).ok_or(LifecycleError::EntryNotFound)?;
        if let Some(expected) = expected_identity {
            let identity = entry.identity.as_ref().ok_or(LifecycleError::IdentityUnavailable)?;
            if identity.identity_id() != expected {
                return Err(LifecycleError::IdentityMismatch);
            }
        }
        Ok(entry.view())
    }

    fn entry(&self, selection: IdentitySelection) -> Option<&Entry> {
        usize::try_from(selection.0).ok().and_then(|index| self.entries.get(index))
    }
}

#[async_trait::async_trait]
impl IdentityOverviewSource for CatalogSnapshot {
    async fn read_public_overview(
        &self,
        selection: IdentitySelection,
    ) -> Result<IdentityOverview, OverviewError> {
        let entry =
            self.entry(selection).ok_or(OverviewError::Unavailable(UnavailableReason::NotKnown))?;
        Ok(IdentityOverview {
            identity: entry.identity.clone().map_or(
                PublicIdentityStatus::Unavailable(UnavailableReason::NotKnown),
                PublicIdentityStatus::Available,
            ),
            availability: ProviderAvailability::Unknown,
            root_exposure: RootExposure::Unknown,
            capabilities: vec![CapabilitySummary {
                capability: IdentityCapability::PublicOverview,
                availability: ProviderAvailability::Available,
            }],
        })
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
