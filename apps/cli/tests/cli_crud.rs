#![cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{Value, json};
use wait_timeout::ChildExt;

const PROTECTION: &[u8] = b"temporary-CLI-secret-marker\n";

fn invoke(store: &Path, args: &[&str], protection: Option<&[u8]>, code: i32) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_idctl"))
        .args(["--output=json", "--non-interactive", "--store"])
        .arg(store)
        .args(args)
        .stdin(if protection.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(protection) = protection {
        child.stdin.take().unwrap().write_all(protection).unwrap();
    }
    if child.wait_timeout(Duration::from_secs(30)).unwrap().is_none() {
        child.kill().unwrap();
        child.wait().unwrap();
        panic!("CLI mutation timed out");
    }
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(code), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("temporary-CLI-secret-marker"));
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schema_version"], 1);
    value
}

fn create(store: &Path, custody: &Path, request: &str) -> Value {
    invoke(
        store,
        &[
            "identity",
            "create",
            "--name",
            "Original",
            "--destination",
            custody.to_str().unwrap(),
            "--request-id",
            request,
            "--passphrase-stdin",
        ],
        Some(PROTECTION),
        0,
    )
}

#[test]
fn id04_id08_full_cli_crud_and_id05_adoption_preserve_identity_and_custody() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("store");
    let custody = dir.path().join("custody.key");
    let first = create(&store, &custody, "first");
    assert_eq!(first["operation_id"], "op-first");
    assert_eq!(first["effects"]["custody"], "created");
    let entry = first["result"]["entry_id"].as_str().unwrap();
    let identity = first["result"]["identity_id"].as_str().unwrap();
    let bytes = std::fs::read(&custody).unwrap();
    invoke(&store, &["identity", "show", entry, "--expect-identity", identity], None, 0);
    let updated = invoke(
        &store,
        &[
            "identity",
            "update",
            entry,
            "--name",
            "Renamed",
            "--if-revision",
            "1",
            "--request-id",
            "rename",
        ],
        None,
        0,
    );
    assert_eq!(updated["result"]["identity_id"], identity);
    assert_eq!(updated["result"]["entry_revision"], 2);
    let stale = invoke(
        &store,
        &[
            "identity",
            "update",
            entry,
            "--clear-name",
            "--if-revision",
            "1",
            "--request-id",
            "stale",
        ],
        None,
        6,
    );
    assert_eq!(stale["error"]["code"], "revision_conflict");
    invoke(
        &store,
        &["identity", "select", entry, "--if-revision", "2", "--request-id", "select"],
        None,
        0,
    );
    let listed = invoke(&store, &["identity", "list"], None, 0);
    assert_eq!(listed["result"]["preferred_entry"], entry);
    let removed = invoke(
        &store,
        &["identity", "forget", entry, "--if-revision", "2", "--request-id", "forget"],
        None,
        0,
    );
    assert_eq!(removed["effects"]["custody"], "not_accessed");
    assert_eq!(std::fs::read(&custody).unwrap(), bytes);
    let listed = invoke(&store, &["identity", "list"], None, 0);
    assert_eq!(listed["result"]["entries"], json!([]));
    assert!(listed["result"]["preferred_entry"].is_null());
    // A completed create retry returns its historical result even after forgetting.
    let replay = invoke(
        &store,
        &[
            "identity",
            "create",
            "--name",
            "Original",
            "--destination",
            custody.to_str().unwrap(),
            "--request-id",
            "first",
        ],
        None,
        0,
    );
    assert_eq!(replay["result"]["identity_id"], first["result"]["identity_id"]);
    assert_eq!(replay["result"]["replayed"], true);
    assert_eq!(invoke(&store, &["identity", "list"], None, 0)["result"]["entries"], json!([]));
    let adopted = invoke(
        &store,
        &[
            "identity",
            "adopt",
            "--name",
            "Adopted",
            "--custody",
            custody.to_str().unwrap(),
            "--expect-identity",
            identity,
            "--request-id",
            "adopt",
            "--passphrase-stdin",
        ],
        Some(PROTECTION),
        0,
    );
    assert_eq!(adopted["result"]["identity_id"], identity);
    assert_eq!(adopted["effects"]["custody"], "authenticated_unchanged");
    assert_eq!(std::fs::read(custody).unwrap(), bytes);
}

#[test]
fn id04_credential_and_destination_failures_do_not_create_or_replace_identity() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("store");
    let custody = dir.path().join("custody.key");
    let missing = invoke(
        &store,
        &[
            "identity",
            "create",
            "--name",
            "Original",
            "--destination",
            custody.to_str().unwrap(),
            "--request-id",
            "create",
        ],
        None,
        5,
    );
    assert_eq!(missing["error"]["code"], "authentication_required");
    assert!(!store.exists());
    assert!(!custody.exists());
    let oversized = vec![b'x'; 4099];
    let invalid = invoke(
        &store,
        &[
            "identity",
            "create",
            "--name",
            "Original",
            "--destination",
            custody.to_str().unwrap(),
            "--request-id",
            "create",
            "--passphrase-stdin",
        ],
        Some(&oversized),
        2,
    );
    assert_eq!(invalid["error"]["code"], "invalid_request");
    assert!(!store.exists());
    std::fs::write(&custody, b"existing bytes").unwrap();
    let conflict = invoke(
        &store,
        &[
            "identity",
            "create",
            "--name",
            "Original",
            "--destination",
            custody.to_str().unwrap(),
            "--request-id",
            "create",
            "--passphrase-stdin",
        ],
        Some(PROTECTION),
        6,
    );
    assert_eq!(conflict["error"]["code"], "destination_conflict");
    assert_eq!(std::fs::read(custody).unwrap(), b"existing bytes");
    assert!(!store.join("catalog.json").exists());
}

