use dioxus::prelude::*;
use styrene_identity::overview::*;
use styrene_identity_ui::OverviewContent;

fn unavailable() -> Element {
    rsx! {OverviewContent{result:Some(Err(OverviewError::Unavailable(UnavailableReason::Unsupported)))}}
}
fn public() -> Element {
    rsx! {OverviewContent{result:Some(Ok(IdentityOverview{
        identity:PublicIdentityStatus::Available(PublicIdentitySummary::from_public_key([42;32])),availability:ProviderAvailability::Unknown,root_exposure:RootExposure::Unknown,capabilities:vec![],
    }))}}
}

#[test]
fn public_page_labels_evidence_without_claiming_unlock_or_runtime_binding() {
    let html = dioxus_ssr::render_element(public());
    assert!(html.contains("Canonical Styrene Identity ID"));
    assert!(html.contains("Key possession and running-session binding are separate evidence"));
    assert!(html.contains("Unknown"));
    let html = dioxus_ssr::render_element(unavailable());
    assert!(html.contains("unavailable"));
    assert!(html.contains("No provider was unlocked"));
}
