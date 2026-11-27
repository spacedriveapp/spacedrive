# Agent Access Threat Model

> Status: Design position, 2026-11-19. Required before the agent MCP endpoint listens on any address other than loopback.
> Companions: `docs/core/design/shares-and-remote-libraries.md` (the grant record), `docs/core/design/mounts.md` (the loopback share), `docs/cli/mcp.mdx` (`sd mcp` over stdio)

An agent grant lets a program you do not run reach one folder on one node, read
only, for a limited time. This document names who can reach each daemon surface
today, what each surface trusts, and what the new HTTP MCP endpoint has to do
before it may bind beyond loopback. It ends with the checklist that endpoint
must pass.

Scope is the read-only grant: list, stat, read, glob, grep, search and content
search inside a grant root. Writes are out of scope until the write path spec exists, and that
spec must revisit this document.

## Actors

| Actor | What it can do | Trust |
| --- | --- | --- |
| Local user | Owns the OS account the daemon runs as. | Full. |
| Local process as that user | Anything the user can do, including connect to every loopback port and read the data directory. A coding agent with a shell is one of these. | Full. A grant does not constrain it. |
| Other OS account or container on the host | Connect to loopback ports. On a NAS or shared Linux host this is a real actor. | Should be none. Today it gets full access (F1). |
| Web page in the user's browser | Send HTTP to any loopback port, rebind its own DNS name to 127.0.0.1. Cannot read responses without CORS or a rebind. | None. |
| Paired device | Run any registered op on this daemon over remote ops, and read every attached source over byterange. Authenticated by its Iroh node key. | High. Pairing is the trust boundary. |
| Agent holding a grant | Call the curated MCP tools with a bearer token. Assume its prompt can be steered by the files it reads and that its token can leak. | Only the grant: one subtree, read, until expiry or revocation. |
| Network attacker | Reach any non-loopback listener, observe plaintext, replay what it observes, flood. | None. |

The line that matters: a grant constrains an agent that holds only the token.
It does not constrain an agent that runs on the same account with a shell,
because that agent can call RPC on 6969 or run `sd mcp` and get the whole
registry. Docs and the app must not present grants as a sandbox for local
agents.

## What each surface trusts today

| Surface | Bind | Authentication | Reach |
| --- | --- | --- | --- |
| RPC (`core/src/infra/daemon/rpc.rs`) | TCP `127.0.0.1:6969`; named instances on 6970 to 7969 | None | Every op in the registry, `Shutdown`, and remote ops on every paired device through the `device` field |
| `sd mcp` (`apps/cli/src/domains/mcp.rs`) | stdio, forwards to RPC | None of its own | The whole registry as tools, destructive actions included |
| WebDAV share (`core/src/service/mounts/webdav.rs`) | `127.0.0.1:7764` or ephemeral | None; Host must be a loopback authority, CORS only for Tauri and loopback origins | Read access to every attached source, local and replicated |
| SMB share (`core/src/service/mounts/smb.rs`) | `127.0.0.1`, ephemeral port | One user, random password per start, or `SD_MOUNT_SMB_PASSWORD` | Same as WebDAV |
| byterange (`core/src/service/network/protocol/byterange.rs`) | Iroh QUIC | Paired node key | Stat, ranged reads (8 MiB per request), snapshots and database exports of attached sources |
| remote ops (`core/src/service/network/protocol/remote_ops.rs`) | Iroh QUIC | Paired node key | Every op in the registry and the log stream |
| `sd-server` (`apps/server/src/main.rs`) | `[::]:8080` by default | HTTP Basic from `SD_AUTH`; release builds refuse to start without it unless set to `disabled` | `/rpc` proxies the whole registry, plus sidecars, thumbnails, extension bundles and the share |

### Findings

- F1. RPC trusts any process that can open a TCP connection to loopback, on any
  OS account. There is no Unix socket and no per-connection secret, although
  `AGENTS.md` describes the transport as Unix domain sockets. Because the
  request envelope carries `device`, a local process on one node can also run
  ops on every paired node.
