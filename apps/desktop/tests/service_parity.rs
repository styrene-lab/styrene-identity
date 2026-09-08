use dioxus::prelude::*;
use std::time::Duration;
use styrene_identity_desktop::service::{self, Action, Event, Outcome, Request, Work};
use styrene_identity_lifecycle::{
    CatalogSnapshot,
    mutations::{Mutation, artifacts::BackupMutation},
};
use zeroize::Zeroizing;

#[tokio::test]
async fn standalone_worker_runs_the_same_backend_create_backup_restore_slice() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("store");
    let key = dir.path().join("identity.key");
    let service = service::Service::start();
    service
        .requests
        .try_send(Work::Execute(Request {
            store: Some(store.clone()),
            action: Action::Identity {
                request_id: "ui-create".into(),
                mutation: Mutation::Create { name: "UI fixture".into(), destination: key.clone() },
                protection: Zeroizing::new(b"source-protection".to_vec()),
            },
        }))
        .unwrap_or_else(|_| panic!("worker queue"));
    let completed = tokio::time::timeout(Duration::from_secs(60), service.events.recv())
        .await
        .unwrap()
        .unwrap();
    let Event::Completed(Ok(Outcome::Identity(created))) = completed else {
        panic!("create outcome")
    };
    let expected = created.identity_id.unwrap().parse().unwrap();
    // The root worker, not a page, retains and publishes completion.
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(10), service.events.recv())
            .await
            .unwrap()
            .unwrap(),
        Event::Snapshot { .. }
    ));
    let backup = dir.path().join("backup.stid");
    let exported = service::perform(Request {
        store: Some(store.clone()),
        action: Action::Backup {
            request_id: "ui-backup".into(),
            mutation: BackupMutation::Export {
                entry_id: created.entry_id,
                output: backup.clone(),
                expected_identity: expected,
            },
            source: Zeroizing::new(b"source-protection".to_vec()),
            destination: Zeroizing::new(b"backup-protection".to_vec()),
        },
    })
    .unwrap_or_else(|_| panic!("export outcome"));
    assert!(matches!(exported, Outcome::Backup(_)));
    let destination = dir.path().join("restored.key");
    let restored_store = dir.path().join("restored");
    service::perform(Request {
        store: Some(restored_store.clone()),
        action: Action::Backup {
            request_id: "ui-restore".into(),
            mutation: BackupMutation::Restore {
                input: backup,
                destination,
                name: "Restored".into(),
                expected_identity: expected,
            },
            source: Zeroizing::new(b"backup-protection".to_vec()),
            destination: Zeroizing::new(b"new-protection".to_vec()),
        },
    })
    .unwrap_or_else(|_| panic!("restore outcome"));
    assert_eq!(CatalogSnapshot::load(&restored_store).unwrap().list().entries.len(), 1);
    assert!(key.exists());
}

#[test]
fn standalone_shell_has_named_navigation_and_no_implicit_store_initialization() {
    let html = dioxus_ssr::render_element(rsx! {styrene_identity_desktop::App{}});
    assert!(html.contains("Styrene Identity"));
    assert!(html.contains("Identity store directory"));
    assert!(html.contains("Recovery"));
    assert!(html.contains("No catalog loaded"));
    assert!(!html.contains("type=\"password\""));
}
