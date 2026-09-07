use std::ffi::OsString;
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use serde_json::Value;
use styrene_identity::IdentityId;
use styrene_identity_lifecycle::backups;
use styrene_identity_lifecycle::mutations::artifacts::{
    self, BackupFailure, BackupMutation, BackupResult,
};
use styrene_identity_lifecycle::mutations::{
    self, Mutation, MutationFailure, MutationResult, NameChange,
};
use styrene_identity_lifecycle::{CatalogSnapshot, LifecycleError, capabilities};
use zeroize::Zeroizing;

#[derive(Clone, Copy, Default, ValueEnum)]
enum Output {
    #[default]
    Human,
    Json,
}

#[derive(Parser)]
#[command(name = "idctl", version, about = "Standalone Identity lifecycle client")]
struct Cli {
    /// Explicit local catalog directory. Required for identity commands.
    #[arg(long, global = true)]
    store: Option<PathBuf>,
    #[arg(long, global = true, value_enum, default_value = "human")]
    output: Output,
    /// Never prompt. Credential input is currently explicit non-terminal stdin only.
    #[arg(long, global = true)]
    non_interactive: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List implemented application operations without accessing storage or custody.
    Capabilities,
    /// Read or manage Identity catalog entries and file custody.
    Identity {
        #[command(subcommand)]
        command: IdentityCommand,
    },
    /// Inspect or reconcile durable local mutation outcomes.
    Operation {
        #[command(subcommand)]
        command: OperationCommand,
    },
    /// Inspect or authenticate a bounded encrypted backup without changing custody.
    Backup {
        #[command(subcommand)]
        command: BackupCommand,
    },
}

