# styrene-identity

Deterministic key hierarchy for Styrene mesh nodes. One root secret derives
all protocol-specific keys, including Git commit signing and repository
authority keys, via HKDF-SHA256 with domain separation.

Consume an immutable Git revision for this development repository. Registry version
`0.3.2` predates the standalone contracts. The pin below is the software-validated
development checkpoint containing verifier and recovery hardening; consumer
acceptance, platform evidence, and registry release approval remain separate.
See the [checkpoint handoff](docs/handoffs/2026-09-07-checkpoint.md) and
[adversarial review](docs/adversarial-review-2026-09-06.md).

Agents start with [AGENTS.md](AGENTS.md). See [CONTRIBUTING.md](CONTRIBUTING.md)
for standalone validation, [integration context](docs/integration-context.md)
for repository ownership, and [the plugin boundary](docs/plugin-boundary.md)
for the accepted direction and remaining design work.

The Identity product includes a shared lifecycle backend, catalog and managed-backup
CRUD, a CLI, an importable Dioxus overview page, and a standalone desktop shell.
Actual Mesh host adoption and advanced custody/key-management operations remain
separate work. See [CLI usage](apps/cli/README.md),
[desktop usage](apps/desktop/README.md), [product architecture](docs/product-architecture.md), and
[versioning and release workflow](RELEASE.md). See
[file-backed CRUD](docs/file-custody-crud.md) for supported mutations. Advanced
custody operations and platform/device acceptance remain pending.

The proposed [CLI CRUD lifecycle](docs/cli-lifecycle.md) maps identity, custody,
key, backup, and lifecycle operations to the shared backend and acceptance cases.
The [adversarial review](docs/adversarial-review-2026-09-06.md) records a reproduced
weak-key signature-verification flaw and its local fix, recovery hardening,
journal migration limits, and the backup inspection/verification slice.

## Quick start

```toml
[dependencies]
styrene-identity = { git = "https://github.com/styrene-lab/styrene-identity", rev = "4ea2d0c7a6e5c89e09c6ba05dead917dc5b33e9a" }
```

### Generate an identity

```rust,no_run
use styrene_identity::file_signer::{ClosurePassphraseProvider, FileSigner};
use styrene_identity::signer::IdentitySigner;

let provider = Box::new(ClosurePassphraseProvider::new(|| {
    Ok(b"my-passphrase".to_vec())
}));
// Supply an explicit private location; Rust does not expand "~" in this string.
let signer = FileSigner::new("/absolute/private/path/identity.key", provider);
signer.generate(b"my-passphrase").expect("generate identity");
```

### Derive keys from a root secret

```rust
use styrene_identity::derive::{KeyDeriver, KeyPurpose};

let root_secret = [0x42u8; 32]; // in practice, from a signer
let deriver = KeyDeriver::new(&root_secret);

// Flat-purpose keys
let signing_seed = deriver.signing_seed();               // Identity/legacy commit signing
let repository_seed = deriver.derive_repository_signing_key(0); // Repository epoch 0
let age_key  = deriver.derive(KeyPurpose::Age);           // X25519 private key
let ssh_seed = deriver.derive(KeyPurpose::SshHost);       // Ed25519 seed

// Parameterized keys (two-level HKDF — structurally collision-free)
let github_ssh = deriver.derive_ssh_user_key("github").unwrap();
let agent_key  = deriver.derive_agent_key("omegon-primary").unwrap();
let tls_key    = deriver.derive_tls_certificate_key("auspex/control").unwrap();

// All keys are deterministic: same root → same keys, always.
```

### Derive identity-bound certificates

```rust,no_run
use styrene_identity::pki::derive_server_certificate_chain;
use styrene_identity::signer::RootSecret;

let root = RootSecret::new([0x42u8; 32]);
let chain = derive_server_certificate_chain(
    &root,
    "auspex-control/dev",
    "omegon-primary",
    ["omegon-primary.default.svc", "127.0.0.1"],
)?;

let cert_chain_pem = chain.cert_chain_pem();
let private_key_pem = chain.leaf.private_key_pem();
let ca_bundle_pem = chain.ca_bundle_pem();
# Ok::<(), styrene_identity::pki::StyrenePkiError>(())
```

The `pki` feature issues deterministic Ed25519 X.509 material for local
control planes, Kubernetes TLS Secrets, and mTLS client identities. It does
not persist private keys; callers decide whether a deployment target needs
an in-memory listener, a Kubernetes Secret, or another secret-grant path.

### Public key derivation

