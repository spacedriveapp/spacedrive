# Physical drives

> **Related.** `2026-08-27-storage-map.md` maps what is on a volume.
> `2026-09-08-locations-demoted.md` makes locations a policy over a path.
> `2026-09-13-drive-catalog.md` describes product knowledge and hardware
> representation without participating in physical identity.
> This plan adds the layer below both: the hardware the volumes live on.
> Optional for a person with one laptop. Foundational for a person with
> twenty drives, and for every planning feature that needs to know what
> storage physically exists.

## The gap

Spacedrive stops at the volume, which is a filesystem mount. That is the
wrong boundary for the questions an archive asks:

- One drive carries many volumes: a partitioned disk, an APFS container.
- Many drives carry one volume: a raidz pool of eight disks presents as a
  single mount.
- A drive can carry zero volumes: an empty disk in a box is real hardware
  with real capacity, and it is invisible to the entire current model.

The consequences are concrete. "This file exists on three volumes" is
today's answer; "two of those volumes are the same physical pool" is the
honest one, and the difference is whether a person ships the only copy of
something believing it was redundant. Transfer planning needs capacity and
bus speed per medium, and a medium is a drive. Speed tests benchmark
volumes today, but the number belongs to the hardware. And an offline
answer ("the file is on the Expansion") wants a physical address ("which
was last seen on titan, and is currently in the cabin bag").

`Volume.hardware_id` today holds a device node, `/dev/sdb1` or `disk4s1`,
reassigned on every plug. `device_model` is unfilled on macOS and Windows.
Nothing reads a disk serial on any platform. There is no row a physical
object could be.

## Why there is no stable id, and why that is fine

The industry already tried. The WWN (World Wide Name) is a 64-bit globally
unique id burned into every SATA, SAS, and NVMe device at the factory,
allocated from IEEE-registered manufacturer ranges. It survives
reformatting, repartitioning, and moving between machines. It fails in
exactly one place: USB bridges speak SCSI to the host and either hide the
WWN or present their own. So the perfect identifier exists precisely for
the drives that never move, and vanishes for the portable drives that get
shuffled between enclosures, docks, and machines, which are the drives an
inventory is for.

The lesson generalises. Every identifier is testimony from some layer, and
layers lie. Bridges fabricate serials. Two drives from one batch share a
model and capacity. A cloned drive shares everything static. There is no
single value to find, and the design must not pretend there is.

**A drive's identity is a conclusion drawn from evidence, never a value
read from a register.** This is the same stance content identity takes in
`sd_store::content`: a ladder of evidence with explicit confidence, where
the rules that matter depend on which rung produced the answer.

## The evidence ladder

Each rung, what it survives, and what breaks it:

| evidence                                 | read from                         | survives                                                           | breaks on                                                                                                           |
| ---------------------------------------- | --------------------------------- | ------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------- |
| WWN                                      | SATA/SAS/NVMe identify            | everything                                                         | USB bridges hiding it                                                                                               |
| disk serial + model                      | identify data                     | reformat, repartition                                              | bridges substituting their own; recorded with the path it was read through                                          |
| GPT disk GUID                            | partition table header            | shucking, enclosure swaps, any bridge (it is bytes on the platter) | repartitioning                                                                                                      |
| pool / container GUID                    | zfs, mdraid, APFS metadata        | export, import on a new host, a dead member                        | recreating the pool                                                                                                 |
| filesystem UUID, `.spacedrive-volume-id` | the filesystem                    | enclosure and machine changes                                      | reformat; a root-level copy carries the fingerprint file to a different drive, so it corroborates and never decides |
| SMART trajectory                         | smartctl and platform equivalents | everything static evidence survives                                | nothing; see below                                                                                                  |
| the person                               | a question in the UI              | everything                                                         | nothing                                                                                                             |

The GPT disk GUID deserves its own sentence: it is readable through any
enclosure, survives shucking by definition, and nobody uses it. It is the
cheap rung that covers most of what actually happens to a portable drive.

**Serial provenance matters.** A serial seen over SATA or NVMe is
disk-tier. The same field seen over USB may belong to the bridge, so it is
bridge-tier: corroboration, never proof. Collapsing the two at write time
would make the reasoner impossible, which is why the observation log below
stores how each value was read.

## Identity as biography

A drive has a history, and histories cannot run backwards. SMART exposes
power-on hours, power cycle count, total bytes written, reallocated
sectors, all monotonically non-decreasing. Capacity never changes.
Firmware revision changes only forward and rarely.

So identity gains a consistency check over time. If a drive claiming to be
a known unit shows up with fewer power-on hours than that unit had at its
last observation, it is a different drive, whatever its serial says. A
clone, a lying bridge, a same-batch twin: all are caught by trajectory
even when every static identifier matches, because the forger would have
to fake a past.

The same data pays twice. A trajectory that identifies a drive is also the
drive's health history across every enclosure and machine it has lived in,
which no single OS can assemble because each one sees the drive only while
it is plugged in.

## Store observations, conclude drives

The mechanism that falls out: never store "the drive's id". Store what was
seen.

Every mount appends an observation: timestamp, the observing device, the
transport it was seen through, every identifier readable through that
path, capacity, model, firmware, and a SMART snapshot when one is
readable. Append-only; an observation is a fact about a moment and is
never edited.

A `drive` row is then a hypothesis that a set of observations describe one
physical object. Matching walks the ladder:

1. Shared WWN, or disk-tier serial + model: the same drive. Merge
   silently.
2. Shared GPT disk GUID, or membership in a known group, with no
   contradicting monotonic evidence: the same drive. Merge silently.
3. Static match with a contradiction (hours ran backwards, capacity
   differs): a different drive. Never merge, and say why.
4. Partial match (same model and capacity, serial changed, GUID
   unreadable): a question, with the evidence laid out. "This looks like
   #119: same GPT GUID, capacity, and model, but the serial changed.
   Shucked?"

A person's answer is recorded as an observation of the strongest tier, so
adjudicating once teaches the system that an enclosure serial and a bare
serial belong to one object, forever. Conflicts surface as questions and
are never resolved by a silent merge, which is the content-identity rule
(never act on a guess) applied to hardware.

## The rows

In library.db, sibling to `volumes`:

```rust
pub struct Drive {
    pub uuid: Uuid,
    /// What the person calls it: "#119", "the VODS Barracuda".
    pub label: Option<String>,
    pub model: Option<String>,
    /// Best-known disk-tier serial. Evidence, not identity.
    pub serial: Option<String>,
    pub capacity: Option<u64>,
    pub bus: Option<Bus>,            // Nvme, Sata, Usb, Sd, Network
    pub form_factor: Option<StorageFormFactor>,
    /// Where it physically is. Only the person knows.
    pub location: Option<String>,
    pub notes: Option<String>,
    /// Detected on a mount, or typed in for hardware never plugged in.
    pub origin: Origin,              // Detected | Manual
    pub first_seen_at: Option<DateTime<Utc>>,
    pub last_seen_at: Option<DateTime<Utc>>,
}

pub struct DriveObservation {
    pub uuid: Uuid,
    pub drive_uuid: Option<Uuid>,    // None until clustered
    pub observed_at: DateTime<Utc>,
    pub device_uuid: Uuid,           // which machine saw it
    pub transport: Transport,        // how it was seen; tiers the serial
    pub wwn: Option<String>,
    pub serial: Option<String>,
    pub gpt_disk_guid: Option<Uuid>,
    pub model: Option<String>,
    pub capacity: Option<u64>,
    pub firmware: Option<String>,
    pub storage_form_factor: Option<StorageFormFactor>,
    /// Raw, component-scoped product evidence. The identity reasoner never reads it.
    pub hardware_components: Vec<ObservedHardwareComponent>,
    pub smart: Option<SmartSnapshot>, // power-on hours, cycles, written, reallocated
}

pub struct DriveGroup {
    pub uuid: Uuid,
    pub kind: GroupKind,             // ZfsPool, Raid, ApfsContainer
    pub group_id: Option<String>,    // zpool GUID, container UUID
    pub label: Option<String>,
    /// The vdev tree as reported (`zpool status -P`, verbatim), plus the
    /// parsed redundancy: how many members can be absent and the group
    /// still assemble. raidz2 answers "any 6 of these 8".
    pub topology: Option<String>,
    pub min_members: Option<u32>,
    // members via drive_group_member(drive_uuid, role), role = Data | Parity | Cache
}
```

The catalog companion makes one D0 requirement urgent: product evidence must be
recorded while the hardware is present even though it does not participate in
identity. `ObservedHardwareComponent` keeps vendor strings and typed USB, PCI,
NVMe, ATA, and SCSI identifiers scoped to media, bridge, enclosure, or
controller. Flattening those layers would make an enclosure look like the disk
inside it. Serial numbers and other unit identifiers remain separate physical
evidence and never enter the shared catalog projection.

Volumes link to drives through observations (a volume observation records
which drive backed it), giving the chain the feature exists for:

**source → volume → drive → location.**

"Where is this file" resolves to "on `footage`, which is the titan pool,
which is eight drives, which are in the crate". Every link in that chain
except the last is machine-derived. The last is a text field, because only
the person knows what is in the crate, and the design treats that as a
feature: the human answer is the strongest evidence tier, never a
fallback.

## Detection

The facts are sitting in platform APIs the volume manager already lives
next to and never asks:

- **macOS**: `diskutil info` resolves a volume to its whole disk; IOKit
  gives model, serial, bus, and removability; APFS container membership is
  already parsed in `volume/fs/apfs.rs`.
- **Linux**: `lsblk -O` gives parent device, WWN, serial, model,
  rotational, and transport in one call. Pool membership is read from
  `zpool status -P` and mapped through `/dev/disk/by-id`.
- **Windows**: `Win32_DiskDrive` and storage device descriptors.

SMART needs elevated access on some platforms and a helper (`smartctl`)
where the OS offers nothing. Absence is fine: an observation with `smart:
None` still carries the static rungs. The reasoner works with whatever
each platform yields.

## Against the filesystem's own metadata

ZFS already preserves everything needed to reassemble a pool: every member
carries four copies of the vdev label, and `zpool import` on any machine
rebuilds the pool from them, in any drive order, on any controller. The
group row does not duplicate that and must not try; the platters are the
authority on their own assembly.

What the labels cannot answer is the question asked with the drives in
boxes: _which_ physical drives make the pool, where they are, and how many
of them suffice. Reading a label requires plugging the drive in, which is
exactly the step the question precedes. So the division of labor: the
filesystem carries the machine-readable truth on the platters, the group
row carries the human-facing map of where those platters physically are.

The same boundary marks what travels outside Spacedrive entirely: a
TrueNAS config backup (shares and users live on the boot disk, not the
pool) and any dataset encryption keys, without which an imported pool is
ciphertext. Those belong on the pre-export checklist, not in a table.

## What this is not

- Not required. A person who never opens the drives view never meets it;
  detection appends observations silently and nothing else changes.
- Not a product catalog. Catalog conclusions may describe a drive, but they
  never merge observations or alter physical identity.
- Not sync. Rows live in library.db and are device-owned like volumes
  until sync returns; the observation log is exactly the append-only
  shape sync ships well.
- Not a filesystem claim. A drive row asserts nothing about records; the
  map stays the map. This is the physical layer under it.

## Phases

### D0 — Observe

1. Platform detection of physical backing per mounted volume: parent
   disk, WWN, serial, GPT disk GUID, model, capacity, firmware,
   transport, storage form factor, and component-scoped product identifiers.
   On Linux most of this is readable unprivileged from sysfs
   (`/sys/block/*/device/`), which matters because the daemon runs as an
   ordinary user on a NAS.
2. SMART snapshot where readable without elevation. Where `smartctl`
   needs root, absence is recorded rather than faked; a one-off elevated
   sweep saved as text is ingestible later through D2's manual path.
3. The `drive_observation` table, appended on every mount and on a
   periodic re-check while mounted. No clustering, no UI. Raw evidence,
   with transport recorded so serial tier is never lost.

Linux lands first. The observation window closes when a drive is boxed,
and the drives with a deadline are in a NAS; macOS detection waits for
the drives that stay in reach.

### D1 — Conclude

1. The `drive` table and the clustering pass: rungs 1 and 2 merge
   silently, rung 3 refuses with a reason, rung 4 parks the observation
   as unassigned.
2. `drives.list`, with each drive's volumes, last observation, and
   evidence summary.
3. Monotonic checks on every new observation against the drive's last.
4. ZFS pool membership, pulled forward from D3: `zpool status -P` parsed
   into a group with its GUID, topology, member roles and `min_members`.
   The rest of D3 stays put; this piece moves because a pool about to be
   exported is the group whose membership is hardest to re-derive later.

### D2 — The person

1. Manual drive rows for hardware never plugged in: label and capacity
   typed in, everything else filled by the first observation that matches.
2. Label, location, and notes ops. `drives.merge` and `drives.split` for
   adjudication, each recording the answer as a top-tier observation.
3. The rung-4 question surfaced in the UI with its evidence.

### D3 — Groups

1. Membership from mdraid and APFS containers (zpool landed with D1).
2. Redundancy honesty: copy counts and the duplicates view collapse
   volumes that share a group, so a pool never counts as two places.

### D4 — Surface

1. Drive cards: volumes carried, capacity, health trend from the SMART
   trajectory, last seen where and when.
2. The source → volume → drive → location chain in the Inspector, online
   or not.
3. Speed test results attach to the drive.

Planning and simulation (what fits where, transfer ordering against
capacity and bus speed) build on D0 through D3 and are their own plan, as
is importing an existing hand-kept inventory, which is a foreign adapter
over a local file like any other.

## The order against everything else

Nothing here blocks the teardown, and the teardown blocks none of this.
D0 is deliberately first and deliberately dumb: it records evidence
without interpreting it, because every later phase reads the log, and an
observation not recorded in September cannot be reasoned about in
December. The reasoner can be wrong and rewritten; the log cannot be
backfilled.
