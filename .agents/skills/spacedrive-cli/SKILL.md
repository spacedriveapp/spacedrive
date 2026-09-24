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

## Copy and move files

`file copy <sources>... --destination <path>` copies files or folders, or
moves them with `--move-files`. Like merge, it validates and previews first:
the findings, the facts, and the plan's counts print before a y/N prompt;
`--dry-run` stops after the plan and `-y` skips the prompt. Each source lands
at its own name in the destination, or at the destination itself for one
source given a new name.

- A folder landing beside a folder of its name is planned like a merge into
  it, file by file, so the plan shows what would be replaced instead of a
  silent overwrite. `--overwrite` replaces what differs; without it a
  colliding file is skipped.
- An error finding refuses: a source that is not there, a folder copied into
  itself (`copy.cycle`), or a destination folder that does not exist when
  several sources need one. Free space on the destination volume is a
  warning with numbers.
- A move on one volume is a rename that keeps the records' identity, which
  validation says (`move.atomic`). Across volumes the plan consumes the
  source, and tag assertions on its records stay behind; validation counts
  them (`move.identity_loss`).

## Delete files

`file delete <paths>...` moves files to the trash, or removes them with
`--permanent`, after a y/N prompt that `--yes` skips. Paths are canonicalized
locally. It validates and previews first: the warning only an index can
give is `delete.last_copy`, how many of the files are the last copy of
their content anywhere in the library, and the plan flags each such row.
`--dry-run` stops after the plan.

`file delete <A> --against <B> --show <set>` deletes from A the files in one
set of `file compare A B`, with the same `--by` and `--include-hidden`. It
prints the comparison's counts and asks before dispatching. `--show` is
required: `both` removes what B already holds, `only-a` what B lacks, and
`different` A's version where B's differs. To delete from B, swap the folders;
`only-b` is refused.

- The client sends the comparison, never a list. The preview streams the set
  from the index for its count and bytes, with the last-copy flag per row;
  the job derives the set again as it runs and checkpoints its cursor after
  each batch, so a paused or interrupted job resumes where it stopped.
- For `both`, a file goes only once its copy in B is proven. Where either
  side has no integrity hash the job reads both files in full and compares,
  and writes what it learned to the stores so the read is paid once. A pair
  whose bytes differ, or whose copy in B is gone by then, stays in A and is
  counted as skipped in the job's output, with its reason.
- Deleting leaves empty directories behind.

## Remove duplicate copies

`file dedupe <folder>` removes the surplus copies of content that exists more
than once beneath the folder: of each duplicated content, the first copy in
the folder's walk order stays. Copies are found within one source at a time,
from the index, so hash first. It validates and previews like `file delete`,
listing each kept copy and each copy that goes, and asks before dispatching;
`--dry-run` stops after the plan, `--permanent` skips the trash, `--min-size`
leaves small contents alone.

- `--keep <file>...` chooses the copies that stay instead: every other copy
  of their content goes, anywhere in the library, or beneath the folder when
  one is given. A file the index has not hashed cannot be chosen; validation
  says so (`delete.unhashed`).
- `--keep-under <dir>` removes from the folder what that directory already
  holds, matched by content wherever it sits. It is `file delete <folder>
  --against <dir> --by content --show both` under another name.
- Before removing a copy the job reads it and the copy that stays in full and
  compares integrity hashes; a pair whose bytes differ stays and is reported.
  What the reads learn is written to the stores.

## Rename files

`file rename <path> --to <name>` renames one path; `file rename <paths>...`
with rules renames many, applying the rules to each name in a fixed order:
`--replace FIND WITH` (`--regex` for captures, `--whole-name` to include the
extension), `--case lower|upper|title` and `--lower-extension`, `--prefix`
and `--suffix`, `--sequence "IMG_{n:04}"` (`--start`, `--step`) for the stem
from a counter, and `--template "{date:%Y-%m-%d} {name}{ext}"` for the whole
name. Tokens: `{name}`, `{ext}` (with its dot), `{n}` or `{n:04}`, `{parent}`,
`{date:FORMAT}` from the modification time, `{captured:FORMAT}` from the
capture time the store holds, or the modification time where it holds none.

- It validates and previews first: each old name beside its new one, then a
  y/N prompt; `--dry-run` stops after the plan. An error finding refuses:
  a name the target filesystem does not write (`rename.illegal_name`, NTFS
  and exFAT rules on those volumes and on SMB shares), a file already at the
  new name (`rename.exists`), a name the directory cannot tell from an
  existing one on a case-insensitive volume (`rename.case_collision`), and
  two files wanting one name (`rename.collision`). A change of case alone is
  `rename.case_only`, an info.
- The job renames in an order that never overwrites: a chain like
  `1 -> 2 -> 3` waits for each name to free, and a swap parks one file under
  a temporary name. Each rename is one `rename` call, so records keep their
  identity, and each is journaled for undo.

## Undo, the journal, and the trash

Every job that changes the filesystem records what it did as effects with
enough to reverse them: created, moved from and to, trashed with where it
went, replaced with where the previous bytes went, removed for good, and
attributes before and after. `job journal <id>` prints them in order.

