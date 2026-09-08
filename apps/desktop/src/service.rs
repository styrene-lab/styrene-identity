use serde::Serialize;
use std::{path::PathBuf, sync::Arc};
use styrene_identity::IdentityId;
use styrene_identity_lifecycle::{
    CatalogSnapshot, IdentityInventory, LifecycleError, backups,
    mutations::{
        self,
        artifacts::{self, BackupMutation},
    },
};
use zeroize::Zeroizing;

// No Debug/Serialize: requests carry operation-scoped credential buffers.
pub enum Action {
    Identity {
        request_id: String,
        mutation: mutations::Mutation,
        protection: Zeroizing<Vec<u8>>,
    },
    Backup {
        request_id: String,
        mutation: BackupMutation,
        source: Zeroizing<Vec<u8>>,
        destination: Zeroizing<Vec<u8>>,
    },
    Inspect {
        path: PathBuf,
    },
    Verify {
        path: PathBuf,
        expected: Option<IdentityId>,
        protection: Zeroizing<Vec<u8>>,
    },
    Reconcile {
        operation: String,
        source: Zeroizing<Vec<u8>>,
        destination: Zeroizing<Vec<u8>>,
    },
}

pub struct Request {
    pub store: Option<PathBuf>,
    pub action: Action,
}
pub enum Work {
    Refresh(PathBuf),
    Execute(Request),
}

#[derive(Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Outcome {
    Identity(mutations::MutationResult),
    Backup(artifacts::BackupResult),
    Inspection(backups::BackupInspection),
    Verification(backups::VerifiedBackup),
}

impl Outcome {
    pub fn operation_id(&self) -> Option<&str> {
        match self {
            Self::Identity(result) => Some(&result.operation_id),
            Self::Backup(result) => Some(&result.operation_id),
            _ => None,
        }
    }
}

pub struct Failure {
    pub error: LifecycleError,
    pub operation_id: Option<String>,
}
impl From<LifecycleError> for Failure {
    fn from(error: LifecycleError) -> Self {
        Self { error, operation_id: None }
    }
}

pub struct Snapshot {
    pub catalog: Arc<CatalogSnapshot>,
    pub inventory: IdentityInventory,
    pub operations: Vec<mutations::ObservedOperation>,
    pub backups: Vec<artifacts::ManagedBackup>,
}

pub enum Event {
    Snapshot { store: PathBuf, result: Result<Snapshot, LifecycleError> },
    Completed(Result<Outcome, Failure>),
}

#[derive(Clone)]
pub struct Service {
    pub requests: async_channel::Sender<Work>,
    pub events: async_channel::Receiver<Event>,
}

impl Service {
    pub fn start() -> Self {
        let (requests, receiver) = async_channel::bounded(1);
        let (sender, events) = async_channel::bounded(8);
        std::thread::spawn(move || {
            while let Ok(work) = receiver.recv_blocking() {
                match work {
                    Work::Refresh(store) => {
                        let result = snapshot(&store);
                        if sender.send_blocking(Event::Snapshot { store, result }).is_err() {
                            break;
                        }
                    }
                    Work::Execute(request) => {
                        let store = request.store.clone();
                        let result = perform(request);
                        if sender.send_blocking(Event::Completed(result)).is_err() {
                            break;
                        }
                        if let Some(store) = store {
                            let result = snapshot(&store);
                            if sender.send_blocking(Event::Snapshot { store, result }).is_err() {
                                break;
                            }
                        }
                    }
                }
            }
        });
        Self { requests, events }
    }
}

fn snapshot(store: &std::path::Path) -> Result<Snapshot, LifecycleError> {
    let catalog = Arc::new(CatalogSnapshot::load(store)?);
    Ok(Snapshot {
        inventory: catalog.list(),
        catalog,
        operations: mutations::list_operations(store, false)?,
        backups: artifacts::list(store, None)?,
    })
}

pub fn perform(request: Request) -> Result<Outcome, Failure> {
    let store = request.store.as_deref();
    match request.action {
        Action::Identity { request_id, mutation, protection } => mutations::execute(
            store.ok_or(LifecycleError::InvalidRequest)?,
            &request_id,
            mutation,
            &protection,
        )
        .map(Outcome::Identity)
        .map_err(|failure| Failure { error: failure.error, operation_id: failure.operation_id }),
        Action::Backup { request_id, mutation, source, destination } => artifacts::execute(
            store.ok_or(LifecycleError::InvalidRequest)?,
            &request_id,
            mutation,
            &source,
            &destination,
        )
        .map(Outcome::Backup)
        .map_err(|failure| Failure { error: failure.error, operation_id: failure.operation_id }),
        Action::Inspect { path } => Ok(Outcome::Inspection(backups::inspect(&path)?)),
        Action::Verify { path, expected, protection } => {
            Ok(Outcome::Verification(backups::verify(&path, expected, &protection)?))
        }
        Action::Reconcile { operation, source, destination } => {
            let store = store.ok_or(LifecycleError::InvalidRequest)?;
            if operation.starts_with("backup-op-") {
                artifacts::reconcile(store, &operation, &source, &destination)
                    .map(Outcome::Backup)
                    .map_err(|failure| Failure {
                        error: failure.error,
                        operation_id: failure.operation_id,
                    })
            } else {
                mutations::reconcile(store, &operation, &source).map(Outcome::Identity).map_err(
                    |failure| Failure { error: failure.error, operation_id: failure.operation_id },
                )
            }
        }
    }
}
