//! Concrete fixture host for the committed one-page extension design. This is
//! not Mesh integration and never constructs custody or a lifecycle worker.
use dioxus::prelude::*;
use std::sync::Arc;
use styrene_identity::overview::*;
use styrene_identity_ui::{IDENTITY_EXTENSION, IdentityExtension, IdentityPage, OverviewClient};

struct Fixture {
    available: bool,
}
#[async_trait::async_trait]
impl IdentityOverviewSource for Fixture {
    async fn read_public_overview(
        &self,
        _: IdentitySelection,
    ) -> Result<IdentityOverview, OverviewError> {
        if !self.available {
            return Err(OverviewError::Unavailable(UnavailableReason::ProviderUnavailable));
        }
        Ok(IdentityOverview {
            identity: PublicIdentityStatus::Available(PublicIdentitySummary::from_public_key(
                [42; 32],
            )),
            availability: ProviderAvailability::Unknown,
            root_exposure: RootExposure::Unknown,
            capabilities: vec![CapabilitySummary {
                capability: IdentityCapability::PublicOverview,
                availability: ProviderAvailability::Available,
            }],
        })
    }
}

#[component]
pub fn MockHost() -> Element {
    let mut model = use_signal(|| {
        let mut model = IdentityExtension::new(true, IDENTITY_EXTENSION, 1, None);
        let _ = model.mount(); // --extension-demo explicitly requests the fixture page.
        model
    });
    let mut available = use_signal(|| true);
    let mut pending = use_signal(|| false);
    let mut independent = use_signal(|| true);
    let mut message = use_signal(|| {
        "Fixture-only host. No Mesh session or custody provider is connected.".to_owned()
    });
    let client =
        use_memo(move || OverviewClient::new(Arc::new(Fixture { available: *available.read() })));
    let generation = model.read().generation();
    let registered = model.read().registered();
    let mounted = model.read().mounted();
    rsx! {
        document::Stylesheet{href:crate::STYLES}
        main{class:"identity-app",h1{"Identity extension dogfood"}code{"fixture / dev {crate::BUILD_REVISION}"}
            section{class:"operation-status",role:"status","{message}"}
            section{class:"store-bar",h2{"Core host controls (mock)"}p{"These controls remain available with the Identity page disabled."}
                button{onclick:move |_|{let result=model.write().session_changed();message.set(format!("Host session changed: {result:?}"));},"Change mock session"}
                button{onclick:move |_|{let next=!*available.read();available.set(next);let _=model.write().session_changed();},"Toggle provider availability"}
            }
            nav{class:"workspace-nav","aria-label":"Extension lifecycle controls",
                button{onclick:move |_|{let result=model.write().enable();message.set(format!("Enable: {result:?}"));},"Enable Identity extension"}
                button{onclick:move |_|{let result=model.write().disable(*pending.read(),*independent.read());message.set(format!("Disable: {result:?}"));},"Disable Identity extension"}
                if registered{button{onclick:move |_|{let result=model.write().mount();message.set(format!("Open page: {result:?}"));},"Open Identity page"}}
                if mounted{button{onclick:move |_|{let result=model.write().unmount();message.set(format!("Close page: {result:?}"));},"Close Identity page"}}
            }
            label{input{r#type:"checkbox",checked:*pending.read(),onchange:move |event|pending.set(event.checked())}"Simulate an accepted backend operation"}
            label{input{r#type:"checkbox",checked:*independent.read(),onchange:move |event|independent.set(event.checked())}"Host retains independent observation"}
            p{"Registered: {registered}; mounted: {mounted}; generation: {generation}"}
            if registered&&mounted{IdentityPage{client:client.read().clone(),request:OverviewRequest{selection:IdentitySelection(0),scope:OverviewScope(generation),expected_identity:None},fixture:true}}
        }
    }
}
