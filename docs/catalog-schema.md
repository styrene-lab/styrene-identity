# Public catalog schema v1

## Implemented public storage contract

`styrene-identity-lifecycle::CatalogSnapshot` reads `<store>/catalog.json`.
The CLI also writes it through [file-backed catalog CRUD](file-custody-crud.md).
Read-only clients need no custody feature. Do not treat manual catalog edits as
authenticated enrollment or a replacement for custody operations.

The catalog contains public metadata and local reference IDs. It contains no
roots, private keys, credentials, custody paths, or decrypted backups. It is not
signed; its public key/ID checks establish hash consistency, not authenticity,
possession, freshness, hardware custody, or active-session binding.

## Schema

Example for a disposable public-metadata fixture:

```json
{
  "schema_version": 1,
  "revision": 0,
  "preferred_entry": null,
  "entries": [
    {
      "entry_id": "example-entry",
      "revision": 0,
      "display_name": "Example",
      "public_identity": null,
      "custody_refs": []
    }
  ]
}
```

An initialized empty catalog has `entries: []` and no preferred entry. A missing
catalog returns `catalog_uninitialized`; reads never create the store or file.

| Field | Contract |
|---|---|
| `schema_version` | Required unsigned integer; only 1 is supported |
| Catalog/entry `revision` | Required unsigned 64-bit integer; catalog increments per mutation, entry increments per metadata update |
| `preferred_entry` | Optional/null entry ID; a non-null value must refer to an existing entry |
| `entries` | Required array, at most 1024 entries; unique entry IDs |
| `entry_id` | 1–64 ASCII letters, digits, hyphens, or underscores |
| `display_name` | Optional/null; otherwise nonblank, at most 128 UTF-8 bytes, no control characters |
| `public_identity` | Optional/null, or an object with both `identity_id` and `public_key` |
| `public_identity.identity_id` | Exactly 32 lowercase hexadecimal characters parsed by `IdentityId` |
| `public_identity.public_key` | Exactly 64 lowercase hexadecimal characters; its SHA-256 prefix must match `identity_id` |
| `custody_refs` | Required array of at most 32 distinct IDs using the entry-ID character/length rules |

Custody references are catalog metadata, not proof of attached/available custody.
Different entries may currently reference the same custody ID. Actual attachment
ownership and uniqueness rules require the later custody catalog contract.

The mutation service retains the private file locator and recovery outcome in its
operation journal. These fields are not added to public `catalog.json`. Creation
and adoption register one custody reference and reject an already registered
canonical identity. Forgetting preserves custody files and journal records.

Unknown fields and duplicate known fields are rejected. Future valid schema
versions return `unsupported_schema` before v1 field validation. Malformed JSON
returns `invalid_catalog`. Newer catalog formats need explicit reader/migration
review; an older reader does not rewrite them. This strict storage policy is
separate from additive fields in CLI output schema v1.

The loader bounds input to 1 MiB. On Unix, descriptor-relative no-follow/nonblocking
opens reject catalog symlinks and nonregular files; directory binding checks reject
substitution. Storage remains local application state, not a sandbox or signed
anti-rollback authority. Mutation use also checks file ownership and write permissions.
The parser validates every entry
before returning an inventory, so one invalid binding fails the complete read.
It sorts output by entry ID for deterministic listing.

## Selection and overview integration

Exact entry IDs take precedence over display names. Otherwise a name must match
exactly one entry. Multiple matches return `ambiguous_name`; no matches return
`entry_not_found`. Names do not authorize operations.

`CatalogSnapshot::selection` issues a token local to that immutable snapshot.
The snapshot implements `IdentityOverviewSource` with no further I/O or custody
access. When replacing the snapshot, the UI must allocate a new observation scope,
even if numeric selection tokens are reused. Old snapshots cannot describe new
storage state. Their summaries must not be presented as live custody evidence.

`CatalogSnapshot::show` optionally checks an expected canonical ID. Missing public
identity produces `identity_unavailable` when an expected ID is required; discovery
otherwise returns explicit `unavailable/not_known` status. A mismatch returns
`identity_mismatch`. CLI and overview clients share the same validated public data.

`load` performs blocking filesystem I/O. Async application hosts must call it on
their blocking worker, then use the immutable public snapshot for overview requests.
