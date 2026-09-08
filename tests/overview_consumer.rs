//! Public-API consumer tests; no custody or mesh services are constructed.
use std::sync::atomic::{AtomicUsize, Ordering};

use styrene_identity::IdentityId;
use styrene_identity::overview::*;

struct MockSource {
    calls: AtomicUsize,
    identity: PublicIdentityStatus,
}

#[async_trait::async_trait]
impl IdentityOverviewSource for MockSource {
    async fn read_public_overview(
        &self,
        selection: IdentitySelection,
    ) -> Result<IdentityOverview, OverviewError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if selection != IdentitySelection(7) {
            return Err(OverviewError::Unavailable(UnavailableReason::ProviderUnavailable));
        }
        Ok(IdentityOverview {
            identity: self.identity.clone(),
            availability: ProviderAvailability::Unknown,
            root_exposure: RootExposure::Unknown,
            capabilities: vec![CapabilitySummary {
                capability: IdentityCapability::PublicOverview,
                availability: ProviderAvailability::Available,
            }],
        })
    }
}

fn request() -> OverviewRequest {
    OverviewRequest {
        selection: IdentitySelection(7),
        scope: OverviewScope(1),
        expected_identity: Some(IdentityId::from_public_key(&[42; 32])),
    }
}

fn source(identity: PublicIdentityStatus) -> MockSource {
    MockSource { calls: AtomicUsize::new(0), identity }
}

#[tokio::test]
async fn public_read_checks_identity_without_claiming_custody() {
    let source =
        source(PublicIdentityStatus::Available(PublicIdentitySummary::from_public_key([42; 32])));
    let result = read_identity_overview(&source, request())
        .await
        .into_result_for(Some(request()))
        .expect("current scope")
        .expect("matching public identity");
    assert_eq!(result.root_exposure, RootExposure::Unknown);
    assert_eq!(result.availability, ProviderAvailability::Unknown);
    assert_eq!(source.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn changed_identity_is_rejected_without_retry() {
    let source =
        source(PublicIdentityStatus::Available(PublicIdentitySummary::from_public_key([43; 32])));
    assert_eq!(
        read_identity_overview(&source, request()).await.into_result_for(Some(request())),
        Some(Err(OverviewError::IdentityMismatch)),
    );
    assert_eq!(source.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn unavailable_identity_is_not_fabricated_or_accepted_as_expected() {
    for reason in [UnavailableReason::AuthenticationRequired, UnavailableReason::NotKnown] {
        let source = source(PublicIdentityStatus::Unavailable(reason));
        let discovery = OverviewRequest { expected_identity: None, ..request() };
        let result = read_identity_overview(&source, discovery)
            .await
            .into_result_for(Some(discovery))
            .unwrap()
            .unwrap();
        assert_eq!(result.identity, PublicIdentityStatus::Unavailable(reason));
        assert_eq!(
            read_identity_overview(&source, request()).await.into_result_for(Some(request())),
            Some(Err(OverviewError::Unavailable(reason))),
        );
    }
}

#[tokio::test]
async fn host_discards_results_after_disable_selection_change_or_reenable() {
    let source = source(PublicIdentityStatus::Unavailable(UnavailableReason::Unsupported));
    let completion = read_identity_overview(&source, request()).await;
    for current in [
        None,
        Some(OverviewRequest { scope: OverviewScope(2), ..request() }),
        Some(OverviewRequest { selection: IdentitySelection(8), ..request() }),
        Some(OverviewRequest { expected_identity: None, ..request() }),
    ] {
        assert_eq!(completion.clone().into_result_for(current), None);
    }
    assert_eq!(source.calls.load(Ordering::SeqCst), 1, "dropping observation never retries");
}

#[tokio::test]
async fn unknown_selection_does_not_use_another_identity() {
    let source =
        source(PublicIdentityStatus::Available(PublicIdentitySummary::from_public_key([42; 32])));
    let request = OverviewRequest { selection: IdentitySelection(9), ..request() };
    assert_eq!(
        read_identity_overview(&source, request).await.into_result_for(Some(request)),
        Some(Err(OverviewError::Unavailable(UnavailableReason::ProviderUnavailable))),
    );
}

#[test]
fn inconsistent_public_claim_is_rejected() {
    assert_eq!(
        PublicIdentitySummary::from_claimed_identity(
            IdentityId::from_public_key(&[1; 32]),
            [2; 32]
        ),
        Err(OverviewError::InvalidPublicBinding),
    );
}
