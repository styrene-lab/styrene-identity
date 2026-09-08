//! Concrete one-page host lifecycle. The host persists only `enabled` and owns
//! session selection and independent observation of accepted backend operations.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExtensionDescriptor {
    pub id: &'static str,
    pub build_version: &'static str,
    pub host_contract_revision: u32,
    pub page_id: &'static str,
    pub title: &'static str,
}

pub const IDENTITY_EXTENSION: ExtensionDescriptor = ExtensionDescriptor {
    id: "styrene.identity",
    build_version: env!("CARGO_PKG_VERSION"),
    host_contract_revision: 1,
    page_id: "identity",
    title: "Identity",
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtensionError {
    NotBundled,
    Incompatible,
    PendingOperation,
    GenerationExhausted,
}

#[derive(Clone, Debug)]
pub struct IdentityExtension {
    bundled: bool,
    compatible: bool,
    enabled: bool,
    mounted: bool,
    generation: u64,
}

impl IdentityExtension {
    pub fn new(
        bundled: bool,
        descriptor: ExtensionDescriptor,
        host_revision: u32,
        saved_enabled: Option<bool>,
    ) -> Self {
        Self {
            bundled,
            compatible: descriptor.id == IDENTITY_EXTENSION.id
                && descriptor.page_id == IDENTITY_EXTENSION.page_id
                && descriptor.host_contract_revision == IDENTITY_EXTENSION.host_contract_revision
                && descriptor.host_contract_revision == host_revision,
            enabled: saved_enabled.unwrap_or(true),
            mounted: false,
            generation: 0,
        }
    }
    pub fn registered(&self) -> bool {
        self.bundled && self.compatible && self.enabled
    }
    pub fn enabled_preference(&self) -> bool {
        self.enabled
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn mounted(&self) -> bool {
        self.mounted
    }
    pub fn enable(&mut self) -> Result<(), ExtensionError> {
        if !self.bundled {
            return Err(ExtensionError::NotBundled);
        }
        if !self.compatible {
            return Err(ExtensionError::Incompatible);
        }
        if !self.enabled {
            self.advance()?;
            self.enabled = true;
        }
        Ok(())
    }
    pub fn mount(&mut self) -> Result<u64, ExtensionError> {
        if !self.registered() {
            return Err(if !self.bundled {
                ExtensionError::NotBundled
            } else {
                ExtensionError::Incompatible
            });
        }
        if !self.mounted {
            self.advance()?;
            self.mounted = true;
        }
        Ok(self.generation)
    }
    pub fn unmount(&mut self) -> Result<(), ExtensionError> {
        if self.mounted {
            self.advance()?;
            self.mounted = false;
        }
        Ok(())
    }
    pub fn session_changed(&mut self) -> Result<(), ExtensionError> {
        self.advance()
    }
    pub fn disable(
        &mut self,
        pending_operation: bool,
        independent_observation: bool,
    ) -> Result<(), ExtensionError> {
        if pending_operation && !independent_observation {
            return Err(ExtensionError::PendingOperation);
        }
        if self.enabled {
            self.advance()?;
            self.enabled = false;
            self.mounted = false;
        }
        Ok(())
    }
    pub fn accepts_view_result(&self, generation: u64) -> bool {
        self.registered() && self.mounted && self.generation == generation
    }
    fn advance(&mut self) -> Result<(), ExtensionError> {
        match self.generation.checked_add(1) {
            Some(value) => {
                self.generation = value;
                Ok(())
            }
            None => {
                self.compatible = false;
                self.mounted = false;
                Err(ExtensionError::GenerationExhausted)
            }
        }
    }
}