```rust
use styrene_identity::derive::{KeyDeriver, KeyPurpose};
use styrene_identity::pubkey::{ed25519_verifying_key, x25519_public_key};

let deriver = KeyDeriver::new(&[0x42u8; 32]);

let git_vk = ed25519_verifying_key(&deriver.derive(KeyPurpose::GitSigning));
let age_pk = x25519_public_key(&deriver.derive(KeyPurpose::Age));
```

### Lifecycle management with IdentityVault

```rust,no_run
use styrene_identity::vault::IdentityVault;
use styrene_identity::file_signer::ClosurePassphraseProvider;

let provider = Box::new(ClosurePassphraseProvider::new(|| {
    Ok(b"my-passphrase".to_vec())
}));
let vault = IdentityVault::with_default_path(provider);

// Create — refuses to overwrite (O_EXCL, no TOCTOU race)
vault.init(b"my-passphrase").unwrap();

// Backup before risky operations
vault.backup("/tmp/identity.key.bak").unwrap();

// Check existence
assert!(vault.exists());
```

## Derivation hierarchy

```text
root_secret (32 bytes)
  │
  HKDF-Extract(salt="styrene-identity-v1", IKM=root_secret) = PRK
  │
  ├─ Expand(PRK, "styrene-rns-encryption-v1")      → RNS X25519
  ├─ Expand(PRK, "styrene-rns-signing-v1")          → RNS Ed25519
  ├─ Expand(PRK, "styrene-yggdrasil-v1")            → Yggdrasil Ed25519
  ├─ Expand(PRK, "styrene-wireguard-v1")            → WireGuard Curve25519
  ├─ Expand(PRK, "styrene-ssh-host-v1")             → SSH host Ed25519
  ├─ Expand(PRK, "styrene-age-v1")                  → age X25519
  ├─ Expand(PRK, "styrene-rns-signing-v1")          → identity and legacy commit signing Ed25519
  │
  ├─ SSH user keys (two-level HKDF)
  │   salt="styrene-identity-ssh-user-v1"
  │   ├─ "github"  → per-host SSH Ed25519
  │   └─ "work"    → per-host SSH Ed25519
  │
  ├─ Agent signing keys (two-level HKDF)
  │   salt="styrene-identity-agent-v1"
  │   ├─ "omegon-primary"   → agent commit signing Ed25519
  │   └─ "omegon-cleave-0"  → worker commit signing Ed25519
  │
  ├─ Repository signing keys (two-level HKDF)
  │   salt="styrene-identity-repository-signing-v1"
  │   └─ "styrene-repository-signing-epoch-v1\0" || u32be(epoch)
  │
  └─ TLS certificate keys (two-level HKDF)
      salt="styrene-identity-tls-cert-v1"
      ├─ "auspex-control/dev/ca"         → scoped CA Ed25519 key
      ├─ "auspex-control/dev/server/0"   → server certificate Ed25519 key
      └─ "auspex-control/dev/client/0"   → client certificate Ed25519 key
```

Parameterized key families use two-level HKDF with distinct salts per family.
Collisions between flat purposes, SSH user keys, agent keys, and TLS
certificate keys are **structurally impossible** — they derive from different
IKM, different salts, and different HKDF trees.

## Identity-bound PKI

Styrene PKI is for control-plane transport identity, not user anonymity. A
certificate chain is bound to the canonical Styrene identity hash and a scoped
label. The default URI SANs use the `spiffe://styrene.dev` namespace:

| Role | URI shape | Intended use |
|------|-----------|--------------|
| CA | `spiffe://styrene.dev/identity/{hash}/ca/{scope}` | Scoped trust anchor |
| Server | `spiffe://styrene.dev/identity/{hash}/agent/{label}` | Omegon/Auspex control plane listener |
| Client | `spiffe://styrene.dev/identity/{hash}/client/{label}` | mTLS caller identity |

Rotation is explicit:

| Rotation knob | Changes | Keeps stable |
|---------------|---------|--------------|
| `leaf_epoch` | Server/client leaf key, cert, fingerprint | Scoped CA |
| `ca_epoch` | Scoped CA and every issued leaf chain | Root Styrene identity |
| Root secret | Everything | Nothing |

Use different CA scopes for different trust domains, such as
`auspex-control/dev`, `auspex-control/prod`, or a Kubernetes namespace.
Use leaf epochs for routine certificate rollover and CA epochs for trust-anchor
rotation.

Certificate seed labels are composed with length-prefixed components under the
`styrene/tls/v2` namespace. This keeps profiles, scopes, labels, and epochs
unambiguous even when they contain path separators.

## Signer tiers

