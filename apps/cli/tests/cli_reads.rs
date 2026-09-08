use std::process::{Command, Output, Stdio};
use std::time::Duration;

use serde_json::{Value, json};
use styrene_identity::IdentityId;
use wait_timeout::ChildExt;

fn run(command: &mut Command) -> Output {
    let mut child =
        command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    if child.wait_timeout(Duration::from_secs(10)).unwrap().is_none() {
        child.kill().unwrap();
        child.wait().unwrap();
        panic!("CLI exceeded its process-test deadline");
    }
    child.wait_with_output().unwrap()
}

fn invoke(store: &std::path::Path, args: &[&str]) -> Output {
    run(Command::new(env!("CARGO_BIN_EXE_idctl"))
        .args(["--output", "json", "--non-interactive", "--store"])
        .arg(store)
        .args(args))
}

fn envelope(output: &Output, code: i32) -> Value {
    assert_eq!(output.status.code(), Some(code), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let value: Value = serde_json::from_slice(&output.stdout).expect("one complete JSON envelope");
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["effects"]["custody"], "not_accessed");
    assert_eq!(value["effects"]["catalog"], "unchanged");
    assert!(value["operation_id"].is_null());
    assert_eq!(value["state"], if code == 0 { "completed" } else { "failed" });
    value
}

fn entry(id: &str) -> Value {
    json!({
        "entry_id": id, "revision": 3, "display_name": "Example",
        "public_identity": {
            "identity_id": IdentityId::from_public_key(&[42; 32]).to_string(),
            "public_key": "2a".repeat(32)
        },
        "custody_refs": ["offline-custody"]
    })
}

fn write_catalog(dir: &std::path::Path, entries: Vec<Value>) -> Vec<u8> {
    let bytes = serde_json::to_vec(&json!({
        "schema_version": 1, "revision": 7, "preferred_entry": null, "entries": entries,
    }))
    .unwrap();
    std::fs::write(dir.join("catalog.json"), &bytes).unwrap();
    bytes
}

#[test]
fn id01_capabilities_needs_no_store_or_mesh() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("absent");
    let value = envelope(&invoke(&missing, &["capabilities"]), 0);
    assert_eq!(
        value["result"]["mutations_supported"],
        cfg!(any(target_os = "linux", target_os = "android", target_vendor = "apple"))
    );
    for operation in ["capabilities", "identity.list", "identity.show"] {
        assert!(value["result"]["supported"].as_array().unwrap().contains(&json!(operation)));
    }
    assert!(!missing.exists());
    let output =
        run(Command::new(env!("CARGO_BIN_EXE_idctl")).args(["--output=json", "capabilities"]));
    envelope(&output, 0);
}

#[test]
fn id02_missing_empty_invalid_unavailable_and_future_stores_have_distinct_errors() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        envelope(&invoke(dir.path(), &["identity", "list"]), 3)["error"]["code"],
        "catalog_uninitialized"
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    write_catalog(dir.path(), vec![]);
    assert_eq!(
        envelope(&invoke(dir.path(), &["identity", "list"]), 0)["result"]["entries"],
        json!([])
    );
    let path = dir.path().join("catalog.json");
    std::fs::write(&path, b"not json PRIVATE-MARKER").unwrap();
    let invalid = invoke(dir.path(), &["identity", "list"]);
    assert_eq!(envelope(&invalid, 2)["error"]["code"], "invalid_catalog");
    assert!(!String::from_utf8_lossy(&invalid.stdout).contains("PRIVATE-MARKER"));
    std::fs::write(&path, br#"{"schema_version":99}"#).unwrap();
    assert_eq!(
        envelope(&invoke(dir.path(), &["identity", "list"]), 4)["error"]["code"],
        "unsupported_schema"
    );
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert_eq!(
        envelope(&invoke(dir.path(), &["identity", "list"]), 4)["error"]["code"],
        "catalog_unavailable"
    );
}