#[test]
fn id44_id45_pending_create_observation_and_reconciliation_across_processes() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("store");
    let custody = dir.path().join("custody.key");
    let original = create(&store, &custody, "recover");
    let bytes = std::fs::read(&custody).unwrap();
    let journal_path = store.join("operations/op-recover.json");
    // Reconstruct the durable state after custody install but before catalog commit.
    // No production fault-injection switch is exposed by the CLI.
    let mut journal: Value =
        serde_json::from_slice(&std::fs::read(&journal_path).unwrap()).unwrap();
    journal["state"] = json!("prepared");
    journal["after"] =
        serde_json::from_slice(&std::fs::read(store.join("catalog.json")).unwrap()).unwrap();
    journal["encrypted_creation"] = json!(bytes);
    std::fs::write(&journal_path, serde_json::to_vec(&journal).unwrap()).unwrap();
    std::fs::remove_file(store.join("catalog.json")).unwrap();
    let pending = invoke(&store, &["operation", "show", "op-recover"], None, 0);
    assert_eq!(pending["result"]["state"], "prepared");
    assert_eq!(pending["result"]["effects"]["custody"], "unknown");
    assert!(pending["result"]["result"].is_null());
    assert!(pending["result"].get("encrypted_creation").is_none());
    let wrong = invoke(
        &store,
        &["operation", "reconcile", "op-recover", "--passphrase-stdin"],
        Some(b"wrong\n"),
        9,
    );
    assert_eq!(wrong["state"], "needs_reconciliation");
    assert_eq!(wrong["error"]["code"], "authentication_failed");
    assert_eq!(wrong["operation_id"], "op-recover");
    let recovered = invoke(
        &store,
        &["operation", "reconcile", "op-recover", "--passphrase-stdin"],
        Some(PROTECTION),
        0,
    );
    assert_eq!(recovered["result"]["identity_id"], original["result"]["identity_id"]);
    assert_eq!(std::fs::read(custody).unwrap(), bytes);
    assert_eq!(
        invoke(&store, &["operation", "show", "op-recover"], None, 0)["result"]["state"],
        "completed"
    );
}

#[test]
fn id04_cross_process_store_lock_and_request_conflict() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("store");
    let custody = dir.path().join("custody.key");
    create(&store, &custody, "create");
    let held = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(store.join(".mutation-lock"))
        .unwrap();
    held.try_lock().unwrap();
    let result = invoke(
        &store,
        &[
            "identity",
            "update",
            "identity-create",
            "--name",
            "Renamed",
            "--if-revision",
            "1",
            "--request-id",
            "update",
        ],
        None,
        6,
    );
    assert_eq!(result["error"]["code"], "store_busy");
    drop(held);
    let result = invoke(
        &store,
        &[
            "identity",
            "update",
            "identity-create",
            "--name",
            "Renamed",
            "--if-revision",
            "1",
            "--request-id",
            "create",
        ],
        None,
        6,
    );
    assert_eq!(result["error"]["code"], "request_conflict");
    invoke(
        &store,
        &[
            "identity",
            "update",
            "identity-create",
            "--clear-name",
            "--if-revision",
            "1",
            "--request-id",
            "update",
        ],
        None,
        0,
    );
    assert!(
        invoke(&store, &["identity", "show", "identity-create"], None, 0)["result"]["display_name"]
            .is_null()
    );
}

#[test]
fn id06_metadata_changes_require_explicit_set_or_clear_intent() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("store");
    let result = invoke(
        &store,
        &["identity", "update", "entry", "--if-revision", "1", "--request-id", "empty"],
        None,
        2,
    );
    assert_eq!(result["error"]["code"], "invalid_arguments");
    assert!(!store.exists());
    let result = invoke(
        &store,
        &[
            "identity",
            "update",
            "entry",
            "--name",
            "Name",
            "--clear-name",
            "--if-revision",
            "1",
            "--request-id",
            "both",
        ],
        None,
        2,
    );
    assert_eq!(result["error"]["code"], "invalid_arguments");
    assert!(!store.exists());
}

#[test]
fn id31_id32_backup_inspection_and_verification_preserve_artifacts_and_identity() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("store");
    let custody = dir.path().join("custody.key");
    let created = create(&store, &custody, "source");
    let expected = created["result"]["identity_id"].as_str().unwrap();
    let bytes = std::fs::read(&custody).unwrap();
    let unused_store = dir.path().join("not-initialized");
    let inspected =
        invoke(&unused_store, &["backup", "inspect", custody.to_str().unwrap()], None, 0);
    assert_eq!(inspected["result"]["payload_authenticated"], false);
    assert!(inspected["result"].get("identity_id").is_none());
    let verified = invoke(
        &unused_store,
        &[
            "backup",
            "verify",
            custody.to_str().unwrap(),
            "--expect-identity",
            expected,
            "--passphrase-stdin",
        ],
        Some(PROTECTION),
        0,
    );
    assert_eq!(verified["result"]["identity_id"], expected);
    assert_eq!(verified["result"]["artifact"]["payload_authenticated"], true);
    assert_eq!(verified["result"]["artifact"]["format_header_authenticated"], false);
    invoke(
        &unused_store,
        &["backup", "verify", custody.to_str().unwrap(), "--passphrase-stdin"],
        Some(b"wrong\n"),
        5,
    );
    let wrong_id = "00".repeat(16);
    invoke(
        &unused_store,
        &[
            "backup",
            "verify",
            custody.to_str().unwrap(),
            "--expect-identity",
            &wrong_id,
            "--passphrase-stdin",
        ],
        Some(PROTECTION),
        6,
    );
    assert_eq!(std::fs::read(custody).unwrap(), bytes);
    assert!(!unused_store.exists());
}