`file undo <job-id>` reverses a job from its journal, newest effect first,
on preflight: validation refuses a job still running or with no journal
(`undo.no_journal`), warns of each file that changed since the job ran
(`undo.changed`) or whose place is now taken (`undo.occupied`), both left as
they are, and counts what cannot be reversed (`undo.irreversible`: permanent
removals, and replacements or trashings whose previous bytes were not kept).
The plan is the reverse: a delete for each creation, a move back for each
move and trashing, a replace for each replacement. `--effects 3,4` reverses
only those sequences. An undo writes its own journal, so undoing an undo is
the same command over it.

- A deletion to the trash records where the item went: `NSFileManager` on
  macOS reports the location, and on Windows and Linux the item is found in
  the trash by its original path. A volume with no trash of its own, a
  network mount among them, gets a Spacedrive trash directory at its root,
  `.spacedrive/trash/<job>/`, reached by a rename.
- An overwrite, by copy, merge or extract, moves the previous file to the
  trash the same way, so a replacement is undoable until the trash is
  emptied and costs a rename rather than a copy.

`file trash list` prints what Spacedrive trashed with a known location,
newest first, with the job and sequence to restore by; `file trash restore
<job> <sequence>` is `file undo` over that one effect; `file trash empty`
removes those items for good along with the Spacedrive trash directories,
and `--os` empties the platform's own trash as well.

## Mirror a folder

`file merge <source> --into <dir> --remove-extras` makes the destination
match the source: the merge as above, then every file the destination holds
that no source does goes to the trash, and the folders left empty are
pruned. The plan lists those files as deletes flagged where they are the
last copy of their content anywhere in the library, and validation counts
them (`merge.last_copies`). With `--on-conflict keep-newer` it is the one
way sync of a working folder to a backup, previewed. Junk is left alone.

## Organize and flatten

`file organize <dir> --by date|kind|extension` moves the folder's files
into subfolders: by date (`--field modified|created|captured`,
`--granularity year|year-month|year-month-day`), by the content kind the
indexer assigned (Images, Videos, Documents), or by extension. `--recursive`
takes the files beneath the folder at any depth. Every move is a rename
inside the folder, so records keep their identity; two files wanting one
place are a conflict left alone (`organize.conflicts`).

`file flatten <dir>` moves every file beneath a folder up to the folder
itself and prunes the emptied folders. `--on-conflict keep-both` numbers a
file whose name is taken at the root; `skip` leaves it where it is.

## Archive and extract

`file archive <sources>... --to <archive>` writes a zip or a tar.zst, by the
name's extension or `--format`, to a temporary name beside the destination
and renames it into place when complete. Validation refuses a name already
taken (`archive.exists`) and warns of free space and, with
`--remove-sources`, of the last copies among the sources; the sources go to
the trash once the archive is complete. A zip does not carry symlinks; they
are left out with a warning.

`file extract <archive> --to <dir>` plans from the archive's own directory:
a create, replace or skip per entry against the folder, `--on-conflict`
taking the merge policies, `--strip-components N` dropping leading folders.
An entry that would land outside the folder refuses the extract
(`extract.escape`). The job checkpoints the entry it reached and resumes
there; a replaced file's previous bytes go to the trash.

## Attributes and links

`file attributes <paths>... --mode 644 --modified <rfc3339> --hidden true`
sets what the filesystem lets a file carry; each flag left out stays as it
is. Validation refuses a mode on FAT32 and exFAT and hidden where it is a
leading dot rather than a flag (`attributes.unsupported`). The job journals
the attributes before and after, so undo sets them back.

`file link <at> --target <path>` makes a symlink, or a hard link with
`--hard`, which validation refuses across volumes (`link.cross_volume`) and
to a directory (`link.directory`).

## Merge folders

`file merge <sources>... --into <dir>` merges folders into an existing
folder: it recurses into folders both sides have, copies what the destination
lacks, skips files whose bytes are proven identical, and resolves a file at
the same path with different bytes by `--on-conflict skip` (the default),
`overwrite`, `keep-both` (a numbered name beside the existing file) or
`keep-newer` (by modification time, which is a claim rather than proof). A
file against a folder, a link against a file, and two sources wanting one
place are conflicts nothing resolves; they are reported and left alone.
`.DS_Store`, `Thumbs.db` and `desktop.ini` are never copied.

- It validates and previews first, printing the findings (`error`,
  `warning`, `info` with a stable code), the facts of the execution, and
  the plan's counts with every conflict and collision, then asks before
  dispatching. `--dry-run` stops after the plan; `-y` skips the prompt.
- An error finding refuses: the destination must be an existing folder, the
  roots must not contain one another, a source must be on this device and
  present. A detached source still previews from its store, so a merge can
  be planned against a drive that is unplugged. Both folders must sit in
  tracked sources for the preview; an untracked source is a warning and the
  job still runs.
- `--consume` removes each source leaf once its copy has landed or its bytes
  are confirmed identical, and prunes emptied folders, so the source ends
  holding exactly what the merge did not settle. Across volumes, tag
  assertions on the source's records stay behind; validation says how many.
- The job re-reads the plan as it starts and marks each leaf whose outcome
  differs from it. `--format json job info <id>` has no output field yet;
  the job log holds the outcomes. Dispatching the same merge while it runs
  returns the live job.
- With `--device`, the whole thing runs on that device, bytes and all.

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
