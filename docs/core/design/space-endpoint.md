# Space Endpoint

> Status: Design position, 2026-11-27. The protocol a remote mount client and
> a harness speak to a node. Built switched off; see "Exposure".
> Companions: `docs/core/design/agent-access-threat-model.md` (the M1
> checklist this endpoint must pass), `docs/core/design/agent-grants.md` (the
> grant and `authorize`), `docs/core/design/agent-writes.md` (preconditions),
> `docs/core/design/mounts.md` (the block cache a mount reuses)

The space endpoint is the network face of a node. One HTTP listener inside the
daemon carries three things:

1. MCP at `/mcp`, the tool set agents already use over loopback.
2. A REST API at `/v1/spaces/<space>/...` that exposes the same tools to
   harnesses that do not speak MCP.
3. A block-read protocol and an event stream, which a thin mount client uses
   to serve a filesystem on another machine.

All three accept one credential, a grant token, and every request goes through
`ops::grants::authorize`. Nothing on the endpoint reaches the op registry or the
RPC socket.

This document is the contract for clients. A mount client built against it
needs nothing else from the daemon.

## Spaces and grants

A space is a directory with a policy. A grant lends a subtree of a space to one
holder. Until the `space` table exists, every directory grant is its own space:
the space id is the grant id, and the space root is the granted directory. When
spaces land, the space id becomes the space's own id and a grant names a
subtree inside it. The URLs below do not change.

A client learns its space ids from `GET /v1/spaces`. It never builds one from
anything else.

Every path in a request or a response is relative to the space root. Paths use
`/` separators, have no leading slash, and name the root as the empty string.
In a URL, each path segment is percent-encoded and the segments are joined with
`/`. The endpoint refuses absolute paths, empty segments, `.` and `..`
segments, NUL, and segments longer than 255 bytes, with 400, before it touches
the filesystem.

## Transport and authentication

- HTTP/1.1. A loopback listener speaks plain HTTP. A listener on any other
  address speaks TLS 1.2 or 1.3, unless the operator states that the network is
  already encrypted (see "Exposure").
- The token travels in `Authorization: Bearer sdg_...` and nowhere else. A
  request with a token in the query string answers 400.
- Unknown, expired and revoked tokens answer the same 401 with
  `WWW-Authenticate: Bearer realm="spacedrive"`. A client that gets 401 stops
  and asks its owner for a new token. It does not retry.
- A request that names a space the token does not grant answers 404, the same
  answer as a space that does not exist.
- The endpoint sends no CORS headers. A request carrying `Origin` answers 403,
  and `OPTIONS` answers 403.
- `Host` must name the bound address or a configured host name, with the bound
  port. Anything else answers 403.

### Errors

Every non-2xx answer outside `/mcp` has a JSON body:

```json
{ "error": { "code": "stale", "message": "the file changed since you read it" } }
```

| Status | `code` | Meaning |
| --- | --- | --- |
| 400 | `invalid` | Malformed path, range or body. |
| 401 | `unauthorized` | Token unknown, expired or revoked. |
| 403 | `forbidden` | The grant does not allow this: outside the root, read-only, a link out of the root, or a protected file. |
| 404 | `not_found` | No such space, file or directory. |
| 409 | `conflict` | A create would replace an existing file. |
| 412 | `stale` | A precondition failed. The body carries the current `etag`. |
| 413 | `too_large` | Body above its limit. |
| 416 | `range` | The range starts at or past the end of the file. |
| 429 | `limited` | A rate or byte budget is spent. `Retry-After` gives seconds. |
| 503 | `unavailable` | The space root is unreachable, or the request could not be audited. |

Messages never contain an absolute host path.

## REST

The REST API and MCP come from one tool registry. Each tool has one name, one
input schema, one output shape and one capability, and both surfaces are
generated from that entry. A tool added to the registry appears on both, so the
two cannot drift.