#[test]
fn id03_show_binds_identity_and_remains_read_only_across_processes() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = write_catalog(dir.path(), vec![entry("example")]);
    let expected = IdentityId::from_public_key(&[42; 32]).to_string();
    for _ in 0..2 {
        let value = envelope(
            &invoke(dir.path(), &["identity", "show", "example", "--expect-identity", &expected]),
            0,
        );
        assert_eq!(value["command"], "identity.show");
        assert_eq!(value["result"]["identity"]["identity_id"], expected);
        assert_eq!(value["result"]["identity"]["status"], "hash_consistent");
        assert_eq!(value["result"]["root_exposure"], "unknown");
        assert_eq!(value["result"]["provider_availability"], "unknown");
        assert_eq!(value["result"]["custody_refs"], json!(["offline-custody"]));
    }
    let wrong = IdentityId::from_public_key(&[43; 32]).to_string();
    assert_eq!(
        envelope(
            &invoke(dir.path(), &["identity", "show", "example", "--expect-identity", &wrong]),
            6
        )["error"]["code"],
        "identity_mismatch"
    );
    assert_eq!(std::fs::read(dir.path().join("catalog.json")).unwrap(), bytes);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn id03_unknown_entry_ambiguous_name_missing_identity_and_bad_binding() {
    let dir = tempfile::tempdir().unwrap();
    write_catalog(dir.path(), vec![entry("one"), entry("two")]);
    assert_eq!(
        envelope(&invoke(dir.path(), &["identity", "show", "absent"]), 3)["error"]["code"],
        "entry_not_found"
    );
    assert_eq!(
        envelope(&invoke(dir.path(), &["identity", "show", "Example"]), 6)["error"]["code"],
        "ambiguous_name"
    );
    let mut value = entry("unknown");
    value["public_identity"] = Value::Null;
    write_catalog(dir.path(), vec![value]);
    assert_eq!(
        envelope(&invoke(dir.path(), &["identity", "show", "unknown"]), 0)["result"]["identity"],
        json!({"status":"unavailable", "reason":"not_known"})
    );
    let mut bad = entry("bad");
    bad["public_identity"]["identity_id"] = json!("00".repeat(16));
    write_catalog(dir.path(), vec![bad]);
    assert_eq!(
        envelope(&invoke(dir.path(), &["identity", "list"]), 2)["error"]["code"],
        "invalid_catalog"
    );
}

#[test]
fn cli_usage_errors_are_json_and_do_not_echo_arguments() {
    let dir = tempfile::tempdir().unwrap();
    let output =
        invoke(dir.path(), &["identity", "show", "example", "--expect-identity", "PRIVATE-MARKER"]);
    assert_eq!(envelope(&output, 2)["error"]["code"], "invalid_arguments");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("PRIVATE-MARKER"));
    let output =
        run(Command::new(env!("CARGO_BIN_EXE_idctl")).args(["--output=json", "identity", "list"]));
    assert_eq!(envelope(&output, 2)["error"]["code"], "store_required");
}

#[test]
fn cli_help_and_version_are_available_without_store() {
    for argument in ["--help", "--version"] {
        let output = run(Command::new(env!("CARGO_BIN_EXE_idctl")).arg(argument));
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("idctl"));
    }
}

#[test]
fn cli_human_output_labels_identity_and_errors_use_stderr() {
    let dir = tempfile::tempdir().unwrap();
    write_catalog(dir.path(), vec![entry("example")]);
    let output = run(Command::new(env!("CARGO_BIN_EXE_idctl"))
        .arg("--store")
        .arg(dir.path())
        .args(["identity", "show", "example"]));
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Styrene Identity ID:"));
    assert!(text.contains("unknown (custody not accessed)"));
    let output = run(Command::new(env!("CARGO_BIN_EXE_idctl"))
        .arg("--store")
        .arg(dir.path())
        .args(["identity", "show", "missing"]));
    assert_eq!(output.status.code(), Some(3));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("entry_not_found"));
}

#[test]
fn id02_oversized_file_is_rejected_without_reading_or_modifying_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("catalog.json");
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(styrene_identity_lifecycle::MAX_CATALOG_BYTES + 1).unwrap();
    let value = envelope(&invoke(dir.path(), &["identity", "list"]), 2);
    assert_eq!(value["error"]["code"], "catalog_too_large");
    assert_eq!(
        std::fs::metadata(path).unwrap().len(),
        styrene_identity_lifecycle::MAX_CATALOG_BYTES + 1
    );
}

#[cfg(unix)]
#[test]
fn id02_symlink_catalog_is_not_followed() {
    let dir = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let original = write_catalog(other.path(), vec![entry("example")]);
    std::os::unix::fs::symlink(other.path().join("catalog.json"), dir.path().join("catalog.json"))
        .unwrap();
    let value = envelope(&invoke(dir.path(), &["identity", "list"]), 4);
    assert_eq!(value["error"]["code"], "catalog_unavailable");
    assert_eq!(std::fs::read(other.path().join("catalog.json")).unwrap(), original);
}

#[test]
fn id31_inspection_needs_no_store_or_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("unauthenticated.stid");
    let mut bytes = [0; 97];
    bytes[..5].copy_from_slice(b"STID\x01");
    std::fs::write(&path, bytes).unwrap();
    let output = run(Command::new(env!("CARGO_BIN_EXE_idctl"))
        .args(["--output=json", "backup", "inspect"])
        .arg(&path));
    let result = envelope(&output, 0);
    assert_eq!(result["result"]["payload_authenticated"], false);
    assert!(result["result"].get("identity_id").is_none());
}
