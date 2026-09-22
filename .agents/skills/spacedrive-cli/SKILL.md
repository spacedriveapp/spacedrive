---
name: spacedrive-cli
description: >
  Operate a Spacedrive daemon with sd-cli: start and stop it, select a library,
  track sources, generate thumbnails, tag files, follow jobs and logs, and call
  any registered op. Use when running, scripting or debugging Spacedrive from a
  terminal.
---

# Spacedrive CLI

`sd-cli` is a client of `sd-daemon`. Apart from `start`, `stop`, `restart`,
`config`, `daemon` and `update`, every command needs a running daemon and fails
with "Spacedrive daemon is not running" without one. Nothing starts it for you.

Run `sd-cli --help` and `sd-cli <command> <subcommand> --help` for flags and
arguments. This skill covers what help can't tell you.

## Binaries

From the repo root, `just cli <args>` runs `cargo run --bin sd-cli -- <args>`.
To call the binary directly, build both:

```bash
cargo build --release --bin sd-cli --bin sd-daemon
```

`start` and `restart` launch the `sd-daemon` next to `sd-cli`, so keep them
from the same build. A desktop build (`just dev-desktop`) copies its release
sidecar over `target/debug/sd-daemon`; run `cargo build --bin sd-daemon` before
`just cli restart` after one. After changing core, rebuild the daemon and
restart it, since the CLI talks to whatever is running.

## Addressing a daemon

`--data-dir`, `--instance` and `--format` go before the subcommand
(`sd-cli --format json sources list`) and are rejected after it. `--device`
works anywhere.

The port comes from the instance alone. The default instance listens on
127.0.0.1:6969, which is also where the desktop app connects. A named instance
gets 6970 plus the byte sum of its name mod 1000, with its data in
`<data-dir>/instances/<name>`.

So `--data-dir` does not pick a daemon. If one is listening, `start` prints
"Daemon is already running" and every command reaches it, whatever data dir it
was started with. `sd-cli status` shows the running daemon's data directory.
Give a second daemon its own `--instance`.

The client reads some state from `--data-dir` itself: `cli.json` (the selected
library), `device.json` and, for `logs show`, `logs/`. Pass the same flags to
every command for a daemon. An alias keeps them together:

```bash
alias sd='/path/to/spacedrive/target/release/sd-cli --data-dir "/Volumes/Archive/.spacedrive"'
```

Inside double quotes a backslash before a space is kept, so
`"/Volumes/Seagate\ 20TB/.spacedrive"` names a directory that doesn't exist and
the daemon can't create its logs directory. Quote the path and drop the
backslash.

## Daemon lifecycle

- `start` spawns the daemon in the background and pings it half a second
  later. "may not be fully initialized yet" means it is still opening
  libraries; check with `status`. `--foreground` keeps it attached and prints
  its logs.
- `stop` requests shutdown and returns. Running jobs pause first, so the
  process can outlive the command.
- `stop --reset` and `restart --reset` delete `libraries/`, `device.json`,
  `spacedrive.json`, `logs/` and `job_logs/` from the data dir after a y/N
  prompt. Only run them when asked.
- `daemon install` starts the daemon at login. `config` edits the CLI's own
  update settings (`update.repo`, `update.channel`) and nothing in the daemon.

## Startup discovery

A daemon indexes nothing at launch until a client asks. The desktop app calls
`indexing.startup` once its window shows, and `sd-cli index start --defaults`
does the same from a terminal. The web UI and Photos never ask. The pass:

1. tracks the system volume,
2. restores every volume and source snapshot, which arms its watcher,
3. adds the home folder as a source,
4. maps attached drives, healing sources with gaps and walking whole drives,
5. hashes every source.

`--no-default-sources` on `start`, `restart` or `sd-daemon` drops steps 1 and 3
and the whole-drive walks. Use it for a library that should hold only what you
add, since the desktop app runs the pass on any daemon it connects to. The pass
runs once per library per daemon process; a repeat reports "already running"
even after it finished.

On a headless daemon, run `index start --defaults` after each start so tracked
sources come back watched and hashing resumes.

## Libraries

- The selection lives in `<data-dir>/cli.json`. When nothing is selected, or
  the selected library is gone, any command selects the first library the
  daemon lists, so a single library needs no setup.
- `library create <name>` switches to the new library.
- `index start` ignores the selection and needs `--library <id>` when several
  libraries exist. `index quick-scan` and `index browse` have no `--library`
  flag and refuse to run with more than one library.
- `op` never uses the selection. Library ops fail with "Library ID required"
  unless you pass `--library <id>`.

## Sources

- `sources track <path>` canonicalizes the path on the machine running the
  CLI, registers the source and queues a walk and a low-priority hashing job.
  The hashing job can start before the walk writes anything and finish having
  hashed nothing; `index start --defaults` after the walk hashes what it found.
  Tracking a mount point covers the whole drive.
- `--unfiltered` also records system files, `.git` and dev directories, which
  archival drives want. `sources update <id> --unfiltered true` widens an
  existing source and walks for what was skipped; `false` narrows future
  captures and removes nothing.
- Source ids for `update`, `verify` and `freeze` come from `sources list`.
- `index start <path>` walks a path into the volume index without registering
  a source. Its `--persistent` and `--include-hidden` flags are parsed and
  ignored. Use `sources track` for anything that should last.

## Thumbnails

`thumbs generate <path> --recursive` covers indexed files only, so run it after
the walk finishes. `--mode missing` fills empty slots and keeps stale tiles,
`stale` (the default) also replaces outdated ones, and `force` bakes everything
again. Relative paths resolve against your shell's directory and `sd://` URIs
pass through; with `--device`, use absolute paths. It prints a JSON job receipt
whatever `--format` says.

