use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use styrene_identity::overview::*;
use styrene_identity_ui::OverviewClient;

struct Source {
    available: AtomicBool,
    calls: AtomicUsize,
}
#[async_trait::async_trait]
impl IdentityOverviewSource for Source {
    async fn read_public_overview(
        &self,
        _: IdentitySelection,
    ) -> Result<IdentityOverview, OverviewError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if !self.available.load(Ordering::SeqCst) {
            return Err(OverviewError::Unavailable(UnavailableReason::ProviderUnavailable));
        }
        Ok(IdentityOverview {
            identity: PublicIdentityStatus::Unavailable(UnavailableReason::NotKnown),
            availability: ProviderAvailability::Unknown,
            root_exposure: RootExposure::Unknown,
            capabilities: vec![],
        })
    }
}

#[tokio::test]
async fn typed_client_retries_errors_and_preserves_scope_correlation() {
    let source = Arc::new(Source { available: AtomicBool::new(false), calls: AtomicUsize::new(0) });
    let client = OverviewClient::new(source.clone());
    let request = OverviewRequest {
        selection: IdentitySelection(1),
        scope: OverviewScope(1),
        expected_identity: None,
    };
    assert!(client.read(request).await.into_result_for(Some(request)).unwrap().is_err());
    source.available.store(true, Ordering::SeqCst);
    let completed = client.read(request).await;
    assert!(completed.clone().into_result_for(Some(request)).unwrap().is_ok());
    assert!(
        completed
            .into_result_for(Some(OverviewRequest { scope: OverviewScope(2), ..request }))
            .is_none()
    );
    assert_eq!(source.calls.load(Ordering::SeqCst), 2);
}
