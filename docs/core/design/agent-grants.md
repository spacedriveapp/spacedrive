# Agent Grants

> **Status:** Built (grants core, signed tokens, standing grants, grants as space assertions). The HTTP surface that consumes grants is separate work.
> **Narrows:** `docs/core/design/shares-and-remote-libraries.md`, phase 1, for agent recipients.
> **Code:** `core/src/ops/grants/`, `apps/cli/src/domains/grant/`

## What a grant is

A grant lends one directory of one tracked source to an agent, for a bounded time. The owner creates it, hands the agent a bearer token, and can end it at any moment.

```
grant:
  id            uuid
  source_id     the filesystem source holding the directory
  subtree       canonical absolute path of the directory
  space_id      the space holding the directory, for grants made since spaces
  actor_id      the agent the grant acts as
  kind          task | standing
  token_format  opaque | signed
  epoch         the space's authority epoch at minting
  caps          read | read_write
  expires_at    see Lifetime
  revoked_at    set once by grants.revoke
  created_by    the device that minted it
  label         the owner's name for it
```

The minting device keeps the row in `library.db` in the `agent_grant` table, with the token hash and, for OAuth grants, the refresh hash. Those secrets never leave the device.

A grant inside a space is also an assertion in the space's source store (`data.db`), in three tables beside `space`:

- `space_grant` holds the grant: space, actor, kind, token format, subtree relative to the space root, caps, issuer device, epoch and expiry. It holds no token and no token hash. The later HLC stamp wins the whole row, so an extension made on one device reaches the others.
- `grant_revocation` holds one row per revoked grant. It is separate from the grant row, so no merge order can undo a revocation. Two revocations of one grant converge on the earlier stamp.
- `grant_issuer` holds the Ed25519 public key each minting device signs tokens with.

The rows merge by the assertion rules in `source-durability.md`, not through `Syncable`. Any node that holds the store knows which grants exist on the space and which are revoked, without asking the minting device.

The directory must be inside a source tracked by the same library: a filesystem source, or a bucket source. A bucket grant names a prefix as a URL, `s3://bucket/prefix`, and the prefix must sit inside a bucket source of the library. See "Bucket prefixes" below. A grant is a scope over something Spacedrive already tracks, not a way to reach arbitrary folders on the node. Removing the source revokes every grant on it. A kept catalog keeps the source id, so without the revocation, re-adding the same scope would revive the old tokens.

## Tokens

A token is `sdg_` followed by 64 hex characters, which encode 32 bytes from the operating system's random source. The prefix lets a person or a secret scanner recognize a leaked token.

A grant can also be minted through the MCP endpoint's OAuth flow (`docs/cli/mcp-oauth.mdx`). The owner picks the folder, caps and lifetime on a consent page the daemon serves, and the access token the client receives is the grant's own `sdg_` token, so `authorize` treats it like any other. Such a grant records its client in `oauth_client` and holds the hash of a refresh token, `sdr_` and 64 hex characters, in `refresh_hash`. A refresh rotates both hashes on the same row and records the replaced refresh hash in `agent_grant_spent_refresh`; presenting any spent refresh token again revokes the grant. Spent hashes of ended grants are pruned on the next rotation. A refresh never moves `expires_at`.

Spacedrive stores only the SHA-256 of the token. `grants.create` returns the token once. No list, audit or error message shows it again. A copy of `library.db` contains no usable credential.

Lookup is by hash. A string without the prefix or of the wrong length never reaches the database.

### Signed tokens

An opaque token means something only to the device that holds its hash. A signed token carries its own claims, so any node that holds the space's store can check it:

```
sdg_s.<claims>.<signature>
claims     {v, grant_id, space_id, subtree, caps, exp, epoch, iss} as JSON, base64url
signature  Ed25519 over "spacedrive grant token v1\0" followed by the claims bytes, base64url
```

