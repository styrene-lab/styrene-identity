use serde_json::{Value, json};
use styrene_identity::IdentityId;
use styrene_identity::overview::{
    OverviewRequest, OverviewScope, ProviderAvailability, PublicIdentityStatus, RootExposure,
    read_identity_overview,
};
use styrene_identity_lifecycle::{CatalogSnapshot, LifecycleError, MAX_CATALOG_BYTES};

fn entry(id: &str) -> Value {
    json!({
        "entry_id": id, "revision": 1, "display_name": "Test",
        "public_identity": {
            "identity_id": IdentityId::from_public_key(&[42; 32]).to_string(),
            "public_key": "2a".repeat(32)
        },
        "custody_refs": ["custody-test"]
    })
}

fn catalog(entries: Vec<Value>) -> Value {
    json!({"schema_version": 1, "revision": 2, "preferred_entry": null, "entries": entries})
}

fn load(value: &Value) -> Result<CatalogSnapshot, LifecycleError> {
    CatalogSnapshot::from_bytes(&serde_json::to_vec(value).unwrap())
}

fn assert_error(value: &Value, error: LifecycleError) {
    assert!(matches!(load(value), Err(actual) if actual == error));
}

#[test]
fn id02_empty_invalid_duplicate_and_future_catalogs_remain_distinct() {
    assert!(load(&catalog(vec![])).unwrap().list().entries.is_empty());
    assert_error(&json!({"schema_version": 2}), LifecycleError::UnsupportedSchema);
    assert_error(&catalog(vec![entry("same"), entry("same")]), LifecycleError::InvalidCatalog);
    let mut value = catalog(vec![entry("one")]);
    value["preferred_entry"] = json!("missing");
    assert_error(&value, LifecycleError::InvalidCatalog);
    assert!(matches!(
        CatalogSnapshot::from_bytes(
            br#"{"schema_version":1,"schema_version":1,"revision":0,"entries":[]}"#
        ),
        Err(LifecycleError::InvalidCatalog)
    ));
    assert!(matches!(
        CatalogSnapshot::from_bytes(&vec![b' '; MAX_CATALOG_BYTES as usize + 1]),
        Err(LifecycleError::CatalogTooLarge)
    ));
}

#[test]
fn id03_validate_every_public_binding_before_returning_inventory() {
    let mut malformed = entry("bad");
    malformed["public_identity"]["identity_id"] = json!("00".repeat(16));
    assert_error(&catalog(vec![entry("good"), malformed]), LifecycleError::InvalidCatalog);
    let mut uppercase = entry("upper");
    uppercase["public_identity"]["public_key"] = json!("2A".repeat(32));
    assert_error(&catalog(vec![uppercase]), LifecycleError::InvalidCatalog);
}

#[test]
fn id03_exact_id_wins_and_ambiguous_names_are_rejected() {
    let snapshot = load(&catalog(vec![entry("z"), entry("a")])).unwrap();
    assert_eq!(snapshot.list().entries[0].entry_id, "a");
    assert_eq!(snapshot.show("a", None).unwrap().entry_id, "a");
    assert!(matches!(snapshot.show("Test", None), Err(LifecycleError::AmbiguousName)));
    assert!(matches!(snapshot.show("missing", None), Err(LifecycleError::EntryNotFound)));
    let mut named = entry("second");
    named["display_name"] = json!("a");
    let snapshot = load(&catalog(vec![entry("a"), named])).unwrap();
    assert_eq!(snapshot.show("a", None).unwrap().entry_id, "a");
}

#[tokio::test]
async fn id03_snapshot_serves_existing_overview_contract_without_custody_io() {
    let snapshot = load(&catalog(vec![entry("one")])).unwrap();
    let expected = IdentityId::from_public_key(&[42; 32]);
    let request = OverviewRequest {
        selection: snapshot.selection("one").unwrap(),
        scope: OverviewScope(1),
        expected_identity: Some(expected),
    };
    let overview = read_identity_overview(&snapshot, request)
        .await
        .into_result_for(Some(request))
        .unwrap()
        .unwrap();
    assert_eq!(overview.root_exposure, RootExposure::Unknown);
    assert_eq!(overview.availability, ProviderAvailability::Unknown);
    assert!(
        matches!(overview.identity, PublicIdentityStatus::Available(identity) if identity.identity_id() == expected)
    );
    assert!(matches!(
        snapshot.show("one", Some(IdentityId::from_public_key(&[43; 32]))),
        Err(LifecycleError::IdentityMismatch)
    ));
}

#[test]
fn id03_absent_identity_is_not_a_transport_identity_or_expected_match() {
    let mut absent = entry("unknown");
    absent["public_identity"] = Value::Null;
    let snapshot = load(&catalog(vec![absent])).unwrap();
    let output = serde_json::to_value(snapshot.show("unknown", None).unwrap()).unwrap();
    assert_eq!(output["identity"]["status"], "unavailable");
    assert!(matches!(
        snapshot.show("unknown", Some(IdentityId::from_public_key(&[42; 32]))),
        Err(LifecycleError::IdentityUnavailable)
    ));
}

#[test]
fn id02_load_never_creates_store_and_preserves_input_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("not-created");
    assert!(matches!(CatalogSnapshot::load(&missing), Err(LifecycleError::CatalogUninitialized)));
    assert!(!missing.exists());
    let bytes = serde_json::to_vec(&catalog(vec![entry("one")])).unwrap();
    std::fs::write(dir.path().join("catalog.json"), &bytes).unwrap();
    CatalogSnapshot::load(dir.path()).unwrap();
    assert_eq!(std::fs::read(dir.path().join("catalog.json")).unwrap(), bytes);
}