The `IdentitySigner` trait exposes a root secret through custody adapters.
Multiple adapters represent the same identity only when provisioned with the
same root. Neither the trait nor `SignerChain` verifies that equivalence.

| Tier | Backend | Feature | Status |
|------|---------|---------|--------|
| A | YubiKey FIDO2 hmac-secret | `yubikey` | Implemented |
| B | Apple Keychain / Android Keystore | `keychain`, `android-keystore` | Implemented |
| C | Bitwarden / 1Password | — | Planned |
| D | Encrypted file (argon2id + ChaCha20Poly1305) | `file-signer` (default) | Implemented |

`SignerChain::new_sorted` sorts by tier; `new` preserves caller order. Both use
the first available signer and propagate its operation error without retrying
other signers. Availability is a hint, not proof of successful authentication:

```rust,ignore
use styrene_identity::signer::SignerChain;

let chain = SignerChain::new_sorted(vec![
    Box::new(yubikey_signer),  // tried first
    Box::new(file_signer),     // used if earlier signers report unavailable
]);
let root = chain.root_secret().await?;
```

## Feature flags

The [read-only overview client](docs/read-only-overview.md) is available without
default features. It supplies typed public summaries, expected-identity checks,
and stale-result filtering for the first UI extension slice. Production custody
sources and UI integration remain pending.

| Feature | Default | Enables |
|---------|---------|---------|
| `file-signer` | **yes** | `FileSigner`, `IdentityVault` (argon2, chacha20poly1305) |
| `signing` | via file-signer | `pubkey` module (ed25519-dalek, x25519-dalek) |
| `repository-signing` | no | repository authority bindings and strict Ed25519 verification |
| `age-format` | no | age Bech32 key encoding |
| `pki` | no | identity-bound X.509 CA/client/server certificates (rcgen) |
| `yubikey` | no | `YubiKeySigner` (FIDO2 hmac-secret) |
| `keychain` | no | device-bound Apple Keychain signer, available after first unlock |
| `android-keystore` | no | Android Keystore-wrapped root secret |
| `ssh-agent` | no | `StyreneAgent` SSH agent protocol |

Minimal dependency footprint — disable `default-features` and pick only
what you need:

```toml
# Derivation and core contracts, without the file signer
styrene-identity = { git = "https://github.com/styrene-lab/styrene-identity", rev = "4ea2d0c7a6e5c89e09c6ba05dead917dc5b33e9a", default-features = false }

# Derivation + public key helpers, no file signer
styrene-identity = { git = "https://github.com/styrene-lab/styrene-identity", rev = "4ea2d0c7a6e5c89e09c6ba05dead917dc5b33e9a", default-features = false, features = ["signing"] }

# Repository authority profile, no signer storage or transport
styrene-identity = { git = "https://github.com/styrene-lab/styrene-identity", rev = "4ea2d0c7a6e5c89e09c6ba05dead917dc5b33e9a", default-features = false, features = ["repository-signing"] }

# Deterministic X.509 issuance for control-plane TLS/mTLS
styrene-identity = { git = "https://github.com/styrene-lab/styrene-identity", rev = "4ea2d0c7a6e5c89e09c6ba05dead917dc5b33e9a", default-features = false, features = ["pki"] }

# Full file-based identity (default)
styrene-identity = { git = "https://github.com/styrene-lab/styrene-identity", rev = "4ea2d0c7a6e5c89e09c6ba05dead917dc5b33e9a" }
```

## File format

The Tier D identity file (`~/.config/styrene/identity.key`) is 97 bytes:

```text
STID [version:1] [salt:32] [nonce:12] [ciphertext:32+16]
 4B      1B          32B       12B          48B
```

- **Encryption**: argon2id (m=64MiB, t=3, p=1) → ChaCha20Poly1305
- **Permissions**: 0o600, set atomically at creation via `O_EXCL`
- **Backward compat**: legacy 92-byte headerless files (pre-v1) are still readable

## Styrene Identity ID