| Method and path | What it does |
| --- | --- |
| `GET /v1/spaces` | The spaces this token reaches: `[{ id, name, caps, expires_at }]`. |
| `GET /v1/spaces/<space>/tools` | The tool table, the same JSON as MCP `tools/list`. |
| `POST /v1/spaces/<space>/tools/<tool>` | Run one tool. The body is the tool's arguments, the same object MCP `tools/call` takes. |

A tool call answers 200 with the tool's structured output, the object MCP
returns as `structuredContent`. A tool that fails answers with the status from
the error table and the tool's message. `get_skill` answers `text/markdown`.

Limits are those of MCP: the same per-grant rate, calls in flight, call wall
clock, page sizes and byte budget, counted together across both surfaces.

## Block reads

A mount reads file bytes in fixed blocks and caches them by block.

- The block size is 1 MiB (1,048,576 bytes). Block `n` covers bytes
  `n * 1 MiB` to `(n + 1) * 1 MiB - 1`. The last block of a file is short.
- A client may read one block, several consecutive blocks, or any byte range.
  Aligned reads are the ones a cache can reuse.

### Stat

`GET /v1/spaces/<space>/stat/<path>`

```json
{
  "path": "docs/report.pdf",
  "kind": "file",
  "size": 3145728,
  "modified_at": "2026-11-27T09:12:44.120Z",
  "etag": "\"f-3145728-1701076364120000000-8812345\"",
  "block_size": 1048576
}
```

`kind` is `file`, `directory`, `symlink` or `other` (a FIFO, socket or
device). A symlink inside the root is reported, never followed. Stat on a
symlink whose target leaves the root answers 403; `list` still reports it as
`symlink`. `etag` is present for files. `GET /v1/spaces/<space>/stat/` stats
the root.

### List

`GET /v1/spaces/<space>/list/<path>?cursor=<c>&limit=<n>`

Answers `{ "entries": [...], "next_cursor": "..." }`. Each entry is shaped like
a stat answer without `block_size`. `next_cursor` is present when there are
more. `limit` defaults to 200 and is at most 1,000. Entries are sorted by name
in byte order, and a cursor is the last name of the previous page, so a cursor
stays valid while the directory changes.

### Read

`GET /v1/spaces/<space>/blocks/<path>` with a `Range` header.

```http
GET /v1/spaces/0193.../blocks/docs/report.pdf HTTP/1.1
Authorization: Bearer sdg_...
Range: bytes=1048576-3145727
If-Match: "f-3145728-1701076364120000000-8812345"
```

- `Range` is required and holds one `bytes=a-b` or `bytes=a-` range. A
  missing range or more than one range answers 400.
- One request returns at most 8 MiB. A longer range is answered with its
  first 8 MiB, and `Content-Range` says which bytes came back; the client asks
  again for the rest.
- The answer is `206 Partial Content` with `Content-Range`, `Content-Length`,
  `ETag` and `Content-Type: application/octet-stream`.
- A range that starts at or past the end of the file answers 416 with
  `Content-Range: bytes */<size>`.
- `If-Match` is optional. When it is present and the file's current `etag`
  differs, the answer is 412 and no bytes are sent. A mount sends the `etag`
  it got when it opened the file, so it never stitches blocks from two
  versions of a file into one read.
- The read goes through a handle opened beneath the space root. A symlink at
  the path, or on the way to it, that leaves the root answers 403.

### The etag

The etag is a cheap version stamp, not a content hash. It is derived from the
file's size, modification time in nanoseconds and file identity (inode or file
index). A write through Spacedrive always changes it. A write outside
Spacedrive that keeps both size and modification time would not; a client that
needs certainty compares `content_hash` from the `stat` tool, which hashes the
bytes.

The etag is opaque. Clients compare it for equality and never parse it.

## Event stream

`GET /v1/spaces/<space>/events` with `Accept: text/event-stream`.

The stream tells a client which cached blocks and directory listings to drop.
It is a server-sent event stream, and each event's data is one JSON object:

```text
id: 7f3a9c-1842
event: change
data: {"seq":1842,"op":"modify","path":"docs/report.pdf"}

id: 7f3a9c-1843
event: change
data: {"seq":1843,"op":"move","path":"a.txt","to_path":"b/a.txt"}
```

