//! Standalone Dioxus application. Backend work is owned by the application worker,
//! never by an individual page. The shared read-only page is independently importable.

use dioxus::prelude::*;
use std::{path::PathBuf, sync::Arc};
use styrene_identity::overview::{OverviewRequest, OverviewScope};
use styrene_identity_lifecycle::{CatalogSnapshot, IdentityInventory, PublicIdentityView};
use styrene_identity_ui::{IdentityPage, OverviewClient};

mod form;
mod mock_host;
pub mod service;

#[derive(Clone, Default)]
pub struct AppConfig {
    pub store: Option<PathBuf>,
    pub extension_demo: bool,
}
const STYLES: Asset = asset!("/assets/desktop.css");
pub const BUILD_REVISION: &str = env!("IDENTITY_BUILD_REVISION");

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Overview,
    Lifecycle,
    Backups,
    Recovery,
}

#[component]
pub fn App() -> Element {
    let config = try_use_context::<AppConfig>().unwrap_or_default();
    if config.extension_demo {
        return rsx! {mock_host::MockHost{}};
    }
    let initial = config.store.clone();
    let mut store_text = use_signal(move || {
        initial.as_ref().map(|path| path.display().to_string()).unwrap_or_default()
    });
    let mut store = use_signal(move || config.store.clone());
    let service = use_hook(service::Service::start);
    let mut catalog = use_signal(|| None::<Arc<CatalogSnapshot>>);
    let mut inventory = use_signal(|| None::<IdentityInventory>);
    let mut selected = use_signal(String::new);
    let mut generation = use_signal(|| 0u64);
    let mut page = use_signal(|| Page::Overview);
    let mut busy = use_signal(|| false);
    let mut message =
        use_signal(|| "Choose a store to inspect public identity information.".to_owned());
    let mut details = use_signal(String::new);
    let mut operations = use_signal(Vec::new);
    let mut backups = use_signal(Vec::new);
    let mut last_operation = use_signal(|| None::<String>);
    let mut form_epoch = use_signal(|| 0u64);
    let events = service.events.clone();
    use_future(move || {
        let events = events.clone();
        async move {
            while let Ok(event) = events.recv().await {
                match event {
                    service::Event::Snapshot { store: observed, result: Ok(snapshot) } => {
                        if store.peek().as_ref() != Some(&observed) {
                            continue;
                        }
                        let old = selected.peek().clone();
                        if !snapshot.inventory.entries.iter().any(|entry| entry.entry_id == old) {
                            selected.set(
                                snapshot
                                    .inventory
                                    .entries
                                    .first()
                                    .map(|entry| entry.entry_id.clone())
                                    .unwrap_or_default(),
                            );
                        }
                        let next = generation.peek().checked_add(1);
                        if let Some(next) = next {
                            generation.set(next);
                            catalog.set(Some(snapshot.catalog));
                        } else {
                            catalog.set(None);
                        }
                        inventory.set(Some(snapshot.inventory));
                        operations.set(snapshot.operations);
                        backups.set(snapshot.backups);
                        if !*busy.peek()
                            && last_operation.peek().is_none()
                            && details.peek().is_empty()
                        {
                            message.set(
                                "Public catalog loaded. Custody has not been unlocked.".into(),
                            );
                        }
                    }
                    service::Event::Snapshot { store: observed, result: Err(error) } => {
                        if store.peek().as_ref() != Some(&observed) {
                            continue;
                        }
                        catalog.set(None);
                        inventory.set(None);
                        operations.set(vec![]);
                        backups.set(vec![]);
                        if last_operation.peek().is_none() {
                            message.set(error.to_string());
                        }
                    }
                    service::Event::Completed(result) => {
                        busy.set(false);
                        match result {
                            Ok(outcome) => {
                                last_operation.set(outcome.operation_id().map(str::to_owned));
                                message.set("Backend operation completed. Inspect the recorded effects below.".into());
                                details.set(serde_json::to_string_pretty(&outcome).unwrap_or_else(
                                    |_| "Unable to render operation details".into(),
                                ));
                                let next = form_epoch.peek().checked_add(1);
                                if let Some(next) = next {
                                    form_epoch.set(next);
                                }
                            }
                            Err(failure) => {
                                last_operation.set(failure.operation_id.clone());
                                message.set(if failure.operation_id.is_some() {
                                    format!(
                                        "{} — inspect the retained operation before retrying.",
                                        failure.error
                                    )
                                } else {
                                    failure.error.to_string()
                                });
                                details.set(String::new());
                            }
                        }
                    }
                }
            }
        }
    });
    let initial_service = service.clone();
    use_hook(move || {
        if let Some(path) = store.peek().clone() {
            let _ = initial_service.requests.try_send(service::Work::Refresh(path));
        }
    });
    let refresh = service.clone();
    let dispatch = service.clone();
    let submit = EventHandler::new(move |request: service::Request| {
        if *busy.peek() {
            return;
        }
        let operation = match &request.action {
            service::Action::Identity { request_id, .. } => Some(format!("op-{request_id}")),
            service::Action::Backup { request_id, .. } => Some(format!("backup-op-{request_id}")),
            service::Action::Reconcile { operation, .. } => Some(operation.clone()),
            _ => None,
        };
        match dispatch.requests.try_send(service::Work::Execute(request)) {
            Ok(()) => {
                busy.set(true);
                last_operation.set(operation);
                message.set(
                    "Operation queued on the application worker. Navigation does not cancel it."
                        .into(),
                );
                details.set(String::new());
            }
            Err(_) => message.set("The application worker is busy or unavailable.".into()),
        }
    });
    let overview = catalog.read().as_ref().and_then(|snapshot| {
        let name = selected.read().clone();
        let selection = snapshot.selection(&name).ok()?;
        let entry = snapshot.show(&name, None).ok()?;
        let expected = match entry.identity {
            PublicIdentityView::HashConsistent { identity_id, .. } => identity_id.parse().ok(),
            _ => None,
        };
        Some((
            OverviewClient::new(snapshot.clone()),
            OverviewRequest {
                selection,
                scope: OverviewScope(*generation.read()),
                expected_identity: expected,
            },
        ))
    });
    rsx! {
        document::Stylesheet {href:STYLES}
        main {class:"identity-app",
            header {class:"app-heading",div{p{class:"eyebrow","SELF-CONTAINED IDENTITY WORKSPACE"}h1{"Styrene Identity"}}code{class:"build-marker","dev {BUILD_REVISION}"}}
            section {class:"store-bar", "aria-label":"Identity store",
                label {r#for:"store-path","Identity store directory"}
                input {id:"store-path",value:"{store_text}",placeholder:"/absolute/path/to/identity-store",disabled:*busy.read(),oninput:move|event|store_text.set(event.value())}
                button {disabled:*busy.read(),onclick:move |_|{
                    let text=store_text.read().trim().to_owned();let path=PathBuf::from(text);
                    if !path.is_absolute(){message.set("Choose an absolute store path; no default identity is selected.".into());return}
                    store.set(Some(path.clone()));last_operation.set(None);catalog.set(None);selected.set(String::new());
                    message.set(if refresh.requests.try_send(service::Work::Refresh(path)).is_ok(){"Reading public catalog information…"}else{"The application worker is busy or unavailable."}.into());
                },"Open store"}
            }
            nav {class:"workspace-nav", "aria-label":"Identity workspace",
                for (target,title) in [(Page::Overview,"Overview"),(Page::Lifecycle,"Lifecycle"),(Page::Backups,"Backups"),(Page::Recovery,"Recovery")] {
                    button {"aria-pressed":*page.read()==target,onclick:move |_|page.set(target),"{title}"}
                }
            }
            section {class:"operation-status",role:"status","aria-live":"polite",p{"{message}"}
                if let Some(id)=last_operation.read().as_ref(){code{"Operation: {id}"}}
                if *busy.read(){p{"Keep this app open to observe completion. If interrupted, reopen this store and inspect Recovery."}}
            }
            div {class:"workspace-grid",
                aside {class:"catalog-list", "aria-label":"Registered identities",h2{"Identities"}
                    if let Some(inventory)=inventory.read().as_ref(){
                        p{class:"muted","Catalog revision {inventory.revision}"}
                        for entry in &inventory.entries {
                            {let id=entry.entry_id.clone();let label=entry.display_name.as_deref().unwrap_or("Unnamed identity").to_owned();rsx!{button {class:"entry-button","aria-pressed":*selected.read()==id,onclick:move |_|{selected.set(id.clone());let next=generation.peek().checked_add(1);if let Some(next)=next{generation.set(next);}else{catalog.set(None);}},strong{"{label}"}small{"{entry.entry_id}"}}}}
                        }
                        if inventory.entries.is_empty(){p{"No registered identities. Create or adopt one in Lifecycle."}}
                    }else{p{"No catalog loaded. Creation can initialize the selected store."}}
                }
                section {class:"workspace-content",
                    match *page.read(){
                        Page::Overview=>rsx!{
                            if let Some((client,request))=overview {IdentityPage {client,request}}
                            else {div{class:"empty-state",h2{"Identity information is not yet available"}p{"Open a store and select an identity. Creating an identity is an explicit lifecycle action."}}}
                        },
                        Page::Lifecycle=>rsx!{form::ActionForm {key:"lifecycle-{form_epoch}-{selected}",store:store.read().clone(),selected:selected.read().clone(),busy:*busy.read(),initial:"create".to_owned(),onsubmit:submit}},
                        Page::Backups=>rsx!{
                            section {key:"backups-{form_epoch}-{selected}",
                            h2{"Managed encrypted backups"}
                            for artifact in backups.read().iter(){article{class:"backup-card",strong{"{artifact.artifact_id}"}code{"{artifact.identity_id}"}p{"{artifact.location.display()}"}small{"Recorded digest: {artifact.sha256}"}}}
                            form::ActionForm {store:store.read().clone(),selected:selected.read().clone(),busy:*busy.read(),initial:"export".to_owned(),onsubmit:submit}
                            }
                        },
                        Page::Recovery=>rsx!{
                            section {key:"recovery-{form_epoch}-{selected}",
                            h2{"Pending operations"}p{"Observation is independent of page navigation. No operation is replayed automatically."}
                            for operation in operations.read().iter(){article{class:"backup-card",code{"{operation.id()}"}}}
                            form::ActionForm {store:store.read().clone(),selected:selected.read().clone(),busy:*busy.read(),initial:"reconcile".to_owned(),onsubmit:submit}
                            }
                        },
                    }
                }
            }
            if !details.read().is_empty(){details{class:"result-details",open:true,summary{"Typed backend outcome"}pre{"{details}"}}}
        }
    }
}
