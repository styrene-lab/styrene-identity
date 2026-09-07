//! A single optional Identity page and its concrete host lifecycle model.
//! No Mesh, daemon, CLI, custody mutation, or runtime plugin loader is required.

use dioxus::prelude::*;
use std::sync::Arc;
use styrene_identity::overview::*;

pub mod lifecycle;
pub use lifecycle::{ExtensionDescriptor, ExtensionError, IDENTITY_EXTENSION, IdentityExtension};

pub const IDENTITY_STYLES: Asset = asset!("/assets/identity.css");

#[derive(Clone)]
pub struct OverviewClient(Arc<dyn IdentityOverviewSource>);
impl OverviewClient {
    pub fn new(source: Arc<dyn IdentityOverviewSource>) -> Self {
        Self(source)
    }
    pub async fn read(&self, request: OverviewRequest) -> OverviewCompletion {
        read_identity_overview(self.0.as_ref(), request).await
    }
}
impl PartialEq for OverviewClient {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// Mount with a fresh request scope whenever the host replaces its client,
/// identity selection, or activation/view generation. The page only performs
/// public reads. Host navigation and pending mutation observation remain outside.
#[component]
pub fn IdentityPage(
    client: OverviewClient,
    request: OverviewRequest,
    #[props(default)] fixture: bool,
) -> Element {
    rsx! {
        document::Stylesheet { href: IDENTITY_STYLES }
        section { class: "identity-page", "aria-labelledby": "identity-page-title",
            header { class: "identity-page-heading",
                div { p { class:"identity-eyebrow", "STYRENE / IDENTITY" } h1 { id:"identity-page-title", "Identity overview" } }
                span { class:"identity-badge", "PUBLIC VIEW" }
            }
            if fixture { p { class:"identity-notice", role:"status", "Fixture data — no custody operations are connected." } }
            OverviewBody { key:"{request.scope.0}:{request.selection.0}:{request.expected_identity:?}", client, request }
        }
    }
}

#[component]
fn OverviewBody(client: OverviewClient, request: OverviewRequest) -> Element {
    let mut resource = use_resource(move || {
        let client = client.clone();
        async move {
            client
                .read(request)
                .await
                .into_result_for(Some(request))
                .unwrap_or(Err(OverviewError::Cancelled))
        }
    });
    let result = resource.read_unchecked().clone();
    rsx! {
        OverviewContent { result }
        button { class:"identity-button", onclick:move |_|resource.restart(), "Refresh public information" }
    }
}

#[component]
pub fn OverviewContent(result: Option<Result<IdentityOverview, OverviewError>>) -> Element {
    match result {
        None => rsx! {p {role:"status", "Loading public identity…"}},
        Some(Err(error)) => {
            rsx! {div {class:"identity-notice", role:"status", h2 {"Identity information unavailable"} p {"{error}"} p {"No provider was unlocked and no identity was generated."}}}
        }
        Some(Ok(overview)) => {
            let availability = format!("{:?}", overview.availability);
            let exposure = format!("{:?}", overview.root_exposure);
            rsx! {
                match overview.identity {
                    PublicIdentityStatus::Available(identity)=>rsx!{
                        article {class:"identity-card",
                            h2 {"Canonical Styrene Identity ID"}
                            code {class:"identity-id", "{identity.identity_id()}"}
                            p {class:"identity-muted", "Public-key hash consistency checked. Key possession and running-session binding are separate evidence."}
                            details {summary {"Public key"} code {class:"identity-key", "{hex_key(identity.public_key())}"}}
                        }
                    },
                    PublicIdentityStatus::Unavailable(reason)=>rsx!{article {class:"identity-card",h2 {"Canonical identity unavailable"}p {"{reason:?}"}p {"An RNS hash or LXMF address will not be substituted."}}},
                    _=>rsx!{p {"Unsupported public identity status"}},
                }
                div {class:"identity-facts",
                    article {class:"identity-card",h2 {"Provider availability"}p {"{availability}"}},
                    article {class:"identity-card",h2 {"Root exposure"}p {"{exposure}"}p {class:"identity-muted","Provider declaration, not hardware attestation."}},
                }
                article {class:"identity-card",h2 {"Declared capabilities"}
                    ul {for item in overview.capabilities {li {"{item.capability:?}: {item.availability:?}"}}}
                }
            }
        }
    }
}

fn hex_key(bytes: &[u8; 32]) -> String {
    use std::fmt::Write;
    let mut text = String::with_capacity(64);
    for byte in bytes {
        let _ = write!(text, "{byte:02x}");
    }
    text
}