The canonical Styrene Identity ID is SHA-256 of the canonical Ed25519 signing
public key, truncated to 16 bytes (32 hex chars). The canonical signing seed
shares the existing RNS signing derivation label. The Styrene Identity ID is
distinct from the RNS transport identity hash and the LXMF delivery destination.
See [identity terminology](docs/integration-context.md#distinguish-the-identifiers).

`IdentityId::from_public_key` implements this calculation. The equivalent
derivation is:

```rust
use styrene_identity::derive::{KeyDeriver, KeyPurpose};
use styrene_identity::pubkey::ed25519_verifying_key;
use sha2::{Digest, Sha256};

let deriver = KeyDeriver::new(&[0x42u8; 32]);
let seed = deriver.derive(KeyPurpose::RnsSigning);
let pubkey = ed25519_verifying_key(&seed);
let hash = Sha256::digest(pubkey.as_bytes());
let identity_hash = hex::encode(&hash[..16]); // 32 hex chars
```

## Git commit signing

Git commit signing authenticates an individual commit or tag. It does not
grant Styrene repository authority. `KeyPurpose::GitSigning` is a deprecated
alias for the canonical identity signing key and exists only for legacy commit
signing integrations.

Derived keys work with `git`'s SSH signing (`gpg.format = ssh`). Agent keys
enable cryptographic distinction between human and agent commits:

| Committer | Key | Comment in `git log --show-signature` |
|-----------|-----|---------------------------------------|
| Human | `GitSigning` | `styrene-git-signing` |
| Agent | `Agent("omegon-primary")` | `styrene-agent:omegon-primary` |

All keys trace back to the same root — one identity, multiple signers.

## Repository authority

Repository authority uses a separate epoch-indexed key family. An
Identity-issued `RepositorySignerBinding` binds one repository public key to
the canonical Identity ID, identity public key, epoch, purpose, and suite.
Consumers must verify the canonical binding before they apply repository
governance policy.

Enable only the required profile:

```toml
styrene-identity = { git = "https://github.com/styrene-lab/styrene-identity", rev = "4ea2d0c7a6e5c89e09c6ba05dead917dc5b33e9a", default-features = false, features = ["repository-signing"] }
```

`styrene-identity` verifies identity attribution and cryptographic validity.
It does not select epochs, delegates, transitions, or publisher namespaces.

## Security properties

- **Zeroize-on-drop** for all secret material (`RootSecret`, `KeyDeriver` PRK, `DerivedKeys`, derived seeds)
- **No private keys on disk** — the SSH agent derives keys in memory per request
- **Domain-separated HKDF** — fixed salt prevents collision with any other HKDF usage
- **Hardened KDF** — argon2id params exceed OWASP minimums
- **Atomic file creation** — `O_EXCL` prevents overwrites with no TOCTOU race
- **Credential injection** — passphrases and PINs via traits, never environment variables

See [SECURITY.md](SECURITY.md) for the full threat model and accepted risks.
See [COMPATIBILITY.md](COMPATIBILITY.md) for repository-signing release and
consumer support policy.

## Linkability warning

**All keys derived from one root are cryptographically linked.** This is
by design for attribution and recovery, but it means derived keys cannot
provide anonymity. For anonymous or pseudonymous identities, use an
independent root:

```rust
use styrene_identity::signer::RootSecret;

// Ephemeral: CSPRNG-generated, no file, zeroized on drop
let anon = RootSecret::ephemeral();

// Or: separate persistent identity
// nex identity init --path ~/.config/styrene/pseudonym.key
```

See [docs/unlinkability.md](docs/unlinkability.md) for the full model,
anti-patterns, and decision matrix.

## Test vectors

From a root secret of `0x42` repeated 32 times:

```text
RnsEncryption = aefdbd63fb6746c2edb73bba3bcb34f61909077f65fe033c9372b55f6ace0c0c
GitSigning    = 6eb3d3ef12a2447f6de281d6f896eba20ad0b0add3bc6fce80499f36b7343842
SSH(github)   = 3c261af80e084a637fd20e0f7274a4106702894f0d23c47e855f6c9adce20d75
Agent(omegon) = 4dd66edcda091a5e3d15aa3fb8ec32d81e212d94760b61915b1d6f204b0672e2
TLS(auspex/control) = bdbce0671a517c65205339d22d04adecc45b588396d8b4762ddedb71cd390ec6
```

These are pinned in the test suite. Any implementation of the derivation
hierarchy must reproduce them.

## Historical ecosystem usage

The following table preserves pre-extraction integration context. Dependency
forms and versions are historical, not verified current consumer configurations.
For new integrations, use a reviewed immutable Git revision as shown above and
follow the [consumer handoff procedure](CONTRIBUTING.md#consumer-handoff-and-publication).

| Crate/Binary | Dependency | Purpose |
|------|------------|---------|
| **nex** | `styrene-identity = "0.1"` | `nex identity init/show/link` — generate and manage identities |
| **aether** | path dep | Mesh node identity and RBAC |
| **auspex** | path dep (`pki`) | Operator identity and scoped control-plane TLS for managed agents |
| **vox** | path dep | LXMF mesh identity |
| **styrened** | workspace member | Daemon identity — RNS, SSH agent, mesh signing |

## License

MIT