- F2. When a request line fails to parse, `rpc.rs` logs the raw line at error
  level. Anything secret sent on a malformed line ends up in `daemon.log`.
- F3. byterange's `authorize_path` rejects `..` and checks `starts_with` on
  the raw path, then calls `File::open`, which follows symlinks. A symlink
  inside a source that points outside it is read through. Today only paired
  devices can send these requests, so this is a containment gap, not an open
  hole. A grant check built the same way would be an open hole. Fixed: every
  peer path now opens through `common::contain::open_beneath`, which resolves
  it beneath the source root's handle (`openat2` with `RESOLVE_BENEATH` on
  Linux) and refuses a link that leaves the root, and reads go through that
  handle. Thumbnail asks are checked the same way, but the bake reopens by path.
- F4. The SMB mount URL, password included, is logged at info on every start.
  The password is also accepted from an environment variable, which other
  processes of the same user can read.
- F5. `sd-server` listens on all interfaces by default over plain HTTP. Basic
  credentials cross the network on every request, and failed logins are not
  rate limited. It is the one surface already exposed to a network, and it
  exposes more than any grant would.
- F6. The RPC line protocol happens to refuse browsers: an HTTP request's first
  line is not JSON, so the connection closes before dispatch. Nothing tests
  this, and nothing keeps a future change from adding an HTTP mode.

The WebDAV `loopback_only` middleware is the pattern to reuse: it refuses a
non-loopback Host, which stops DNS rebinding, and answers CORS only for local
origins.

## Token theft, replay and leakage

A grant token is a bearer secret. Whoever holds it can use it until it expires
or is revoked, from anywhere that can reach the endpoint. The controls below
keep it short-lived, out of places it can leak, and useless to a passive
observer.

Format and storage:

- 32 bytes from the OS CSPRNG, base64url, with a fixed `sdg_` prefix so secret
  scanners and log filters can match it.
- The record stores SHA-256 of the token, never the token. A 256-bit random
  secret needs no slow hash. Lookup is by hash, so there is no string
  comparison to time.
- The token is shown once, in the output of `grants.create`. Every other op
  (list, extend, revoke, audit) names the grant by id. No op accepts a token as
  input except the endpoint's own verifier.

Where it must never appear:

- URLs. The endpoint reads `Authorization: Bearer` only and refuses a token in
  the query string with 400, because URLs end up in proxy logs, shell history
  and browser history.
- Process arguments. `sd grant create` prints to stdout. No Spacedrive command
  takes a token as an argument, since arguments are visible to other users in
  `ps` on most systems.
- Logs. The HTTP layer must not log request headers or query strings. F2 must be
  fixed before grants ship, by logging the parse error and the line length, not
  the line. A test asserts that `daemon.log` contains no `sdg_` string after a
  create, a call and a failed call.
- MCP client configs, where possible. `--print-mcp-config` writes a config that
  reads the token from an environment variable by default, for clients that
  support that, and writes the literal token only with `--inline-token`. The
  connect docs (M2) warn that a project-level MCP config with a literal token
  is one commit away from a public repository.

Replay: a bearer token can be replayed by anyone who observes it. Over loopback
only local processes can observe it, and they already have more access than
the grant. Off loopback, the channel must be encrypted (see TLS below), so
observation needs a compromised endpoint. Lifetime is 5 minutes to 12 hours,
default 1 hour, and an extension sets a new expiry no more than 12 hours ahead.
A grant may also carry an optional allowlist of client address ranges; the
endpoint checks the TCP peer address against it. Sender-constrained tokens are
out of scope for v1; the verifier interface leaves room for them.

Revocation and expiry take effect on the next request. A tool call already
running when its grant is revoked or expires is cancelled within one second,
and an open SSE stream for that grant is closed.

## Path escape

The grant root is a source id plus a subtree inside it. Every path an agent
sends is relative to that root, and every path the endpoint returns is too.