`subtree` is relative to the space root, so it means the same folder on every node that holds the space. `iss` is the minting device. The signature covers the bytes as sent, so no re-serialization can disagree about what was signed. The domain label keeps a grant signature from being mistaken for any other signature the key makes. The prefix starts with `sdg_`, so every URL check, log filter and secret scanner that matches opaque tokens matches signed ones. `token::hash` never accepts one.

The signing key is derived from the device master key with its own HKDF label. It is never stored and is separate from the network identity key. Minting a signed grant publishes the device's public key in the space's store.

A signed grant needs a space. Its expiry is in its claims, so it cannot be extended; the owner creates a new grant. A token over 4,096 bytes is never signed or accepted.

## Lifetime

A grant has one of two kinds:

| Kind | Lifetime | Default | Where | Default token |
| --- | --- | --- | --- | --- |
| `task` | 5 minutes to 12 hours | 1 hour | any granted folder | opaque |
| `standing` | 5 minutes to 30 days | 30 days | inside a `workspace` space only | signed |

A standing grant is for an agent's own workspace, a folder Spacedrive created empty for it. It lets a harness hold one credential across tasks without reaching anything the person keeps elsewhere. `grants.create` refuses a standing grant on a folder space or on a folder in no space.

`grants.extend` sets a new expiry measured from now, with the bounds of the grant's kind. So no sequence of extensions keeps a task grant alive more than 12 hours past its last extension, or a standing grant more than 30 days. A revoked or expired grant cannot be extended, and neither can a signed one. The owner creates a new grant instead. An extension of a grant in a space is written to its assertion too.

## The enforcement point

Every agent-facing surface checks every request with one function:

```rust
pub struct Access<'a> {
	pub token: &'a str,
	pub op: &'a str,          // the surface's name for the request, recorded in the audit
	pub space: Option<Uuid>,  // the space the request names; None means the grant's own
	pub path: &'a Path,       // relative to the grant root, or absolute
	pub cap: GrantCap,        // Read or ReadWrite
	pub epoch: Option<u64>,   // the authority epoch a write was made against; None for reads
	pub meta: RequestMeta,    // client address, session and pattern, for the audit
}

sd_core::ops::grants::authorize(context: &CoreContext, access: &Access<'_>)
	-> Result<Authorized, GrantError>  // Authorized { scope: GrantScope, audit: AuditId }
```

`authorize_with` and `authorize_in` remain as wrappers for surfaces that name no space and no epoch. `authorize_in` checks one library's grants only.

`authorize` first verifies the token and finds its grant. It then resolves the grant's space. A grant row that records a space, as every grant made inside one does, uses it. An older grant belongs to the innermost space of its source whose root holds the granted directory. A grant outside every space is its own space, with the grant id as its id. A request naming another space is refused with `GrantError::UnknownSpace`, which an HTTP surface answers like a missing space, with 404. The granted directory remains the containment limit even inside a larger space.

A write that names an epoch is checked against the space's authority epoch and refused with `GrantError::StaleEpoch { current }` when they differ. An HTTP surface answers 409. Spaces store no epoch yet, so the current epoch is 0: `Some(0)` passes and any other value is stale. `None` is not checked.

On success, `GrantScope` carries the grant id, the library id, the space id and its authority epoch, the canonical root, and the resolved `path` the surface must open. The surface never opens the path the agent sent. `GrantScope::path` is spelled under the grant root whatever spelling the request used.

`authorize` has one verifier, a private step that turns a token into the grant it is a credential for:

- An opaque token is looked up by hash in each open library's `agent_grant` table.
- A signed token names its space and issuer. The verifier finds the store that holds the space among the library's sources, reads the issuer's public key from that store's `grant_issuer`, and checks the signature. A malformed token, an altered byte, another key, or a space whose store never published the issuer all answer `UnknownToken`. A genuine token past its `exp` answers `Expired` and is audited against its grant.

For both formats the verifier reads the revocation on every call: the grant row's `revoked_at` and, for a grant in a space, the `grant_revocation` assertion. A revocation made on another device and merged into the store refuses the next request here. Nothing is cached. The rest of `authorize` (path resolution, protected names and the audit) is the same for both formats.

