use crate::service::{Action, Request};
use dioxus::prelude::*;
use rand_core::RngCore;
use std::path::PathBuf;
use styrene_identity::IdentityId;
use styrene_identity_lifecycle::mutations::{Mutation, NameChange, artifacts::BackupMutation};
use zeroize::Zeroizing;

#[derive(Default)]
struct SecretDraft(Zeroizing<String>);
impl SecretDraft {
    fn set(&mut self, text: String) {
        self.0 = Zeroizing::new(text)
    }
    fn take(&mut self) -> Zeroizing<Vec<u8>> {
        Zeroizing::new(std::mem::take(&mut *self.0).into_bytes())
    }
}

fn request_id() -> String {
    let mut bytes = [0u8; 16];
    rand_core::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn absolute(text: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(text.trim());
    if path.is_absolute() {
        Ok(path)
    } else {
        Err("Input and output paths must be absolute.".into())
    }
}
fn expected(text: &str) -> Result<IdentityId, String> {
    text.trim()
        .parse()
        .map_err(|_| "Expected identity must be 32 lowercase hexadecimal characters.".into())
}

#[component]
pub fn ActionForm(
    store: Option<PathBuf>,
    selected: String,
    busy: bool,
    initial: String,
    onsubmit: EventHandler<Request>,
) -> Element {
    let mut kind = use_signal(move || initial.clone());
    let mut entry = use_signal(move || selected.clone());
    let mut name = use_signal(String::new);
    let mut input = use_signal(String::new);
    let mut output = use_signal(String::new);
    let mut identity = use_signal(String::new);
    let mut revision = use_signal(|| "1".to_owned());
    let mut artifact = use_signal(String::new);
    let mut digest = use_signal(String::new);
    let mut operation = use_signal(String::new);
    let mut request = use_signal(request_id);
    let mut source = use_signal(SecretDraft::default);
    let mut target = use_signal(SecretDraft::default);
    let mut secret_epoch = use_signal(|| 0u64);
    let mut error = use_signal(String::new);
    let choice = kind.read().clone();
    let needs_entry =
        matches!(choice.as_str(), "update" | "clear" | "select" | "forget" | "export");
    let needs_name = matches!(choice.as_str(), "create" | "adopt" | "update" | "restore");
    let needs_input =
        matches!(choice.as_str(), "adopt" | "inspect" | "verify" | "reprotect" | "restore");
    let needs_output =
        matches!(choice.as_str(), "create" | "export" | "reprotect" | "restore" | "migrate");
    let needs_identity = matches!(
        choice.as_str(),
        "adopt" | "verify" | "export" | "reprotect" | "restore" | "migrate"
    );
    let needs_revision = matches!(choice.as_str(), "update" | "clear" | "select" | "forget");
    let needs_artifact = matches!(choice.as_str(), "forget-backup" | "delete-backup");
    let needs_operation = matches!(choice.as_str(), "reconcile" | "migrate");
    let needs_source = matches!(
        choice.as_str(),
        "adopt" | "verify" | "export" | "reprotect" | "restore" | "migrate" | "reconcile"
    );
    let needs_target = matches!(
        choice.as_str(),
        "create" | "export" | "reprotect" | "restore" | "migrate" | "reconcile"
    );
    let mutation = !matches!(choice.as_str(), "inspect" | "verify" | "reconcile");
    rsx! {
        form {class:"action-form",onsubmit:move|event|{
            event.prevent_default();if busy{return}
            let source_bytes=source.write().take();let target_bytes=target.write().take();
            let next=secret_epoch.peek().checked_add(1);if let Some(next)=next{secret_epoch.set(next);}
            let result=(||->Result<Request,String>{
                let kind=kind.read().clone();let id=request.read().clone();let selected=entry.read().clone();
                let rev=||revision.read().parse::<u64>().map_err(|_|"Revision must be an unsigned integer.".to_owned());
                if !matches!(kind.as_str(),"inspect"|"verify")&&store.is_none(){return Err("Choose an identity store first.".into())}
                let action=match kind.as_str(){
                    "create"=>Action::Identity{request_id:id,mutation:Mutation::Create{name:name.read().clone(),destination:absolute(&output.read())?},protection:target_bytes},
                    "adopt"=>Action::Identity{request_id:id,mutation:Mutation::Adopt{name:name.read().clone(),custody:absolute(&input.read())?,expected_identity:expected(&identity.read())?},protection:source_bytes},
                    "update"|"clear"=>Action::Identity{request_id:id,mutation:Mutation::Update{entry_id:selected,if_revision:rev()?,name:if kind=="clear"{NameChange::Clear}else{NameChange::Set(name.read().clone())}},protection:Zeroizing::new(vec![])},
                    "select"=>Action::Identity{request_id:id,mutation:Mutation::Select{entry_id:selected,if_revision:rev()?},protection:Zeroizing::new(vec![])},
                    "forget"=>Action::Identity{request_id:id,mutation:Mutation::Forget{entry_id:selected,if_revision:rev()?},protection:Zeroizing::new(vec![])},
                    "inspect"=>Action::Inspect{path:absolute(&input.read())?},
                    "verify"=>Action::Verify{path:absolute(&input.read())?,expected:if identity.read().is_empty(){None}else{Some(expected(&identity.read())?)},protection:source_bytes},
                    "reconcile"=>Action::Reconcile{operation:operation.read().clone(),source:source_bytes,destination:target_bytes},
                    other=>{
                        let action=match other{
                            "export"=>BackupMutation::Export{entry_id:selected,output:absolute(&output.read())?,expected_identity:expected(&identity.read())?},
                            "reprotect"=>BackupMutation::Reprotect{input:absolute(&input.read())?,output:absolute(&output.read())?,expected_identity:expected(&identity.read())?},
                            "restore"=>BackupMutation::Restore{input:absolute(&input.read())?,destination:absolute(&output.read())?,name:name.read().clone(),expected_identity:expected(&identity.read())?},
                            "migrate"=>BackupMutation::MigrateRecovery{source_operation:operation.read().clone(),output:absolute(&output.read())?,expected_identity:expected(&identity.read())?},
                            "forget-backup"=>BackupMutation::Forget{artifact_id:artifact.read().clone(),expected_digest:digest.read().clone()},
                            "delete-backup"=>BackupMutation::Delete{artifact_id:artifact.read().clone(),expected_digest:digest.read().clone()},
                            _=>return Err("Unsupported operation.".into()),
                        };
                        Action::Backup{request_id:id,mutation:action,source:source_bytes,destination:target_bytes}
                    }
                };
                Ok(Request{store:store.clone(),action})
            })();
            match result{Ok(request)=>{error.set(String::new());onsubmit.call(request)},Err(message)=>error.set(message)}
        },
            h2{"Lifecycle operation"}
            p{class:"muted","Changes run through the shared backend. Forgetting removes a reference; deleting a managed backup does not revoke identity or erase other copies."}
            label{r#for:"action-kind","Operation"}
            select{id:"action-kind",value:"{kind}",disabled:busy,onchange:move|event|{kind.set(event.value());request.set(request_id());source.write().set(String::new());target.write().set(String::new());let next=secret_epoch.peek().checked_add(1);if let Some(next)=next{secret_epoch.set(next);}},
                for (value,label) in [("create","Create file identity"),("adopt","Adopt existing file custody"),("update","Rename local identity"),("clear","Clear local display name"),("select","Select local preferred identity"),("forget","Forget catalog reference"),("inspect","Inspect encrypted backup"),("verify","Verify encrypted backup"),("export","Export encrypted backup"),("reprotect","Reprotect backup to a new file"),("restore","Restore backup and register identity"),("forget-backup","Forget managed backup reference"),("delete-backup","Delete managed backup artifact"),("migrate","Migrate retained recovery material"),("reconcile","Reconcile recorded operation")] {option{value,"{label}"}}
            }
            if needs_entry {label{r#for:"entry-id","Entry ID"}input{id:"entry-id",value:"{entry}",disabled:busy,oninput:move|event|entry.set(event.value())}}
            if needs_name {label{r#for:"display-name","Local display name"}input{id:"display-name",value:"{name}",disabled:busy,oninput:move|event|name.set(event.value())}}
            if needs_input {label{r#for:"input-path","Source file (absolute path)"}input{id:"input-path",value:"{input}",disabled:busy,oninput:move|event|input.set(event.value())}}
            if needs_output {label{r#for:"output-path","Destination file (absolute path)"}input{id:"output-path",value:"{output}",disabled:busy,oninput:move|event|output.set(event.value())}}
            if needs_identity {label{r#for:"expected-identity","Expected canonical Identity ID"}input{id:"expected-identity",value:"{identity}",disabled:busy,oninput:move|event|identity.set(event.value())}}
            if needs_revision {label{r#for:"revision","Expected revision (catalog for Select; entry for other edits)"}input{id:"revision",r#type:"number",min:"0",value:"{revision}",disabled:busy,oninput:move|event|revision.set(event.value())}}
            if needs_artifact {label{r#for:"artifact-id","Managed artifact ID"}input{id:"artifact-id",value:"{artifact}",disabled:busy,oninput:move|event|artifact.set(event.value())}label{r#for:"artifact-digest","Expected artifact SHA-256"}input{id:"artifact-digest",value:"{digest}",disabled:busy,oninput:move|event|digest.set(event.value())}}
            if needs_operation {label{r#for:"operation-id","Operation ID"}input{id:"operation-id",value:"{operation}",disabled:busy,oninput:move|event|operation.set(event.value())}}
            div{key:"secret-{secret_epoch}",
                if needs_source {label{r#for:"source-protection","Source / current protection"}input{id:"source-protection",r#type:"password",autocomplete:"current-password",disabled:busy,oninput:move|event|source.write().set(event.value())}}
                if needs_target {label{r#for:"destination-protection","Destination / new protection"}input{id:"destination-protection",r#type:"password",autocomplete:"new-password",disabled:busy,oninput:move|event|target.write().set(event.value())}}
            }
            if mutation {label{r#for:"request-id","Request ID (reuse only for the same request)"}input{id:"request-id",value:"{request}",disabled:busy,oninput:move|event|request.set(event.value())}}
            if !error.read().is_empty(){p{class:"form-error",role:"status","{error}"}}
            button{r#type:"submit",disabled:busy,if busy{"Operation in progress"}else{"Run selected operation"}}
            p{class:"muted","Protection input is cleared on submission or page change. Browser/platform memory copies are not guaranteed to be erased."}
        }
    }
}