#[derive(Subcommand)]
enum IdentityCommand {
    List,
    Show {
        /// Exact entry ID, or an unambiguous display name.
        entry: String,
        #[arg(long)]
        expect_identity: Option<IdentityId>,
    },
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        destination: PathBuf,
        #[arg(long, value_enum, default_value = "file")]
        custody: CustodyKind,
        #[arg(long)]
        request_id: String,
        #[arg(long)]
        passphrase_stdin: bool,
    },
    Adopt {
        #[arg(long)]
        name: String,
        #[arg(long)]
        custody: PathBuf,
        #[arg(long)]
        expect_identity: IdentityId,
        #[arg(long)]
        request_id: String,
        #[arg(long)]
        passphrase_stdin: bool,
    },
    Update {
        entry: String,
        #[arg(long, required_unless_present = "clear_name", conflicts_with = "clear_name")]
        name: Option<String>,
        #[arg(long)]
        clear_name: bool,
        #[arg(long)]
        if_revision: u64,
        #[arg(long)]
        request_id: String,
    },
    Select {
        entry: String,
        /// Expected catalog revision (not the entry revision).
        #[arg(long)]
        if_revision: u64,
        #[arg(long)]
        request_id: String,
    },
    Forget {
        entry: String,
        /// Expected entry revision. Custody and backups are retained.
        #[arg(long)]
        if_revision: u64,
        #[arg(long)]
        request_id: String,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum CustodyKind {
    File,
}

#[derive(Subcommand)]
enum OperationCommand {
    List { #[arg(long)] all: bool },
    Show {
        operation: String,
    },
    Reconcile {
        operation: String,
        #[arg(long, conflicts_with = "protection_stdin")]
        passphrase_stdin: bool,
        #[arg(long)]
        protection_stdin: bool,
    },
}

#[derive(Subcommand)]
enum BackupCommand {
    Inspect {
        input: PathBuf,
    },
    Verify {
        input: PathBuf,
        #[arg(long)]
        expect_identity: Option<IdentityId>,
        #[arg(long)]
        passphrase_stdin: bool,
    },
    Export {
        entry: String,
        #[arg(long)]
        output_file: PathBuf,
        #[arg(long)]
        expect_identity: IdentityId,
        #[arg(long)]
        request_id: String,
        #[arg(long)]
        protection_stdin: bool,
    },
    Reprotect {
        input: PathBuf,
        #[arg(long)]
        output_file: PathBuf,
        #[arg(long)]
        expect_identity: IdentityId,
        #[arg(long)]
        request_id: String,
        #[arg(long)]
        protection_stdin: bool,
    },
    Restore {
        input: PathBuf,
        #[arg(long)]
        destination: PathBuf,
        #[arg(long)]
        name: String,
        #[arg(long)]
        expect_identity: IdentityId,
        #[arg(long)]
        request_id: String,
        #[arg(long)]
        protection_stdin: bool,
    },
    VerifyRecovery {
        operation: String,
        #[arg(long)]
        expect_identity: Option<IdentityId>,
        #[arg(long)]
        passphrase_stdin: bool,
    },
    MigrateRecovery {
        operation: String,
        #[arg(long)]
        output_file: PathBuf,
        #[arg(long)]
        expect_identity: IdentityId,
        #[arg(long)]
        request_id: String,
        #[arg(long)]
        protection_stdin: bool,
    },
    List {
        #[arg(long)]
        identity: Option<IdentityId>,
    },
    Show {
        artifact: String,
    },
    Forget {
        artifact: String,
        #[arg(long)]
        expect_digest: String,
        #[arg(long)]
        request_id: String,
    },
    Delete {
        artifact: String,
        #[arg(long)]
        expect_digest: String,
        #[arg(long)]
        request_id: String,
    },
}

struct Success {
    value: Value,
    operation_id: Option<String>,
    effects: Value,
}
struct ClientFailure {
    error: LifecycleError,
    operation_id: Option<String>,
    effects: Value,
}

impl From<LifecycleError> for ClientFailure {
    fn from(error: LifecycleError) -> Self {
        Self {
            error,
            operation_id: None,
            effects: serde_json::json!({"catalog":"unchanged", "custody":"not_accessed"}),
        }
    }
}

impl From<MutationFailure> for ClientFailure {
    fn from(failure: MutationFailure) -> Self {
        Self {
            error: failure.error,
            operation_id: failure.operation_id,
            effects: serde_json::to_value(failure.effects).unwrap_or(Value::Null),
        }
    }
}

impl From<BackupFailure> for ClientFailure {
    fn from(failure: BackupFailure) -> Self {
        Self {
            error: failure.error,
            operation_id: failure.operation_id,
            effects: serde_json::json!({"backup":"unknown", "catalog":"unknown", "custody":"unknown", "phase":failure.phase}),
        }
    }
}

#[derive(Serialize)]
struct Envelope {
    schema_version: u32,
    command: &'static str,
    target: Value,
    operation_id: Option<String>,
    state: &'static str,
    effects: Value,
    result: Option<Value>,
    error: Option<Failure>,
}

#[derive(Serialize)]
struct Failure {
    code: String,
    message: String,
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().collect();
    // Preserve machine-readable failures even when argument parsing fails. Never
    // echo parser input: it may contain accidentally supplied secret material.
    let requested_json = args.iter().any(|arg| arg == "--output=json")
        || args.windows(2).any(|pair| pair[0] == "--output" && pair[1] == "json");
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => {
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                return if error.print().is_ok() { ExitCode::SUCCESS } else { ExitCode::from(8) };
            }
            let envelope = failure(
                "usage",
                Value::Null,
                "invalid_arguments",
                "Invalid arguments; run idctl --help.",
            );
            let output = if requested_json { Output::Json } else { Output::Human };
            return emit(&envelope, output, 2);
        }
    };
    let (command, target) = match &cli.command {
        Command::Capabilities => ("capabilities", Value::Null),
        Command::Identity { command: IdentityCommand::List } => ("identity.list", Value::Null),
        Command::Identity { command: IdentityCommand::Show { entry, expect_identity } } => (
            "identity.show",
            serde_json::json!({"selector": entry, "expected_identity_id": expect_identity.map(|id| id.to_string())}),
        ),
        Command::Identity { command: IdentityCommand::Create { .. } } => {
            ("identity.create", Value::Null)
        }
        Command::Identity { command: IdentityCommand::Adopt { expect_identity, .. } } => (
            "identity.adopt",
            serde_json::json!({"expected_identity_id":expect_identity.to_string()}),
        ),
        Command::Identity { command: IdentityCommand::Update { entry, .. } } => {
            ("identity.update", serde_json::json!({"entry_id":entry}))
        }
        Command::Identity { command: IdentityCommand::Select { entry, .. } } => {
            ("identity.select", serde_json::json!({"entry_id":entry}))
        }
        Command::Identity { command: IdentityCommand::Forget { entry, .. } } => {
            ("identity.forget", serde_json::json!({"entry_id":entry}))
        }
        Command::Operation { command: OperationCommand::Show { operation } } => {
            ("operation.show", serde_json::json!({"operation_id":operation}))
        }
        Command::Operation { command: OperationCommand::List {..} } => ("operation.list",Value::Null),
        Command::Operation { command: OperationCommand::Reconcile { operation, .. } } => {
            ("operation.reconcile", serde_json::json!({"operation_id":operation}))
        }
        Command::Backup { command: BackupCommand::Inspect { .. } } => {
            ("backup.inspect", Value::Null)
        }
        Command::Backup { command: BackupCommand::Verify { expect_identity, .. } } => (
            "backup.verify",
            serde_json::json!({"expected_identity_id": expect_identity.map(|id| id.to_string())}),
        ),
        Command::Backup { command: BackupCommand::Export { .. } } => ("backup.export", Value::Null),
        Command::Backup { command: BackupCommand::Reprotect { .. } } => {
            ("backup.reprotect", Value::Null)
        }
        Command::Backup { command: BackupCommand::Restore { .. } } => {
            ("backup.restore", Value::Null)
        }
        Command::Backup { command: BackupCommand::VerifyRecovery { .. } } => {
            ("backup.verify-recovery", Value::Null)
        }
        Command::Backup { command: BackupCommand::MigrateRecovery { .. } } => {
            ("backup.migrate-recovery", Value::Null)
        }
        Command::Backup { command: BackupCommand::List { .. } } => ("backup.list", Value::Null),
        Command::Backup { command: BackupCommand::Show { .. } } => ("backup.show", Value::Null),
        Command::Backup { command: BackupCommand::Forget { .. } } => ("backup.forget", Value::Null),
        Command::Backup { command: BackupCommand::Delete { .. } } => ("backup.delete", Value::Null),
    };
    let needs_store = !matches!(
        cli.command,
        Command::Capabilities
            | Command::Backup {
                command: BackupCommand::Inspect { .. } | BackupCommand::Verify { .. }
            }
    );
    if needs_store && cli.store.is_none() {
        return emit(
            &failure(
                command,
                target,
                "store_required",
                "This command requires an explicit --store directory.",
            ),
            cli.output,
            2,
        );
    }
    let result = execute(&cli);
    match result {
        Ok(result) => emit(
            &Envelope {
                schema_version: 1,
                command,
                target,
                operation_id: result.operation_id,
                state: "completed",
                effects: result.effects,
                result: Some(result.value),
                error: None,
            },
            cli.output,
            0,
        ),
        Err(client_failure) => {
            let error = client_failure.error;
            let code = if client_failure.operation_id.is_some() {
                9
            } else {
                match error {
                    LifecycleError::CatalogUninitialized | LifecycleError::EntryNotFound => 3,
                    LifecycleError::OperationNotFound | LifecycleError::ArtifactNotFound => 3,
                    LifecycleError::CatalogUnavailable
                    | LifecycleError::UnsupportedSchema
                    | LifecycleError::IdentityUnavailable
                    | LifecycleError::CustodyUnavailable
                    | LifecycleError::UnsupportedOperation
                    | LifecycleError::UnsafeStorage
                    | LifecycleError::OperationSuperseded => 4,
                    LifecycleError::AuthenticationRequired
                    | LifecycleError::AuthenticationFailed => 5,
                    LifecycleError::AmbiguousName
                    | LifecycleError::IdentityMismatch
                    | LifecycleError::RevisionConflict
                    | LifecycleError::DestinationConflict
                    | LifecycleError::RequestConflict
                    | LifecycleError::StoreBusy => 6,
                    LifecycleError::LocationChanged => 6,
                    LifecycleError::ReconciliationRequired => 9,
                    LifecycleError::CatalogTooLarge
                    | LifecycleError::InvalidCatalog
                    | LifecycleError::InvalidRequest
                    | LifecycleError::InvalidBackup => 2,
                    _ => 8,
                }
            };
            let error_code = serde_json::to_value(error)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_else(|| "operation_failed".to_owned());
            let mut envelope = failure(command, target, &error_code, &error.to_string());
            envelope.state = if code == 9 { "needs_reconciliation" } else { "failed" };
            envelope.operation_id = client_failure.operation_id;
            envelope.effects = client_failure.effects;
            emit(&envelope, cli.output, code)
        }
    }
}

