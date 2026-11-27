# Agent Grants

> **Status:** Built (grants core). The HTTP surface that consumes grants is separate work.
> **Narrows:** `docs/core/design/shares-and-remote-libraries.md`, phase 1, for agent recipients.
> **Code:** `core/src/ops/grants/`, `apps/cli/src/domains/grant/`

## What a grant is

A grant lends one directory of one tracked source to an agent, for a bounded time. The owner creates it, hands the agent a bearer token, and can end it at any moment.

```
grant:
  id          uuid
  source_id   the filesystem source holding the directory
  subtree     canonical absolute path of the directory
  caps        read | read_write
  expires_at  5 minutes to 12 hours after creation, 1 hour by default
  revoked_at  set once by grants.revoke
  created_by  the device that minted it
  label       the owner's name for it
```

The row lives in `library.db` in the `agent_grant` table. It is small shared state under the rules in `AGENTS.md`: no ingest can rebuild it, and it has no place in a source store. In this version a grant stays on the node that minted it and does not sync.

The directory must be inside a source tracked by the same library: a filesystem source, or a bucket source. A bucket grant names a prefix as a URL, `s3://bucket/prefix`, and the prefix must sit inside a bucket source of the library. See "Bucket prefixes" below. A grant is a scope over something Spacedrive already tracks, not a way to reach arbitrary folders on the node. Removing the source revokes every grant on it. A kept catalog keeps the source id, so without the revocation, re-adding the same scope would revive the old tokens.

## Tokens

A token is `sdg_` followed by 64 hex characters, which encode 32 bytes from the operating system's random source. The prefix lets a person or a secret scanner recognize a leaked token.

A grant can also be minted through the MCP endpoint's OAuth flow (`docs/cli/mcp-oauth.mdx`). The owner picks the folder, caps and lifetime on a consent page the daemon serves, and the access token the client receives is the grant's own `sdg_` token, so `authorize` treats it like any other. Such a grant records its client in `oauth_client` and holds the hash of a refresh token, `sdr_` and 64 hex characters, in `refresh_hash`. A refresh rotates both hashes on the same row and records the replaced refresh hash in `agent_grant_spent_refresh`; presenting any spent refresh token again revokes the grant. Spent hashes of ended grants are pruned on the next rotation. A refresh never moves `expires_at`.

Spacedrive stores only the SHA-256 of the token. `grants.create` returns the token once. No list, audit or error message shows it again. A copy of `library.db` contains no usable credential.

Lookup is by hash. A string without the prefix or of the wrong length never reaches the database.

## Lifetime

A grant lives between 5 minutes and 12 hours, and 1 hour by default. `grants.extend` sets a new expiry measured from now, with the same bounds. So no sequence of extensions keeps a grant alive more than 12 hours past its last extension. A revoked or expired grant cannot be extended. The owner creates a new grant instead.

## The enforcement point

Every agent-facing surface checks every request with one function:

```rust
sd_core::ops::grants::authorize(
	context: &CoreContext,
	token: &str,
	op: &str,       // the surface's name for the request, recorded in the audit
	path: &Path,    // relative to the grant root, or absolute
	cap: GrantCap,  // Read or ReadWrite
) -> Result<GrantScope, GrantError>
```

`authorize_in` does the same against one library's database.

On success, `GrantScope` carries the grant id, the library id, the canonical root, and the resolved `path` the surface must open. The surface never opens the path the agent sent. `GrantScope::path` is spelled under the grant root whatever spelling the request used.

`authorize` reads the grant row on every call and caches nothing. A revocation or expiry refuses the next request.

`GrantError::is_credential` separates refusals of the token (unknown, expired, revoked, source removed) from refusals of one request (read-only grant, path outside the grant, protected file). An HTTP surface answers the first group with 401 and the second with 403.

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

Each decision against a known grant is recorded in the `grant_audit` table, whether the request was allowed or refused. A record holds the grant id, the operation, the path as the agent sent it, the result, the reason for a refusal, and the time.

The table keeps the newest 10,000 rows and trims the oldest on every write. If the audit row for an allowed request cannot be written, the request is refused. Nothing is served without a record.

A token that matches no grant has no grant and no library to record against. That refusal is logged through `tracing`.

`grants.list` shows the last allowed use of each grant from the audit. `grants.audit` returns the log, newest first, for one grant or for all grants.

## Ops and CLI

| Op | Kind | Does |
| --- | --- | --- |
| `grants.create` | library action | Grant a directory. Returns the grant and its token, once. |
| `grants.list` | library query | Active grants, or all with `include_inactive`. Never returns tokens. |
| `grants.revoke` | library action | End a grant now. |
| `grants.extend` | library action | Set a new expiry from now. |
| `grants.audit` | library query | The audit log, newest first. |

```
sd grant create <path> [--read | --read-write] [--ttl 1h] [--label name]
sd grant list [--all]
sd grant revoke <id>
sd grant extend <id> [--ttl 1h]
sd grant audit [<id>] [--limit 50]
```

The CLI binary is `sd-cli`. `--ttl` takes values such as `30m`, `1h` or `2h30m`.

## Known limits

- **Time of check and time of use.** `authorize` resolves the path, then the surface opens it. A process that can create symlinks inside the directory could swap one in between. A read-only grant cannot create links, so this matters only for writes. The write path resolves the request again just before its rename and refuses a path that no longer resolves to the same place, which narrows the window to the rename itself. See [agent-writes.md](agent-writes.md).
- **Hard links.** A hard link to a protected file under another name is a different name for the same bytes. It is not detected, and it does not need to be: agent writes never modify a file in place. They stage new bytes and rename them over the name, so the other name keeps the old bytes. Git does not create hard links into a working tree, and no agent surface can create one.
- **No sync.** A grant exists on one node. Grants that follow a library between devices need the assertion-layer sync rules in `source-durability.md`.
- **One verifier.** Tokens are opaque and stored hashed. Signed tokens, which a node could verify without a lookup, can be added behind `token::hash` without changing `authorize`.
