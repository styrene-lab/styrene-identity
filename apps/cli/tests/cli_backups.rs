#![cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]

use serde_json::Value;
use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};
use wait_timeout::ChildExt;

fn run(store: &Path, args: &[&str], secrets: Option<&[u8]>, code: i32) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_idctl"))
        .args(["--output=json", "--non-interactive", "--store"])
        .arg(store)
        .args(args)
        .stdin(if secrets.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(secrets) = secrets {
        child.stdin.take().unwrap().write_all(secrets).unwrap();
    }
    if child.wait_timeout(Duration::from_secs(60)).unwrap().is_none() {
        child.kill().unwrap();
        child.wait().unwrap();
        panic!("backup command exceeded deadline")
    }
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(code), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!text.contains("secret-source"));
    assert!(!text.contains("secret-backup"));
    assert!(!text.contains("secret-new"));
    serde_json::from_str(&text).unwrap()
}

#[test]
fn id30_through_id36_cli_backup_lifecycle_uses_distinct_protection_roles() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("store");
    let key = dir.path().join("identity.key");
    let created = run(
        &store,
        &[
            "identity",
            "create",
            "--name",
            "Operator",
            "--destination",
            key.to_str().unwrap(),
            "--request-id",
            "source",
            "--passphrase-stdin",
        ],
        Some(b"secret-source\n"),
        0,
    );
    let identity = created["result"]["identity_id"].as_str().unwrap();
    let backup = dir.path().join("backup.stid");
    let exported = run(
        &store,
        &[
            "backup",
            "export",
            "identity-source",
            "--output-file",
            backup.to_str().unwrap(),
            "--expect-identity",
            identity,
            "--request-id",
            "export",
            "--protection-stdin",
        ],
        Some(b"secret-source\nsecret-backup\n"),
        0,
    );
    assert_eq!(exported["operation_id"], "backup-op-export");
    run(
        &store,
        &[
            "backup",
            "verify",
            backup.to_str().unwrap(),
            "--expect-identity",
            identity,
            "--passphrase-stdin",
        ],
        Some(b"secret-backup\n"),
        0,
    );
    run(
        &store,
        &["backup", "verify", backup.to_str().unwrap(), "--passphrase-stdin"],
        Some(b"secret-source\n"),
        5,
    );
    let reprotected = dir.path().join("reprotected.stid");
    let updated = run(
        &store,
        &[
            "backup",
            "reprotect",
            backup.to_str().unwrap(),
            "--output-file",
            reprotected.to_str().unwrap(),
            "--expect-identity",
            identity,
            "--request-id",
            "reprotect",
            "--protection-stdin",
        ],
        Some(b"secret-backup\nsecret-new\n"),
        0,
    );
    assert!(backup.exists());
    let restored_store = dir.path().join("restored");
    let restored_key = dir.path().join("restored.key");
    let restored = run(
        &restored_store,
        &[
            "backup",
            "restore",
            reprotected.to_str().unwrap(),
            "--destination",
            restored_key.to_str().unwrap(),
            "--name",
            "Restored",
            "--expect-identity",
            identity,
            "--request-id",
            "restore",
            "--protection-stdin",
        ],
        Some(b"secret-new\nsecret-source\n"),
        0,
    );
    assert!(restored["result"]["entry_id"].is_string());
    assert_eq!(
        run(&restored_store, &["identity", "list"], None, 0)["result"]["entries"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let digest = updated["result"]["sha256"].as_str().unwrap();
    run(
        &store,
        &[
            "backup",
            "delete",
            "backup-reprotect",
            "--expect-digest",
            digest,
            "--request-id",
            "delete",
        ],
        None,
        0,
    );
    assert!(!reprotected.exists());
    assert!(restored_key.exists());
    let first_digest = exported["result"]["sha256"].as_str().unwrap();
    run(
        &store,
        &[
            "backup",
            "forget",
            "backup-export",
            "--expect-digest",
            first_digest,
            "--request-id",
            "forget",
        ],
        None,
        0,
    );
    assert!(backup.exists());
    assert!(key.exists());
    assert!(run(&store, &["backup", "list"], None, 0)["result"].as_array().unwrap().is_empty());
    assert_eq!(
        run(&store, &["backup", "show", "backup-export"], None, 0)["result"]["status"],
        "forgotten"
    );
}

#[test]
fn backup_cli_rejects_ambiguous_credentials_and_never_overwrites_existing_output() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("store");
    let key = dir.path().join("identity.key");
    let created = run(
        &store,
        &[
            "identity",
            "create",
            "--name",
            "Operator",
            "--destination",
            key.to_str().unwrap(),
            "--request-id",
            "source",
            "--passphrase-stdin",
        ],
        Some(b"secret-source\n"),
        0,
    );
    let identity = created["result"]["identity_id"].as_str().unwrap();
    let output = dir.path().join("output");
    run(
        &store,
        &[
            "backup",
            "export",
            "identity-source",
            "--output-file",
            output.to_str().unwrap(),
            "--expect-identity",
            identity,
            "--request-id",
            "bad-input",
            "--protection-stdin",
        ],
        Some(b"secret-source\n"),
        2,
    );
    assert!(!output.exists());
    std::fs::write(&output, b"existing").unwrap();
    let result = run(
        &store,
        &[
            "backup",
            "export",
            "identity-source",
            "--output-file",
            output.to_str().unwrap(),
            "--expect-identity",
            identity,
            "--request-id",
            "conflict",
            "--protection-stdin",
        ],
        Some(b"secret-source\nsecret-backup\n"),
        6,
    );
    assert_eq!(result["error"]["code"], "destination_conflict");
    assert_eq!(std::fs::read(output).unwrap(), b"existing");
}