fn execute(cli: &Cli) -> Result<Success, ClientFailure> {
    match &cli.command {
        Command::Capabilities => read_result(capabilities()),
        Command::Identity { command } => {
            let store = cli.store.as_deref().ok_or(LifecycleError::CatalogUnavailable)?;
            match command {
                IdentityCommand::List => read_result(CatalogSnapshot::load(store)?.list()),
                IdentityCommand::Show { entry, expect_identity } => {
                    read_result(CatalogSnapshot::load(store)?.show(entry, *expect_identity)?)
                }
                IdentityCommand::Create {
                    name, destination, request_id, passphrase_stdin, ..
                } => {
                    let protection = read_protection(*passphrase_stdin)?;
                    mutation_result(mutations::execute(
                        store,
                        request_id,
                        Mutation::Create { name: name.clone(), destination: destination.clone() },
                        &protection,
                    )?)
                }
                IdentityCommand::Adopt {
                    name,
                    custody,
                    expect_identity,
                    request_id,
                    passphrase_stdin,
                } => {
                    let protection = read_protection(*passphrase_stdin)?;
                    mutation_result(mutations::execute(
                        store,
                        request_id,
                        Mutation::Adopt {
                            name: name.clone(),
                            custody: custody.clone(),
                            expected_identity: *expect_identity,
                        },
                        &protection,
                    )?)
                }
                IdentityCommand::Update { entry, name, if_revision, request_id, .. } => {
                    mutation_result(mutations::execute(
                        store,
                        request_id,
                        Mutation::Update {
                            entry_id: entry.clone(),
                            if_revision: *if_revision,
                            name: name.clone().map_or(NameChange::Clear, NameChange::Set),
                        },
                        &[],
                    )?)
                }
                IdentityCommand::Select { entry, if_revision, request_id } => {
                    mutation_result(mutations::execute(
                        store,
                        request_id,
                        Mutation::Select { entry_id: entry.clone(), if_revision: *if_revision },
                        &[],
                    )?)
                }
                IdentityCommand::Forget { entry, if_revision, request_id } => {
                    mutation_result(mutations::execute(
                        store,
                        request_id,
                        Mutation::Forget { entry_id: entry.clone(), if_revision: *if_revision },
                        &[],
                    )?)
                }
            }
        }
        Command::Operation { command } => {
            let store = cli.store.as_deref().ok_or(LifecycleError::CatalogUnavailable)?;
            match command {
                OperationCommand::List {all} => read_result(mutations::list_operations(store,*all)?),
                OperationCommand::Show { operation } => {
                    if operation.starts_with("backup-op-") {
                        read_result(artifacts::show_operation(store, operation)?)
                    } else {
                        read_result(mutations::show_operation(store, operation)?)
                    }
                }
                OperationCommand::Reconcile { operation, passphrase_stdin, protection_stdin } => {
                    if operation.starts_with("backup-op-") {
                        if *passphrase_stdin {
                            return Err(LifecycleError::InvalidRequest.into());
                        }
                        let protection = read_protections(*protection_stdin)?;
                        artifact_result(artifacts::reconcile(
                            store,
                            operation,
                            protection.source(),
                            protection.destination(),
                        )?)
                    } else {
                        if *protection_stdin {
                            return Err(LifecycleError::InvalidRequest.into());
                        }
                        let protection = read_protection(*passphrase_stdin)?;
                        mutation_result(mutations::reconcile(store, operation, &protection)?)
                    }
                }
            }
        }
        Command::Backup { command } => {
            let mut result = match command {
                BackupCommand::Inspect { input } => read_result(backups::inspect(input)?),
                BackupCommand::Verify { input, expect_identity, passphrase_stdin } => {
                    let protection = read_protection(*passphrase_stdin)?;
                    read_result(backups::verify(input, *expect_identity, &protection)?)
                }
                BackupCommand::VerifyRecovery { operation, expect_identity, passphrase_stdin } => {
                    let protection = read_protection(*passphrase_stdin)?;
                    read_result(artifacts::verify_recovery(
                        cli.store.as_deref().ok_or(LifecycleError::CatalogUnavailable)?,
                        operation,
                        *expect_identity,
                        &protection,
                    )?)
                }
                BackupCommand::List { identity } => read_result(artifacts::list(
                    cli.store.as_deref().ok_or(LifecycleError::CatalogUnavailable)?,
                    *identity,
                )?),
                BackupCommand::Show { artifact } => read_result(artifacts::show(
                    cli.store.as_deref().ok_or(LifecycleError::CatalogUnavailable)?,
                    artifact,
                )?),
                command => return execute_backup_mutation(cli, command),
            }?;
            result.effects["backup"] = serde_json::json!("unchanged");
            Ok(result)
        }
    }
}