## Tags

- `tag create "Trips/Iceland"` creates by path. An existing path prints
  "Already exists:" with its id, so it's safe to repeat.
- `tag search` with no query lists every tag as `<id> <path>`.
- `tag apply` and `tag unapply` take file ids as positionals and one tag id per
  `--tags`. Help says "space-separated", but `--tags A B` reads `B` as a file;
  repeat the flag for several tags.
- File ids come from `file info <path>` and `file list <dir>`. Pass absolute
  paths, since both send the path as typed.
- A file outside every tracked source is skipped with
  `warning: no tracked source holds file <id>`, and the "Tagged N target(s)"
  count leaves it out.
- `--content` tags the bytes so the tag reaches every copy. It takes content
  ids (`content_identity.uuid`), which exist once hashing has reached the file.
- One call takes at most 1000 targets.
- `tag delete <id>` removes the tag and every application of it in every
  source this daemon can write.

## Compare folders

`file compare <a> <b>` compares two indexed folders from their source stores,
without reading either drive. The output calls the first folder A and the
second B, and names both at the top. Both paths are canonicalized locally and
must sit inside tracked sources on mounted drives.

- `--by path`, the default, pairs files by their path relative to each folder.
  `--show` picks `only-a` (the default), `only-b`, `both` or `different`. A
  pair is in both when its content ids match or, where either side is
  unhashed, when size and modification time match; otherwise it is different.
- `--by content` asks whether a file's bytes exist anywhere under the other
  folder, whatever the file is called: `--show only-a` is what a backup in B
  is missing. `both` counts A's files, so a B file whose bytes A holds is in
  no count. `different` is refused. Files with no content id are counted as
  "Not hashed yet" and listed in no set, so hash both sides first.
- It prints every set's count, then the set's files, one per line, as each
  page of 5000 arrives. `--limit` stops after that many files, printing
  `N of M listed` when that cuts the set short. Piping into `head` ends it
  cleanly.
- `--format json` prints one document with `totals` and every entry. Under
  `--limit`, fewer entries than the set's count in `totals` means it stopped
  early.
- `--include-hidden` brings in hidden files.

## Jobs and logs

- `job list --status` takes `queued`, `running`, `paused`, `completed`,
  `failed` or `cancelled`. A misspelled value lists every job.
- `job monitor` is a TUI. Without a terminal use `--simple`, or poll
  `--format json job list`.
- Walks don't resume after the daemon exits; they start over.
- `logs show` reads `<data-dir>/logs` directly, so it needs the daemon's
  `--data-dir`. `logs follow` streams from the daemon.

## Output

`--format json` prints each command's output type as pretty JSON, with these
exceptions:

| Command | Output |
|---------|--------|
| `op`, `thumbs generate` | JSON always |
| `file info` | JSON in both formats |
| `job list` | one `{"jobs": [...]}` document per library, each holding the selected library's jobs |
| `events monitor` | its own `-f` (human, json, json-pretty) |
| `sync metrics`, `sync events` | their own `--json` and `-f` (json, sql, markdown) |
| `job monitor`, `logs follow` | streams |

Errors go to stderr as `Error: <message>` with exit code 1. Failures inside the
daemon read `Core operation failed: <detail>`.

## Calling any op

`sd-cli op <name> --json '<input>' [--library <id>]` tries `query:<name>`, then
`action:<name>.input`, and prints the result as JSON. The input is the op's
input type as serde shapes it; `packages/ts-client/src/generated/types.ts`
spells each one out. List every op name with:

```bash
rg -U -o --no-filename -r '$2' \
  'register_(library_query|core_query|library_action|core_action)!\(\s*[A-Za-z_:]+,\s*"([^"]+)"' \
  core/src | sort -u
```

```bash
sd op tags.search --library <LIBRARY_ID> --json '{"query":""}'
sd op indexing.startup --library <LIBRARY_ID> --json '{"force":true}'
```

## Other devices

`--device <name|slug|id>` runs the command on a paired device against that
device's open library and leaves the local selection alone. Give paths as they
exist on the target. `sources track` still canonicalizes locally, so its path
has to exist on this machine as well.

## Workflows

### Catalog a drive into a library stored on it

Check `sd-cli status` first. If a daemon already holds 6969, such as one the
desktop app started, stop it or the drive's daemon never starts.

```bash
cargo build --release --bin sd-cli --bin sd-daemon
alias sd='/path/to/spacedrive/target/release/sd-cli --data-dir "/Volumes/Archive/.spacedrive"'

sd start --no-default-sources
sd status                                  # data directory should be on the drive
sd library create Archive                  # when `sd library list` is empty
sd sources track /Volumes/Archive/Photos --unfiltered
sd --format json job list | jq -c '.jobs[] | {name, status, progress}'
sd thumbs generate /Volumes/Archive/Photos --recursive --mode missing
```

After each restart:

```bash
sd start --no-default-sources
sd index start --defaults                  # restore, watch, heal and hash what's tracked
```

### Tag a folder

```bash
TAG=$(sd --format json tag create "Trips/Iceland" | jq -r .tag.id)
sd --format json file list /Volumes/Archive/Photos/Iceland --limit 1000 \
  | jq -r '.files[] | select(.kind == "File") | .id' \
  | xargs /path/to/spacedrive/target/release/sd-cli \
      --data-dir "/Volumes/Archive/.spacedrive" tag apply --tags "$TAG"
```

xargs can't run an alias, so it names the binary. `--limit 1000` matches the
per-call target cap.