`GrantError::is_credential` separates refusals of the token (unknown, expired, revoked, source removed) from refusals of one request (read-only grant, path outside the grant, protected file, another space, a stale epoch). An HTTP surface answers the first group with 401 and the second with 403, except 404 for another space and 409 for a stale epoch.

The local RPC socket does not use grants. It stays as trusted as before. Grants apply only to surfaces built for agents.

## Path resolution

A string prefix check cannot keep a request inside a directory. `authorize` resolves the path in five steps:

1. A `..` component anywhere in the request is refused. No legitimate client needs one.
2. A relative request is joined to the grant root. An absolute request is taken as given.
3. `canonicalize` follows every symlink in the deepest part of the path that exists. A path that does not exist yet, such as the target of a create, resolves through its nearest existing ancestor. If any part of the path is a dangling symlink, the request is refused, because a write through it would create a file wherever the link points.
4. The resolved directory is inside the grant only when the grant root is one of its ancestors. A plain prefix test settles the common case. Otherwise ancestors are compared with the root by filesystem identity: device and inode on Unix, volume and file index on Windows.
5. The grant root itself must still be a real directory. A root that was replaced by a symlink is unavailable.

Identity comparison handles the spellings that a prefix test gets wrong in either direction:

- **Symlinks out of the directory** resolve to their target, which is outside, so they are refused.
- **Firmlinks.** macOS reaches the same directory as `/Users/me` and `/System/Volumes/Data/Users/me`, and `realpath` resolves neither to the other. Both spellings have one identity, so a request through either one is inside exactly when it names the granted directory. The firmlink handling added for file info and the watcher relies on the same fact.
- **Case-insensitive aliases.** On APFS, NTFS or casefolded ext4, `Granted` and `GRANTED` are one directory with one identity. On a case-sensitive volume they are two directories, and the other one is outside.

### Bucket prefixes

A bucket has no symlinks, firmlinks or case aliases, so a bucket grant resolves a request by its spelling. Every component must be a plain name: `..`, an absolute path and a URL outside the granted prefix are refused. The key is the prefix with the request joined below it, compared by whole components, so a grant on `s3://media/projects` never reaches `s3://media/projects-old`. Protected names are checked the same way as on a filesystem source. The backend comes from the bucket's cloud volume, connected with `volumes.add_cloud`. A writable request on a service without conditional writes is refused with `NoConditionalWrites`.

## Protected files

A writable grant can never modify:

- `.git` at any depth, and everything under it
- any file named `AGENTS.md`, `CLAUDE.md` or `SKILL.md`, at any depth
- anything matching a default pattern, at any depth: `GEMINI.md`, `AGENT.md`, `.cursor/rules/**`, `.github/workflows/**` and `.env*`
- anything matching a pattern of a space that holds the path

These names are compared the way the most permissive filesystem compares them: case-folded, with the Unicode code points that HFS+ ignores removed, with trailing dots and spaces that Windows drops removed, with an NTFS stream suffix removed, and including the 8.3 short names `GIT~1`, `AGENTS~1.MD` and `CLAUDE~1.MD`. Both the spelling the agent sent and the resolved path are checked. So a symlink named `notes.md` that points at `AGENTS.md` is refused for writing.

Patterns are globs over `/`-separated paths. A pattern that starts with `/` is anchored at the space root. Any other pattern matches at every depth, so `*.pem` covers `keys/id.pem`. A match covers everything under it, and `dir/**` also covers `dir` itself. Names are folded as above before they are compared, and patterns are lowercased. The 8.3 short names of pattern matches are caught through the resolved path, which names the long form.

A space's patterns are set with `spaces.create` or `spaces.update` (`sd space create --protect`). They live on the space in its source store, and `authorize` reads them on every request, so a change applies to the next one. Every space that holds a path contributes its patterns, so a nested space can add protection but never remove it. Moving or deleting a directory checks every path under it against the same lists.

Reads of these files are allowed.

A surface that changes a directory as a whole, such as a recursive delete or move, has to authorize each path it touches. Authorizing the directory does not authorize what is inside it.