fn execute_backup_mutation(cli: &Cli, command: &BackupCommand) -> Result<Success, ClientFailure> {
    let store = cli.store.as_deref().ok_or(LifecycleError::CatalogUnavailable)?;
    let (request_id, request, input) = match command {
        BackupCommand::Export {
            entry,
            output_file,
            expect_identity,
            request_id,
            protection_stdin,
        } => (
            request_id,
            BackupMutation::Export {
                entry_id: entry.clone(),
                output: output_file.clone(),
                expected_identity: *expect_identity,
            },
            *protection_stdin,
        ),
        BackupCommand::Reprotect {
            input,
            output_file,
            expect_identity,
            request_id,
            protection_stdin,
        } => (
            request_id,
            BackupMutation::Reprotect {
                input: input.clone(),
                output: output_file.clone(),
                expected_identity: *expect_identity,
            },
            *protection_stdin,
        ),
        BackupCommand::Restore {
            input,
            destination,
            name,
            expect_identity,
            request_id,
            protection_stdin,
        } => (
            request_id,
            BackupMutation::Restore {
                input: input.clone(),
                destination: destination.clone(),
                name: name.clone(),
                expected_identity: *expect_identity,
            },
            *protection_stdin,
        ),
        BackupCommand::MigrateRecovery {
            operation,
            output_file,
            expect_identity,
            request_id,
            protection_stdin,
        } => (
            request_id,
            BackupMutation::MigrateRecovery {
                source_operation: operation.clone(),
                output: output_file.clone(),
                expected_identity: *expect_identity,
            },
            *protection_stdin,
        ),
        BackupCommand::Forget { artifact, expect_digest, request_id } => (
            request_id,
            BackupMutation::Forget {
                artifact_id: artifact.clone(),
                expected_digest: expect_digest.clone(),
            },
            false,
        ),
        BackupCommand::Delete { artifact, expect_digest, request_id } => (
            request_id,
            BackupMutation::Delete {
                artifact_id: artifact.clone(),
                expected_digest: expect_digest.clone(),
            },
            false,
        ),
        _ => return Err(LifecycleError::InvalidRequest.into()),
    };
    let protection = read_protections(input)?;
    artifact_result(artifacts::execute(
        store,
        request_id,
        request,
        protection.source(),
        protection.destination(),
    )?)
}

