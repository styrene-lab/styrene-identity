//! Deterministic key hierarchy for Styrene mesh nodes.
//!
//! One 32-byte root secret derives all protocol-specific keys — RNS,
//! Yggdrasil, WireGuard, SSH, age, Git commit signing, repository authority,
//! and per-agent delegation via HKDF-SHA256 with domain separation.
//!
//! # Usage
//!
//! ```rust
//! use styrene_identity::derive::{KeyDeriver, KeyPurpose};
//!
//! let root_secret = [0x42u8; 32]; // in practice, from a signer
//! let deriver = KeyDeriver::new(&root_secret);
//!
//! // Flat-purpose keys (7 protocols)
//! let signing_seed = deriver.signing_seed();
//! let repository_seed = deriver.derive_repository_signing_key(0);
//! let age_key  = deriver.derive(KeyPurpose::Age);
//!
//! // Parameterized keys (two-level HKDF, structurally collision-free)
//! let github_ssh = deriver.derive_ssh_user_key("github").unwrap();
//! let agent_key  = deriver.derive_agent_key("omegon-primary").unwrap();
//! ```
//!
//! # Signer tiers
//!
//! The [`IdentitySigner`] trait exposes roots through custody adapters.
//! Multiple adapters represent one identity only when provisioned with the same
//! root. The trait and chain do not verify this equivalence. Current adapters
//! release the root into process memory; tier names do not attest hardware custody.
//!
//! | Tier | Backend | Feature |
//! |------|---------|---------|
//! | A | YubiKey FIDO2 hmac-secret | `yubikey` |
//! | B | Apple Keychain / Android Keystore | `keychain`, `android-keystore` |
//! | C | Credential manager | — (planned) |
//! | D | Encrypted file (argon2id + ChaCha20Poly1305) | `file-signer` (default) |
//!
//! [`SignerChain::new_sorted`](signer::SignerChain::new_sorted) sorts by tier;
//! `new` preserves order. Both select the first available signer and return its
//! result without retrying operation errors. Availability is only a hint.
//!
//! # Feature flags
//!
//! | Feature | Default | Enables |
//! |---------|---------|---------|
//! | `file-signer` | **yes** | `FileSigner`, `IdentityVault` |
//! | `signing` | via file-signer | `pubkey` module (ed25519, x25519) |
//! | `repository-signing` | no | Repository authority bindings and strict verification |
//! | `pki` | no | identity-bound X.509 CA/client/server certificates |
//! | `yubikey` | no | `YubiKeySigner` (FIDO2 hmac-secret) |
//! | `keychain` | no | Apple Keychain storage (Apple targets) |
//! | `android-keystore` | no | AES-wrapped root storage (Android targets) |
//! | `age-format` | no | age Bech32 encoding |
//! | `ssh-agent` | no | `StyreneAgent` (SSH agent protocol) |
//!
//! # Derivation hierarchy
//!
//! ```text
//! root_secret (32 bytes)
//!   HKDF-Extract(salt="styrene-identity-v1", IKM=root_secret) = PRK
//!   │
//!   ├─ Expand("styrene-rns-encryption-v1")  → RNS X25519
//!   ├─ Expand("styrene-rns-signing-v1")     → RNS Ed25519 (canonical identity)
//!   ├─ Expand("styrene-yggdrasil-v1")       → Yggdrasil Ed25519
//!   ├─ Expand("styrene-wireguard-v1")       → WireGuard Curve25519
//!   ├─ Expand("styrene-ssh-host-v1")        → SSH host Ed25519
//!   ├─ Expand("styrene-age-v1")             → age X25519
//!   ├─ Expand("styrene-rns-signing-v1")     → identity and legacy Git commit signing Ed25519
//!   │
//!   ├─ SSH user keys (two-level, salt="styrene-identity-ssh-user-v1")
//!   │   └─ Expand(label) → per-host SSH Ed25519
//!   │
//!   ├─ Agent keys (two-level, salt="styrene-identity-agent-v1")
//!   │   └─ Expand(name) → per-agent signing Ed25519
//!   │
//!   ├─ Repository signing keys (two-level, epoch-indexed)
//!   │   └─ Expand("styrene-repository-signing-epoch-v1\0" || u32be(epoch))
//!   │
//!   └─ TLS certificate keys (two-level, salt="styrene-identity-tls-cert-v1")
//!       └─ Expand(label) → per-certificate Ed25519 X.509 key
//! ```
//!
//! Git commit signatures authenticate individual Git objects. They do not
//! establish Styrene repository authority. Repository authority uses the
//! epoch-indexed key family and an Identity-issued `RepositorySignerBinding`.
//!
//! # Linkability warning
//!
//! **All keys derived from one root are cryptographically linked.** This is
//! by design for attribution and recovery, but it means derived keys cannot
//! provide anonymity or unlinkability. If you need an identity that cannot be
//! traced to your primary identity, use [`ephemeral()`](signer::RootSecret::ephemeral) or a
//! separate identity file. See `docs/unlinkability.md` for the full model.
//!
//! ```rust
//! use styrene_identity::signer::RootSecret;
//!
//! // Anonymous: independent CSPRNG root, no link to any persistent identity
//! let anon = RootSecret::ephemeral();
//! ```
//!
//! # Security
//!
//! - [`RootSecret`], [`KeyDeriver`], and [`DerivedKeys`] zeroize their owned secrets on drop
//! - Passphrase/PIN providers are traits; credential origin is caller-controlled
//! - New identity file creation uses exclusive creation to prevent overwrites
//! - File encryption uses Argon2id (m=64MiB, t=3, p=1)
//!
//! [`IdentitySigner`]: signer::IdentitySigner
//! [`SignerChain`]: signer::SignerChain
//! [`RootSecret`]: signer::RootSecret
//! [`KeyDeriver`]: derive::KeyDeriver
//! [`DerivedKeys`]: derive::DerivedKeys