## Audit

Each decision against a known grant is recorded in the `grant_audit` table, whether the request was allowed or refused, before the request is answered. A record holds:

- the time, the grant id and the grant's label
- the client address and the MCP session id, on surfaces reached over the network
- the operation, such as `mcp.read`, and the path as the agent sent it
- the grep or glob pattern, cut to its first 256 bytes
- the bytes of the response body, the result (`ok`, `denied`, `limited` or `error`), the duration, and the reason for a refusal

A record never holds the token, file contents or match text.

`authorize` writes the decision. A surface that knows more about the request calls `authorize_with` with a `RequestMeta` (client address, session, pattern). It gets back the scope and the id of the row, and calls `audit::finish` with the bytes, result and duration once it has the response body and before it sends it. If either write fails, the request is not served. The agent endpoint answers 503, on its MCP and REST routes alike.

Some requests are counted instead of recorded one by one, so a flood cannot fill the table. They go to `grant_audit_tally`, one row per kind, subject and minute with a count:

- `unknown_token`: a token that matches no grant, per client address. Nothing says which library the token was meant for, so the count goes to every open library. This count is best-effort: if it cannot be written, the failure is logged and the request still answers 401, so the per-address lockout keeps counting.
- `locked_out`: a request from an address locked out after failed authentications, per client address.
- `rate_limited`: a request over its grant's request rate, per grant. Its decision row is folded into the count.

Retention is 30 days or 1,000,000 rows per table, whichever comes first. The trim runs on about one in 1,000 audit writes to each library, so a table can hold some rows over the cap between two passes.

`grants.list` shows the last allowed use and the request count of each grant, both within retention. `grants.audit` returns the rows and the tallies, newest first, for one grant or for all grants.

## Ops and CLI

| Op | Kind | Does |
| --- | --- | --- |
| `grants.create` | library action | Grant a directory, as a task or standing grant, with an opaque or signed token. Returns the grant and its token, once. |
| `grants.list` | library query | Active grants, or all with `include_inactive`. Never returns tokens. |
| `grants.revoke` | library action | End a grant now. |
| `grants.extend` | library action | Set a new expiry from now. |
| `grants.audit` | library query | The audit rows and tallies, newest first. |

```
sd grant create <path> [--read | --read-write] [--ttl 1h] [--standing] [--signed | --opaque] [--label name]
sd grant list [--all]
sd grant revoke <id>
sd grant extend <id> [--ttl 1h]
sd grant audit [<id>] [--limit 50]   # also: sd grant log <id>
```

The CLI binary is `sd-cli`. `--ttl` takes values such as `30m`, `1h`, `2h30m` or `7d`.

## Known limits

- **Time of check and time of use.** `authorize` resolves the path, then the surface opens it. A process that can create symlinks inside the directory could swap one in between. A read-only grant cannot create links, so this matters only for writes. The write path resolves the request again just before its rename and refuses a path that no longer resolves to the same place, which narrows the window to the rename itself. See [agent-writes.md](agent-writes.md).
- **Hard links.** A hard link to a protected file under another name is a different name for the same bytes. It is not detected, and it does not need to be: agent writes never modify a file in place. They stage new bytes and rename them over the name, so the other name keeps the old bytes. Git does not create hard links into a working tree, and no agent surface can create one.
- **Sync transport.** Grant assertions merge by the rules in `source-durability.md`, but no transport carries assertions between devices yet. Until one does, a replica honours a signed grant only once it holds a copy of the space's store with the grant's issuer key, and learns of a revocation only when the store reaches it.
- **Opaque tokens stay on the minting device.** Their hash never leaves `library.db`, so only that device can check one. Use a signed token for a grant another node must honour.
- **Issuer trust is the store's.** A replica trusts whichever issuer keys the space's store publishes. Anyone who can write that store can already change its grants, so this adds no new writer, but a store must only be merged from devices of the library.
- **Authority epoch.** Grants record the space's authority epoch, which is 0 until hand-over exists. Refusing grants minted under an older epoch is the endpoint's check.