fn artifact_result(result: BackupResult) -> Result<Success, ClientFailure> {
    let operation_id = Some(result.operation_id.clone());
    let effects = serde_json::json!({"backup":result.effect,"catalog":if result.entry_id.is_some(){"registered"}else{"unchanged"},"custody":if result.effect=="restored"{"restored_or_already_present"}else{"unchanged"}});
    let value = serde_json::to_value(&result).map_err(|_| ClientFailure {
        error: LifecycleError::OperationFailed,
        operation_id: operation_id.clone(),
        effects: effects.clone(),
    })?;
    Ok(Success { value, operation_id, effects })
}

struct ProtectionInputs {
    bytes: Zeroizing<Vec<u8>>,
    source_end: usize,
    destination_start: usize,
}
impl ProtectionInputs {
    fn source(&self) -> &[u8] {
        &self.bytes[..self.source_end]
    }
    fn destination(&self) -> &[u8] {
        &self.bytes[self.destination_start..]
    }
}

fn read_protections(enabled: bool) -> Result<ProtectionInputs, LifecycleError> {
    if !enabled {
        return Ok(ProtectionInputs {
            bytes: Zeroizing::new(vec![]),
            source_end: 0,
            destination_start: 0,
        });
    }
    let stdin = io::stdin();
    if stdin.is_terminal() {
        return Err(LifecycleError::AuthenticationRequired);
    }
    let mut bytes = Zeroizing::new(Vec::with_capacity(8197));
    stdin
        .lock()
        .take(8197)
        .read_to_end(&mut bytes)
        .map_err(|_| LifecycleError::AuthenticationRequired)?;
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
        if bytes.last() == Some(&b'\r') {
            bytes.pop();
        }
    }
    let split =
        bytes.iter().position(|byte| *byte == b'\n').ok_or(LifecycleError::InvalidRequest)?;
    let source_end = if split > 0 && bytes[split - 1] == b'\r' { split - 1 } else { split };
    let destination_start = split + 1;
    if source_end > 4096
        || bytes.len() - destination_start > 4096
        || bytes[..source_end].contains(&b'\r')
        || bytes[destination_start..].iter().any(|byte| matches!(byte, b'\r' | b'\n'))
    {
        return Err(LifecycleError::InvalidRequest);
    }
    Ok(ProtectionInputs { bytes, source_end, destination_start })
}