Input rules, applied before touching the filesystem:

- Reject absolute paths, empty components, `.` and `..` components, NUL, and
  any component longer than 255 bytes.
- On Windows also reject `:` (alternate data streams), `\`, components that
  end in a dot or space, and reserved device names (`CON`, `NUL`, `COM1`).

Containment, applied after resolution:

- Never decide containment by comparing strings the agent sent. Open the root
  once per request and resolve the target beneath it, then check the real path
  of the opened handle: `openat2` with `RESOLVE_BENEATH` on Linux, `F_GETPATH`
  on macOS, `GetFinalPathNameByHandleW` on Windows. Read only through that
  handle. This closes the window where a symlink is swapped
  between the check and the open.
- Compare the handle's real path with the root's real path, both taken from the
  OS the same way. That makes case-insensitive volumes, Unicode normalization
  and macOS firmlinks (`/Users` against `/System/Volumes/Data/Users`) compare
  correctly, because both sides come back in the same spelling.
- A symlink, junction or reparse point whose target is inside the root works. One
  whose target is outside is listed as a link, refused on read, and never
  followed by stat, glob or grep.
- Walkers for list, glob and grep do not follow links and run every yielded path
  through the same check.
- Search answers from the index. Results are filtered on the record's current
  path under the grant root at query time, and a record whose path left the root
  is not returned.

Accepted risks: a hard link inside the root to a file outside it is readable,
since creating one needs write access inside the root; a volume mounted inside
the root is readable, since it is under the folder the user chose.

byterange and the grant verifier share one containment function,
`core/src/common/contain.rs`.

## Untrusted content

Every file in the grant is input to the agent, and some files will try to steer
it. Spacedrive cannot stop that. It bounds the result: a read-only grant can
disclose the subtree and nothing else, and no tool output carries instructions
from Spacedrive except `get_skill`. The skill text is compiled into the
binary, never read from the grant subtree. Tool descriptions are static.

## Resource exhaustion

An agent loops, retries and fans out. A grant must not be able to starve the
node, other grants or the user's own work. These are the defaults M1 adopts.
A grant may set lower limits at creation; nothing may set higher ones without a
code change.

| Limit | Default |
| --- | --- |
| Live grants per node | 64 |
| MCP sessions per grant | 4 |
| Tool calls in flight per grant | 4 |
| Tool calls in flight, whole endpoint | 32 |
| Request rate per grant | 10 per second, burst 30 |
| Request body | 64 KiB |
| Request headers | 16 KiB |
| Tool call wall clock | 30 s, then cancelled |
| `read` per call | 64 KiB or 2,000 lines by default, 1 MiB maximum |
| Bytes returned per grant | 256 MiB per hour |
| `list_dir` and `glob` page | 1,000 entries |
| `glob` entries visited per call | 100,000 |
| `grep` pattern length | 1 KiB |
| `grep` compiled regex size | 1 MiB |
| `grep` text scanned per file | 1 MiB, as in X1 |
| `grep` bytes scanned per call | 1 GiB |
| `grep` matches per page | 1,000 |
| `grep` calls in flight per grant | 1 |
| `search` results per page | 100 |
| `search_content` query length | 1 KiB |
| `search_content` results per page | 200 |
| Failed authentications per client address | 20 per minute, then 429 for 5 minutes |
| Idle SSE stream | closed after 60 s |

Exceeding a rate or byte budget answers 429 with `Retry-After`. Exceeding a
size or page limit answers a truncated result that says it was truncated and how
to continue. The regex engine is the `regex` crate, which matches in linear
time, so the size limit is the only regex control needed.

## Audit

Every request that presents a grant token is recorded in `library.db`, beside
the grant records, with: time, grant id, grant label, client address, MCP
session id, tool, grant-relative path, grep or glob pattern (first 256 bytes),
bytes returned, result (ok, denied, limited, error) and duration.

- Never recorded: the token, file contents, match text.
- Requests with an unknown token are recorded per client address as one row
  per minute with a count, so a flood cannot fill the table.
- Rate-limited requests are aggregated the same way per grant.
- The write happens before the response is sent. If it fails, the request
  answers 503. An unaudited read is not served.
- Retention is 30 days or 1 million rows, whichever comes first.
- `sd grant list` shows last use and request count; `sd grant log <id>` shows
  the rows.

## The HTTP MCP endpoint

The endpoint is a separate listener inside the daemon. It never proxies to
RPC on 6969 and has no route into the op registry, so a grant cannot reach
remote ops, the `device` field or any action.

Bind:

- Off by default. When enabled with no address it binds `127.0.0.1` only.
- `--listen <addr>` binds one explicit address. Wildcards (`0.0.0.0`, `::`)
  are refused.
- Nothing is granted by address. A request from loopback still needs a token.
  This matters when a reverse proxy on the same host forwards remote traffic to
  loopback.

TLS:

- Loopback: plain HTTP.
- Any other address: the daemon serves TLS from `--tls-cert` and `--tls-key`,
  or the user passes `--plaintext` to state that the network itself is
  encrypted, such as a WireGuard or Tailscale address. The daemon logs a warning
  at every start with `--plaintext`.
- The documented remote setup is the endpoint on loopback behind a TLS reverse
  proxy the user already runs, such as `tailscale serve`.

Host, Origin and CORS:

- On loopback, the Host header must be `127.0.0.1`, `localhost` or `[::1]`
  with the bound port; otherwise 403. This stops DNS rebinding.
- On another address, Host must be that address or a name passed with
  `--host-name`, for reverse proxies.
- A request with an Origin header is refused with 403 unless that origin is in
  an allowlist, which is empty by default. MCP clients outside a browser send
  no Origin.
- The endpoint sends no CORS headers and answers OPTIONS with 403.
- No cookies and no Basic auth, so a browser never attaches a credential on its
  own.

Sessions and errors:

- An MCP session is bound to the grant that opened it. A session id presented
  with any other token answers 404. A session id alone authorizes nothing.
- Unknown, expired and revoked tokens get the same 401 body and a
  `WWW-Authenticate: Bearer` header.
- Error messages never include absolute host paths.

## OAuth on the HTTP endpoint

The endpoint also runs the MCP authorization flow (`docs/cli/mcp-oauth.mdx`)
so a client can connect with only the URL. The daemon is its own authorization
server. What it adds to the attack surface, and how each part is held:

- The access token is the grant token. OAuth adds no second credential that
  reaches `/mcp`, so every rule above applies to it unchanged.
- A web page cannot drive the flow. Metadata, registration, token and
  revocation requests that carry any Origin answer 403, and no CORS headers are
  sent. The consent form accepts a post only with the daemon's own Origin and
  a 256-bit request id that only the page itself holds. The page sends
  `frame-ancestors 'none'` and `X-Frame-Options: DENY`.
- The consent page is the only place a grant is minted, and only by a person
  pressing Allow. A malicious client can register and open the page, but the
  page shows its self-chosen name as unverified, shows where the browser will
  return, and needs the user to pick the folder.
- Code interception is covered by required PKCE S256, single-use codes that
  live 60 seconds, exact redirect matching (any port for a loopback redirect,
  RFC 8252), and revocation of the grant on a reused code or a wrong verifier.
- Refresh tokens are bound to their client, stored as SHA-256 and rotated on
  every use. Every spent hash is kept while its grant lives, so a replay of any
  rotated refresh token revokes the grant.
- The consent page sends `Referrer-Policy: same-origin`. With `no-referrer` a
  browser would send `Origin: null` on the form post, which the form refuses.
- Refresh never extends a grant past the lifetime the owner picked.
- Registration is unauthenticated, as RFC 7591 allows, and capped at 100
  clients with the oldest dropped first. A local process could churn it; local
  processes can already reach RPC (F1), so this adds nothing they lack.
- Logs carry client ids and grant ids, never tokens, codes or verifiers.
- Issuer and resource URLs are built from the Host header, which is already
  limited to loopback with the bound port. A non-loopback listener must serve
  the metadata over HTTPS with the configured host name before OAuth is offered
  there.

## Should RPC on 6969 stay loopback-trusted?

Yes for this release, with the trust stated correctly. It is not a blocker for
M1, because M1 does not expose RPC and grants cannot reach it. What 6969 trusts
today is every process on every account that can reach loopback, not the local
user (F1). On a single-user desktop those are close. On a NAS, a shared Linux
host or a host-networked container they are not.

Follow-up work, in order:

1. Fix F2 now, since it is small and grants make it matter.
2. Add a test that an HTTP request to 6969 closes the connection without
   dispatch, so F6 stays true.
3. Require a per-daemon secret on RPC: a random cookie written to a `0600` file
   in the data directory at start and sent as the first line of every
   connection. It keeps TCP, works on every platform and limits RPC to the
   daemon's own OS account. This should land before Spacedrive documents
   running a daemon on a multi-user host.

## What blocks network exposure

Off by default stays the rule. Two levels of exposure need different bars.

Opt-in listening beyond loopback (`--listen`) is blocked until every M1
checklist item below passes, F2 is fixed, and this document is merged.

Exposure by default (an endpoint that listens beyond loopback without the user
choosing an address, a toggle that is on by default in the app, or relay
serving that makes a node reachable from the internet) is blocked until:

- an outside security review covers the grant verifier, the path containment
  function on all three platforms, the HTTP endpoint and the limits;
- F1 has its per-daemon secret, so a shared host is not one local hop from
  every paired device;
- F5 is resolved: `sd-server` binds loopback by default and rate limits failed
  logins.

The outside review is not required for opt-in listening on loopback or a
user-chosen overlay address with a read-only grant, because the reachable set is
the user's own devices. It is required before the first of default exposure,
relay serving or writable grants.

## M1 checklist

Each item needs a test or a recorded manual check in the M1 PR.

1. The endpoint is off by default, binds `127.0.0.1` when enabled without an
   address, and refuses wildcard binds.
2. A non-loopback bind starts only with `--tls-cert` and `--tls-key` or with
   `--plaintext`, and `--plaintext` logs a warning at start.
3. The endpoint has no route into the op registry and does not connect to RPC.
4. Tokens are 32 random bytes with the `sdg_` prefix, stored as SHA-256,
   shown once, and accepted only from `Authorization: Bearer`. A token in the
   query string answers 400.
5. Unknown, expired and revoked tokens answer the same 401. A revoked or
   expired grant fails its next call, an in-flight call is cancelled within one
   second, and its SSE stream closes.
6. `daemon.log` contains no `sdg_` string after create, use, a failed call and
   a malformed RPC line (F2 fixed).
7. Host validation answers 403 to a rebinding Host on loopback; any Origin not
   in the allowlist answers 403; no CORS headers are sent; OPTIONS answers 403.
8. MCP sessions are bound to their grant; a session id with another token
   answers 404.
9. Path containment uses the handle-based check, with tests for `..`, absolute
   paths, a symlink and a directory symlink out of the root, a symlink swapped
   during a read, case-only aliases on a case-insensitive volume, a firmlink
   spelling on macOS, and Windows junctions, alternate data streams and device
   names.
10. list, glob and grep do not follow links and filter every yielded path; search
    and search_content filter by current path under the root.
11. Every limit in the table is enforced with the default value, answers 429 or
    a marked truncation, and has a test.
12. Every token-bearing request writes an audit row before responding; an audit
    write failure answers 503; tokens, contents and match text never appear in
    audit rows.
13. `get_skill` returns text compiled into the binary.
14. `--print-mcp-config` references an environment variable by default and
    writes a literal token only with `--inline-token`.
15. The connect docs say that a grant does not constrain an agent running on the
    same account with a shell.
