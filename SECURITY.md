# Security Model — styrene-identity

## Cryptographic Architecture

- **HKDF-SHA256** with fixed domain-separation salt (`styrene-identity-v1`)
- **Two-level derivation** for parameterized families (agent keys, SSH user keys) with distinct level-2 salts (`styrene-identity-agent-v1`, `styrene-identity-ssh-user-v1`)
- **Pinned test vectors** ensure derivation stability across versions
- **Ed25519** signing via `ed25519-dalek` (pinned to `=2.1.1`)
- General Identity verification uses strict Ed25519 checks. See the
  [2026-09-06 adversarial review](docs/adversarial-review-2026-09-06.md) for the
  reproduced weak-key forgery, local fix, and consumer/release implications.
- **Argon2id** with hardened parameters (m=64MiB, t=3, p=1) for file-based encryption
- **Versioned file format** with `STID` magic bytes and version marker (backward-compatible with legacy headerless files)

## Key Material Lifecycle

- `RootSecret`: Zeroize-on-drop, Debug-redacted
- `KeyDeriver`: PRK stored as `[u8; 32]` with explicit `Drop` zeroization. No non-zeroizable `Hkdf` struct is persisted.
- `DerivedKeys`: Zeroize-on-drop via `#[zeroize(drop)]`
- File signer: generated roots, KDF buffers, and decrypted plaintext use RAII
  zeroization, including the owned Argon2 block workspace. Passphrase providers
  remain caller-controlled. Transient library/OS/caller copies can still remain.
- SSH agent: Private seeds derived on-demand per `sign()` call, zeroized immediately after signing. Public key map holds no private material.

## Secret Input Handling

Passphrases and PINs enter through `PassphraseProvider` and `PinProvider`.
The crate has no built-in environment-variable provider. Caller implementations
can still read environment variables; the traits do not enforce credential origin.
Callers must select an appropriate protected input mechanism.

## Custody boundaries

`IdentitySigner::root_secret()` returns secret bytes to the caller. Current
adapters derive and sign in process memory. YubiKey keeps its credential secret
on the token but returns the FIDO2-derived root. Apple Keychain stores a
retrievable root; Android Keystore wraps a root with an AES key. Neither adapter
implements Styrene Ed25519 signing inside a Secure Enclave or StrongBox.

`SignerTier` is a classification, not hardware attestation. `is_available()` is
an adapter-specific hint, not proof of unlock, user verification, or successful
signing. `SignerChain` selects the first available adapter and returns its error
without retrying. It does not verify that adapters have the same Identity ID.

Zeroization protects the documented buffers, not every copy made by callers,
libraries, operating systems, or crash collection. See accepted risks below.

## File Permissions

Identity files are written with `mode(0o600)` set atomically at creation time via `OpenOptions` on Unix. No TOCTOU race between creation and permission setting.

The lifecycle service additionally uses descriptor-relative no-follow filesystem
operations, directory identity binding for pending custody writes, and ownership/
permission checks. These guarantees do not apply automatically to all legacy
`FileSigner` callers. See [file-backed CRUD](docs/file-custody-crud.md).

## Recovery records and backup evidence

Prepared v2 journals retain encrypted creation material until custody/catalog
commit. Completed v2 receipts drop that ciphertext and the full catalog snapshot.
Legacy v1 records may still retain recovery copies; they are reported explicitly
and are not silently erased. Reprotection of one file does not revoke an old
password's access to copies elsewhere. Unlinking files is not secure erasure.

Current STID and legacy backup headers are not authenticated. Payload authentication
does not prove header provenance or external identity ownership. Journals/catalogs
are local state, not signed anti-rollback authorities. See the adversarial review.
Encrypted custody does not authenticate an independently modified catalog or request
history. Strong expected-identity and authorization decisions need trusted consumer
context outside self-asserted metadata.

## Identity Linkability

All keys derived from one root secret are deterministically linked. This is a feature for attribution and recovery, but a liability for anonymity. Derived keys do not provide unlinkability or deniability.

For anonymous or pseudonymous use cases, use `RootSecret::ephemeral()` (no persistence, CSPRNG-generated, zeroized on drop) or a separate identity file. See `docs/unlinkability.md` for the detailed threat model, anti-patterns, and decision matrix.

## Accepted Risks

### A1. `Hkdf::from_prk()` intermediates not zeroized

`KeyDeriver::expander()` reconstructs an `Hkdf` from stored PRK bytes on each derivation call. The `hkdf` crate's `Hkdf` struct does not implement `Zeroize`. These transient stack-allocated values are dropped at end of scope but not explicitly wiped.

**Rationale**: The PRK bytes themselves (which are root-equivalent) *are* zeroized on `KeyDeriver::drop()`. The transient `Hkdf` wrapper exists only for the duration of a single `expand()` call. The risk is stack residue, which requires memory forensics to exploit.

### A2. 128-bit identity identifiers

The canonical Styrene Identity ID uses truncated SHA-256 (16 bytes / 128 bits).
RNS transport hashes also use a 128-bit identifier, but identify different public
inputs and are not substitutes for the Styrene ID. A 128-bit hash provides:
- 2^128 preimage resistance (infeasible for targeted attacks)
- Approximately 2^64 work for generic collision search

Consumers must verify the expected identity/public-key binding and strict signature,
not treat an identifier or a self-asserted public key as authorization.

### A3. No replay protection in `sign()` trait

The `IdentitySigner::sign()` method signs arbitrary data with no built-in nonce, timestamp, or sequence number. Replay protection is the responsibility of the protocol layer (e.g., RNS link establishment, LXMF message IDs, aether request correlation).

### A4. SSH agent double `root_secret()` on sign

The SSH agent calls `root_secret()` twice per `sign()` request — once to build the public key map, once to derive the matching seed. For hardware signers (YubiKey), this makes two root requests; physical interaction depends on token policy. This is a deliberate trade-off: the alternative (caching all seeds) would hold all private key material in memory simultaneously, increasing the blast radius of a memory disclosure.

### A5. Non-Unix file permissions

On non-Unix platforms, the identity file is written without platform-specific ACL restrictions. The file-signer is effectively Unix-only in production deployments.

### A6. `HOME` fallback

`FileSigner::default_path()` falls back to `"."` if `HOME` is unset. Callers (styrened, CLI tools) should always provide an explicit path rather than relying on the default.