fn read_result(value: impl Serialize) -> Result<Success, ClientFailure> {
    Ok(Success {
        value: serde_json::to_value(value).map_err(|_| LifecycleError::OperationFailed)?,
        operation_id: None,
        effects: serde_json::json!({"catalog":"unchanged", "custody":"not_accessed"}),
    })
}

fn mutation_result(result: MutationResult) -> Result<Success, ClientFailure> {
    // Typed results contain only serializable public fields. Preserve the committed
    // outcome if serialization ever fails, rather than reporting no effect.
    let fail = || ClientFailure {
        error: LifecycleError::OperationFailed,
        operation_id: Some(result.operation_id.clone()),
        effects: serde_json::json!({"catalog":"committed", "custody":"unknown"}),
    };
    Ok(Success {
        value: serde_json::to_value(&result).map_err(|_| fail())?,
        operation_id: Some(result.operation_id.clone()),
        effects: serde_json::to_value(&result.effects).map_err(|_| fail())?,
    })
}

fn read_protection(enabled: bool) -> Result<Zeroizing<Vec<u8>>, LifecycleError> {
    if !enabled {
        return Ok(Zeroizing::new(Vec::new()));
    }
    let stdin = io::stdin();
    if stdin.is_terminal() {
        return Err(LifecycleError::AuthenticationRequired);
    }
    let mut bytes = Zeroizing::new(Vec::with_capacity(4098));
    stdin
        .lock()
        .take(4098)
        .read_to_end(&mut bytes)
        .map_err(|_| LifecycleError::AuthenticationRequired)?;
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
        if bytes.last() == Some(&b'\r') {
            bytes.pop();
        }
    }
    if bytes.is_empty() {
        return Err(LifecycleError::AuthenticationRequired);
    }
    if bytes.len() > 4096 || bytes.contains(&b'\n') || bytes.contains(&b'\r') {
        return Err(LifecycleError::InvalidRequest);
    }
    Ok(bytes)
}

