use styrene_identity_ui::{ExtensionError, IDENTITY_EXTENSION, IdentityExtension};

#[test]
fn optional_registration_and_lifecycle_do_not_duplicate_or_accept_stale_results() {
    let mut host = IdentityExtension::new(true, IDENTITY_EXTENSION, 1, None);
    assert!(host.registered());
    let first = host.mount().unwrap();
    assert_eq!(host.mount().unwrap(), first);
    host.enable().unwrap();
    assert_eq!(host.generation(), first);
    host.unmount().unwrap();
    assert!(!host.accepts_view_result(first));
    let next = host.mount().unwrap();
    assert_ne!(first, next);
    host.session_changed().unwrap();
    assert!(!host.accepts_view_result(next));
    host.disable(false, false).unwrap();
    assert!(!host.registered());
    host.enable().unwrap();
    assert!(host.registered());
    assert!(!host.mounted());
}

#[test]
fn pending_work_requires_independent_observation_before_disable() {
    let mut host = IdentityExtension::new(true, IDENTITY_EXTENSION, 1, None);
    let scope = host.mount().unwrap();
    assert_eq!(host.disable(true, false), Err(ExtensionError::PendingOperation));
    assert!(host.accepts_view_result(scope));
    host.disable(true, true).unwrap();
    assert!(!host.accepts_view_result(scope));
    let restarted =
        IdentityExtension::new(true, IDENTITY_EXTENSION, 1, Some(host.enabled_preference()));
    assert!(!restarted.registered());
    assert!(!restarted.mounted());
}

#[test]
fn missing_or_incompatible_component_cannot_activate() {
    assert!(!IdentityExtension::new(false, IDENTITY_EXTENSION, 1, None).registered());
    assert!(!IdentityExtension::new(true, IDENTITY_EXTENSION, 2, None).registered());
    let mut descriptor = IDENTITY_EXTENSION;
    descriptor.page_id = "unexpected";
    assert!(!IdentityExtension::new(true, descriptor, 1, None).registered());
    let mut compatible = IDENTITY_EXTENSION;
    compatible.build_version = "0.2.0";
    assert!(IdentityExtension::new(true, compatible, 1, None).registered());
}