| `op` | Drop from the cache |
| --- | --- |
| `create` | The parent's listing. |
| `modify` | The file's blocks and stat, and the parent's listing. |
| `move` | Both paths, everything beneath them, and both parents' listings. |
| `delete` | The path, everything beneath it, and the parent's listing. |

Two more events carry no path:

- `reset`: the server cannot say what changed since the client's position.
  The client drops every cached listing and stat for the space and revalidates
  each open file by etag before it reuses that file's cached blocks.
- `end`: the grant was revoked or expired. The stream closes and the client
  stops serving the mount.

Resuming:

- An event id is `<log>-<seq>`. `<log>` names the log the seq counts in and
  changes whenever seqs could repeat, such as a daemon restart for the
  in-memory feed. Clients treat the whole id as opaque.
- A client reconnects with `Last-Event-ID` set to the last id it saw. The
  server replays every change after it when `<log>` matches and it still
  holds them. Otherwise it sends `reset` first, so a seq from an earlier log
  never suppresses a reset.
  A first connection with no `Last-Event-ID` starts at the current position
  with no replay.
- The server closes a stream that has carried no event for 60 seconds, and the
  client reconnects at once with `Last-Event-ID`. A comment line (`: keep`)
  every 15 seconds keeps intermediaries from closing it sooner.
- A grant may hold 4 streams at once, the same as its MCP session limit.

Where the changes come from: when a space has an event log, the stream is that
log read from `seq`, so it resumes across daemon restarts. Before that, the
stream carries the changes the source's watcher observes under the space root.
Those are held in memory only. A daemon start picks a new random `<log>`, so
a client that reconnects across a restart gets `reset`. Nothing is
written to disk for the stream.

Events are hints. A client that misses one sees a stale listing or a 412 on its
next read, never wrong bytes.

## Writes

Writes are not part of the block protocol yet. A mount that writes uses the
write tools through REST (`write_file`, `make_dir`, `move`, `delete`), with
the `content_hash` it read as the precondition. Writable grants over a
non-loopback address stay off until the outside security review in the threat
model passes.

## Exposure

The endpoint is off by default. The daemon config holds it:

| Key | Default | Meaning |
| --- | --- | --- |
| `agent_mcp.enabled` | `false` | Start the listener. |
| `agent_mcp.listen` | `127.0.0.1:7765` | One explicit address. Wildcards (`0.0.0.0`, `::`) are refused. |
| `agent_mcp.tls_cert`, `agent_mcp.tls_key` | none | PEM files. Required for a non-loopback address. |
| `agent_mcp.host_names` | none | Extra names accepted in `Host`, for a reverse proxy. |
| `agent_mcp.block_bytes_per_hour` | `8589934592` (8 GiB) | Block-read budget per grant per hour. Must be greater than 0. |

A non-loopback address without TLS files does not start, and the daemon logs
why. Nothing is granted by address: a loopback request still needs a token.

Writes need a direct local request: a loopback bind, a loopback Host with the
bound port, and no `Forwarded`, `X-Forwarded-For`, `X-Forwarded-Host` or
`X-Real-IP` header, the same test OAuth uses. Every other request that needs a
writable grant, the write tools over MCP or REST, answers 403 with "writes over
the network are off until the security review" before it runs. The grant check
still records the attempt. A non-loopback listener is therefore read-only, and
so is a loopback listener reached through a reverse proxy or a configured host
name.

## Limits

The limits of `agent-access-threat-model.md` apply to every route here,
counted per grant across MCP, REST, block reads and streams.

A grant has two byte budgets. Block reads spend the block-read budget, 8 GiB
per hour unless `agent_mcp.block_bytes_per_hour` sets another, and nothing else, so a mount can page files in without starving the
grant's tools. Everything else, the tools over MCP or REST, `stat`, `list`
and the event stream, spends the 256 MiB per hour budget. A spent budget
answers 429 with `Retry-After` set to the seconds until its hour resets.