fn failure(command: &'static str, target: Value, code: &str, message: &str) -> Envelope {
    Envelope {
        schema_version: 1,
        command,
        target,
        operation_id: None,
        state: "failed",
        effects: serde_json::json!({"catalog": "unchanged", "custody": "not_accessed"}),
        result: None,
        error: Some(Failure { code: code.to_owned(), message: message.to_owned() }),
    }
}

fn emit(envelope: &Envelope, output: Output, code: u8) -> ExitCode {
    let written = match output {
        Output::Json => {
            let stdout = io::stdout();
            let mut writer = stdout.lock();
            serde_json::to_writer(&mut writer, envelope)
                .map_err(io::Error::other)
                .and_then(|()| writeln!(writer))
                .and_then(|()| writer.flush())
        }
        Output::Human => match &envelope.error {
            Some(error) => {
                let mut writer = io::stderr().lock();
                writeln!(writer, "{}: {}", error.code, error.message).and_then(|()| {
                    if let Some(operation) = &envelope.operation_id {
                        writeln!(
                            writer,
                            "Operation: {operation}; state: {}; inspect before retrying",
                            envelope.state
                        )
                    } else {
                        Ok(())
                    }
                })
            }
            None => match &envelope.result {
                Some(result) => render_human(&mut io::stdout().lock(), envelope.command, result),
                None => Ok(()),
            },
        },
    };
    if written.is_err() {
        ExitCode::from(if envelope.operation_id.is_some() { 9 } else { 8 })
    } else {
        ExitCode::from(code)
    }
}

fn render_human(writer: &mut impl Write, command: &str, result: &Value) -> io::Result<()> {
    match command {
        "capabilities" => {
            writeln!(writer, "Available operations:")?;
            if let Some(operations) = result["supported"].as_array() {
                for operation in operations {
                    writeln!(writer, "  {}", operation.as_str().unwrap_or("unknown"))?;
                }
            }
            writeln!(writer, "File mutations supported: {}", result["mutations_supported"])
        }
        "identity.list" => {
            writeln!(writer, "Catalog revision: {}", result["revision"])?;
            writeln!(
                writer,
                "Preferred entry: {}",
                result["preferred_entry"].as_str().unwrap_or("none")
            )?;
            if let Some(entries) = result["entries"].as_array() {
                if entries.is_empty() {
                    writeln!(writer, "No identities registered.")?;
                }
                for entry in entries {
                    render_entry(writer, entry)?;
                }
            }
            Ok(())
        }
        "identity.show" => render_entry(writer, result),
        _ => writeln!(writer, "{result:#}"),
    }
}

fn render_entry(writer: &mut impl Write, entry: &Value) -> io::Result<()> {
    writeln!(writer, "Entry: {}", entry["entry_id"].as_str().unwrap_or("unknown"))?;
    writeln!(writer, "  Name: {}", entry["display_name"].as_str().unwrap_or("not set"))?;
    writeln!(writer, "  Revision: {}", entry["revision"])?;
    writeln!(
        writer,
        "  Styrene Identity ID: {}",
        entry["identity"]["identity_id"].as_str().unwrap_or("unavailable")
    )?;
    writeln!(
        writer,
        "  Public binding: {}",
        entry["identity"]["status"].as_str().unwrap_or("unknown")
    )?;
    writeln!(writer, "  Provider availability: unknown (custody not accessed)")?;
    writeln!(writer, "  Root exposure: unknown")
}
