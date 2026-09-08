//! Read-only public Identity summaries for application clients.
//!
//! This module does not access custody. Implementations supply already available
//! public information and must not unlock, acquire a root, or mutate storage to
//! answer a read. A trait cannot enforce the behavior of trusted implementation
//! code. No built-in custody adapter implements this contract yet.
//!
//! The contract is an in-process Rust API, not a serialized plugin protocol.
//! A summary establishes neither key possession nor a running session's identity.

use crate::IdentityId;

/// Backend-issued selector, scoped to a particular client instance.
///
/// The backend allocates and resolves tokens; callers must not interpret them as
/// paths, identity IDs, or authorization. Do not reuse a token for another identity
/// during the lifetime of a client. Unknown tokens return `Unavailable`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IdentitySelection(pub u64);

/// Host-owned observation generation.
///
/// Allocate a fresh value on activation, selection change, and re-enable. Do not
/// reuse values while an earlier request can complete. This is correlation, not
/// an identity check or a backend cancellation token.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OverviewScope(pub u64);

/// One public read. An absent expected ID permits discovery, not binding proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverviewRequest {
    pub selection: IdentitySelection,
    pub scope: OverviewScope,
    pub expected_identity: Option<IdentityId>,
}

/// Why public information cannot currently be supplied without interaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnavailableReason {
    Unsupported,
    ProviderUnavailable,
    AuthenticationRequired,
    NotKnown,
}

/// A public key and its canonical ID, checked for hash consistency only.
///
/// This does not validate possession of the private key or attest custody.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicIdentitySummary {
    identity_id: IdentityId,
    public_key: [u8; 32],
}

impl PublicIdentitySummary {
    /// Compute the canonical ID from already available public key bytes.
    pub fn from_public_key(public_key: [u8; 32]) -> Self {
        Self { identity_id: IdentityId::from_public_key(&public_key), public_key }
    }

    /// Check a claimed ID against its public key without claiming possession.
    pub fn from_claimed_identity(
        identity_id: IdentityId,
        public_key: [u8; 32],
    ) -> Result<Self, OverviewError> {
        if !identity_id.matches_public_key(&public_key) {
            return Err(OverviewError::InvalidPublicBinding);
        }
        Ok(Self { identity_id, public_key })
    }

    pub fn identity_id(&self) -> IdentityId {
        self.identity_id
    }

    pub fn public_key(&self) -> &[u8; 32] {
        &self.public_key
    }
}

/// Public identity is either hash-consistent or explicitly unavailable.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PublicIdentityStatus {
    Available(PublicIdentitySummary),
    Unavailable(UnavailableReason),
}

/// An availability hint, separate from public identity and successful operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProviderAvailability {
    Available,
    Unavailable(UnavailableReason),
    Unknown,
}

/// Provider-declared root exposure; not hardware attestation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RootExposure {
    HostMemory,
    NonExportable,
    Unknown,
}

/// Operation identifiers for discovery only. This client cannot execute them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum IdentityCapability {
    PublicOverview,
    PublicDerivation,
    Signing,
    EncryptedBackupExport,
    EncryptedBackupRestore,
    PrivateKeyExport,
}

/// A provider's declaration, not a grant of authorization or proof of success.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapabilitySummary {
    pub capability: IdentityCapability,
    pub availability: ProviderAvailability,
}

/// Public information only. Never put credentials or private bytes in this DTO.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentityOverview {
    pub identity: PublicIdentityStatus,
    pub availability: ProviderAvailability,
    pub root_exposure: RootExposure,
    pub capabilities: Vec<CapabilitySummary>,
}

/// Structured read failures. Diagnostic strings are deliberately absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum OverviewError {
    #[error("public identity information unavailable: {0:?}")]
    Unavailable(UnavailableReason),
    #[error("public identity does not match the expected canonical Identity ID")]
    IdentityMismatch,
    #[error("claimed canonical Identity ID does not match its public key")]
    InvalidPublicBinding,
    #[error("public identity read cancelled")]
    Cancelled,
    #[error("public identity read failed")]
    OperationFailed,
}

/// Source of already available public summaries.
///
/// Implementations must not prompt, unlock, acquire roots, mutate custody, invoke
/// CLI commands, or silently fall back to another selection. Report unavailable
/// if these actions would be needed. There is deliberately no `IdentitySigner`
/// adapter: that interface only promises access to a root, not public metadata.
#[async_trait::async_trait]
pub trait IdentityOverviewSource: Send + Sync {
    async fn read_public_overview(
        &self,
        selection: IdentitySelection,
    ) -> Result<IdentityOverview, OverviewError>;
}

/// Correlated read outcome. Consume it against the host's current request scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverviewCompletion {
    request: OverviewRequest,
    result: Result<IdentityOverview, OverviewError>,
}

impl OverviewCompletion {
    /// Discard obsolete results, including failures. Pass `None` when disabled.
    ///
    /// This drops observation only. It does not cancel accepted backend work.
    pub fn into_result_for(
        self,
        current: Option<OverviewRequest>,
    ) -> Option<Result<IdentityOverview, OverviewError>> {
        (current == Some(self.request)).then_some(self.result)
    }
}

/// Read public metadata, enforce an expected canonical identity, and correlate
/// the outcome. No cached discovery result is used for the identity check.
pub async fn read_identity_overview(
    source: &dyn IdentityOverviewSource,
    request: OverviewRequest,
) -> OverviewCompletion {
    let result = source.read_public_overview(request.selection).await.and_then(|overview| {
        if let Some(expected) = request.expected_identity {
            match &overview.identity {
                PublicIdentityStatus::Available(identity) if identity.identity_id() != expected => {
                    return Err(OverviewError::IdentityMismatch);
                }
                PublicIdentityStatus::Unavailable(reason) => {
                    return Err(OverviewError::Unavailable(*reason));
                }
                _ => {}
            }
        }
        Ok(overview)
    });
    OverviewCompletion { request, result }
}