#[cfg(all(feature = "android-keystore", target_os = "android"))]
pub mod android_keystore_signer;
pub mod derive;
pub mod discover;
#[cfg(feature = "signing")]
pub mod export;
#[cfg(feature = "file-signer")]
pub mod file_signer;
#[cfg(feature = "signing")]
pub mod format;
#[cfg(feature = "signing")]
pub mod identity;
mod identity_id;
#[cfg(all(feature = "keychain", any(target_os = "macos", target_os = "ios")))]
pub mod keychain_signer;
pub mod overview;
#[cfg(feature = "pki")]
pub mod pki;
#[cfg(feature = "signing")]
pub mod pubkey;
pub mod records;
#[cfg(feature = "repository-signing")]
pub mod repository_signing;
pub mod signer;
#[cfg(feature = "ssh-agent")]
pub mod ssh_agent;
#[cfg(feature = "file-signer")]
pub mod vault;
#[cfg(feature = "yubikey")]
pub mod yubikey_signer;

pub use derive::{
    DeriveError, DerivedKeys, KeyDeriver, KeyPurpose, derive_key, derive_keys, validate_label,
};
pub use discover::{DiscoveredIdentity, discover};
#[cfg(feature = "signing")]
pub use export::AllPublicKeys;
#[cfg(feature = "signing")]
#[allow(deprecated)]
pub use identity::{
    IDENTITY_HASH_BYTES, IdentityInfo, PublicIdentity, SignedAttestation, identity_hash,
    identity_pubkey, identity_sign, identity_verify,
};
pub use identity_id::{IdentityId, IdentityIdError};
#[cfg(feature = "pki")]
pub use pki::{
    CertificateRole, StyreneCertificate, StyreneCertificateChain, StyreneCertificateProfile,
    StyrenePkiError, derive_ca_certificate, derive_ca_certificate_with_profile,
    derive_client_certificate_chain, derive_client_certificate_chain_with_profile,
    derive_server_certificate_chain, derive_server_certificate_chain_with_profile,
    styrene_agent_uri, styrene_ca_uri, styrene_client_uri,
};
#[cfg(feature = "repository-signing")]
pub use repository_signing::{
    RepositorySignerBinding, RepositorySignerBindingError, RepositorySignerBindingErrorClass,
    VerifiedRepositorySignerBinding, verify_repository_signer_binding,
};
pub use signer::{IdentitySigner, SignerChain, SignerError, SignerTier};
