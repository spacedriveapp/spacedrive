# Filesystem Intelligence

> The product angle, written down before it evaporates. Sources landed
> (`2026-08-22-source-convergence.md`), which is what makes any of this
> reachable.
>
> **Related.** `2026-08-20-entries-teardown.md` phase 5 already names
> `catalog.db` as "global enumeration + placement rows swept from source
> stores". That sentence turns out to be the whole feature, and this document
> is the argument for pulling it forward.
> `2026-08-20-architecture-previs.md` is the destination.
> `docs/core/design/source-durability.md` settles what a store owes when its
> origin stops answering, which is the case a detached drive lives in.

## The bet

Every tool being built right now sits above the filesystem: editors, harnesses,
runners, sandboxes. The filesystem itself is assumed. It is assumed to be one
machine, one mount, everything attached at once, and an agent shelling out to
`find`.

That assumption breaks the moment a person owns more than one drive, and it
breaks completely for anyone with real data: a founder, a creator, a studio, a
lab. Their data is spread over drives, devices and clouds that are almost never
all connected at the same time. There is no map. An agent pointed at that
situation can only see the fraction currently mounted.

**Spacedrive's claim: the map is the product, and any agent is the interface to
it.** Index everything once, keep the map when the drive goes in a drawer, and
answer questions about data that is not currently plugged in. Which harness asks
the questions is the person's choice and none of Spacedrive's business.

## The use case that proves it

Moving countries with 160TB across a NAS, loose drives, and machines. The
questions are ordinary and currently unanswerable:

- Where is everything, including on the drives that are unplugged right now?
- What is duplicated, and how many bytes would I get back?
- What exists in exactly one place, and is therefore one drive failure from gone?
- Given these drives with these capacities and these speeds, what configuration
  fits, and what is the cheapest sequence of moves to reach it?

Nothing about this is exotic. It is what anyone with more than one drive
eventually wants, and the reason nobody asks is that no tool has ever been able
to answer.

The current workaround is the tell: hand-written scripts to hash and index a
NAS, run in place, to build a throwaway map that answers one question once.

## The primitive

The open question was what the planning primitive should be. It falls out of
the catalog, which is already designed.

**The catalog holds actual placement. A plan is a desired placement. The
workflow is the diff.**

That is the whole model:

| | Rows | Written by | Meaning |
|---|---|---|---|
| Catalog | `(content_uuid, source_id, external_id)` | swept from source stores | where bytes are now |
| Plan | `(content_uuid, target_source_id, reason)` | a person, or their agent | where bytes should be |
| Workflow | the diff, ordered | derived | how to get from one to the other |

Everything the UI wants to show is a projection of that diff. Bytes to move,
per-drive capacity after, time from measured volume speeds, and a reason per
step, because every row carries the one that put it there.

Redundancy is a constraint over the same rows: a plan targeting two copies of
the company records is satisfied when two placements exist on volumes with
different `device_id`. At-risk is the same query with the count at one. No new
concept, just a predicate over placement.

This matters because it means the planner is not a special subsystem. It is a
diff, and the executor is a queue of moves with verification by re-index.

## What already exists

More than expected. The sources work put most of it in place.

**Drive inventory is done.** `core/src/domain/volume.rs` already carries
fingerprint, `device_id`, total capacity, available space, disk type
(SSD/HDD), filesystem, mount type, read-only, hardware id, and
`core/src/volume/speed.rs` measures real read and write throughput per volume.
Capacity and transfer-time projection need no new data.

**The map per drive is done.** `record` plus `facet_file`, with `parent_uuid`
links and identity that survives a move
(`crates/store/src/file.rs`).

**A drive in a drawer stays queryable.** A detached source restores its arena
from its snapshot read-only, and its store is a normal SQLite file that does
not care whether the origin is present. This is the hard part of the use case
and it already works.

**Annotation has a home.** `record_overlay` was built to hold "what no ingest
produced: a person's assertions". It has no foreign key to `record` on purpose,
so it outlives re-indexing. It carries `(type, external_id, content_uuid)` as
rebind evidence, and `hlc` plus `device_uuid` to order merges. A model writing
notes about a directory is exactly this table, and directories are records, so
attaching to a subtree needs nothing new. What it needs is an author field, so
a model's claim is distinguishable from a person's.

**Content identity is already tiered.** `crates/store/src/content.rs` splits
`ContentId` into `Candidate` (sampled hash) and `Confirmed` (integrity hash),
with the rule that a destructive decision requires the confirmed tier. That
distinction is load-bearing for this feature and is examined below.

## What does not exist

- **Cross-source anything.** `sd-store` says it outright: "Nothing here spans
  sources. That arrives with the catalog." Duplicates across drives, at-risk
  filters, and global search are all catalog work, and the catalog is currently
  scheduled last.
- **A way to make a source deliberately.** Today a filesystem source appears as
  a side effect of indexing a volume. There is no flow for "track this drive",
  and no flow for a folder that is not a whole volume.
- **MCP.** Nothing in the tree.
- **Any model runtime.** `crates/sdk/src/ai.rs` is extension stubs with no
  inference behind them, so the local provider client, the agent loop, and its
  budget accounting are all greenfield.
- **The plan and workflow tables**, and any executor for them.

## The physics, which decides the design

160TB read once at a generous sustained 200MB/s is on the order of nine days of
continuous reading, per full pass, assuming no seeks and nothing else competing.
Real mixed content with millions of small files is worse. A copy is a read plus
a write.

Three consequences, and they are not negotiable:

**Full hashing of everything is off the table.** The sampled tier reads a
bounded number of blocks per file regardless of file size, so a sampled pass is
bounded by file count and IOPS rather than by bytes. A 100GB video costs the
same as a 1MB one. This is the difference between a map that can be built and
one that cannot.

**So the ladder gets spent deliberately.** Planning needs size, path and volume
only, which the walk already produces. Finding duplicate candidates needs the
sampled tier. Deleting one copy of two needs the confirmed tier, and only for
the specific pairs a person is about to act on. `ContentId::confirmed()`
already encodes that gate; the planner has to respect it rather than treat a
candidate as an answer.

**The planner's objective is bytes not moved.** Moving data is bounded by the
same physics as reading it, so a good plan is mostly a plan to leave things
where they are. Any placement solver that optimises for tidiness over movement
is solving the wrong problem.

## Two agents, and only one of them is the interface

**The interface is a person and their agent in a harness, over MCP.** Claude
Code, or whatever replaces it. Spacedrive does not ship a harness and does not
compete with one.

**Filesystem intelligence is a Spacedrive-native agent loop over a local
model.** Headless, unattended, and its output is rows rather than messages. It
runs while drives are attached and walking, and by the time anyone connects a
harness the map already carries meaning.

Those are different things and conflating them is the trap. The native loop is
not a chat surface, not a product feature a person talks to, and not a fallback
for people without an agent. It is a producer.

**Harnesses are commoditising at speed and Spacedrive should not be loyal to
one, including its own.** Spacebot is a variable: it may exist, it may not, and
nothing here may assume it. The map is the asset because nobody else is building
it. A harness is a depreciating one because everybody is.

### The native loop consumes the same MCP tools

The tools the native loop calls are the tools shipped over MCP. One definition,
two consumers.

This is what keeps the loop from becoming lock-in. It is a client of the public
surface with no privileged access, so pointing a frontier model at the same
tools is a configuration change rather than a rewrite. It also makes the loop a
completeness test with teeth: if the native agent needs something the MCP
surface does not expose, the surface is wrong, and that shows up as a failure
rather than as an internal shortcut nobody notices.

Ordering consequence: MCP is a dependency of the annotation loop, not a sibling.

### The reference model

**Qwen3.8-27B.** Apache 2.0, dense 27B, native multimodal, 262k native context,
controllable reasoning effort. Quantised to 4-bit it is roughly 16 to 17GB, so
it runs on a high-end laptop with enough unified memory or a 24GB GPU, and it is
already in Ollama and LM Studio.

Reference rather than requirement. The loop targets an OpenAI-compatible
endpoint and a tool schema, so the model is configuration. Its published agentic
numbers are mostly vendor-reported and independent confirmation is still
settling, so nothing here should depend on a specific benchmark holding. What
matters is the class: open-weights models at this size now do multi-step tool
use well enough to drive a loop, which was not true a year ago.

Three properties change the design.

**Apache 2.0 means it can be recommended and shipped with.** No licensing
asterisk on the one component that would otherwise carry one.

**262k context flips the unit of work from a directory to a branch.** An entire
subtree's structure fits in a single prompt, so the loop's step is "look at this
branch and tell me what matters in it" rather than "describe this one folder".
Fewer round trips, and the model can see siblings and depth when deciding where
to spend.

**Reasoning effort is a dial, and annotation wants it low.** Thinking is on by
default and `xhigh` is verbose. Annotation is a judgement over structure that is
already laid out, not a proof, and intermediate tokens are the scarce resource
here for the reason below.

### Why a loop, and why long context does not rescue it

Output tokens bind, not input. Long context makes it cheap to *look* at a
hundred thousand directories and no cheaper to *write* a hundred thousand
annotations.

Rough shape of it: a short annotation is on the order of tens of tokens, local
generation on this hardware is tens of tokens a second, and a large tree has a
few hundred thousand directories. That multiplies out to days of continuous
generation for uniform coverage, on top of the days of walking drives. Long
context does not touch that number.

What it does change is that **fewer annotations become sufficient.** Seeing a
whole branch at once is what lets the model say "this entire subtree is 2019
Twitch VODs, one annotation covers it" with justified confidence, instead of
descending and writing three thousand nearly identical ones. Coverage per output
token goes up because the unit of assertion gets bigger, not because generation
got faster.

So selective attention is still the feature, and it is still a loop. `node_modules`
gets a glance and a skip. An ambiguous directory gets a drill and a real answer.
The difference long context makes is that the decision is now well informed at
the point it is taken.

Hierarchy pushes the same way: a parent's annotation is better written after its
children's, which is state carried across steps.

### What the loop needs to survive contact

- **Providers.** LM Studio and Ollama first, both over OpenAI-compatible chat
  completions, so the abstraction is a base URL, a model name, a reasoning
  effort and a tool schema. A hosted endpoint is the same client.
- **Budget and resume.** A per-source budget in output tokens, since that is the
  binding constraint, checkpointed so unplugging a drive mid-run costs the
  current step rather than the run.
- **Cheap skips.** Deciding not to look must cost far less than looking, or the
  selectivity that justifies the loop is spent on deciding it.
- **A weaker-provider fallback.** Tool calling is no longer the risk it was for
  this model class, but the loop should not assume it. A constrained
  structured-output path keeps smaller or older local models usable, at worse
  coverage per token.
- **Graceful absence.** The hardware floor is real: a 32GB machine or a 24GB
  GPU. Below it the loop simply does not run, and the map is still complete and
  still queryable by whatever agent the person already pays for. The feature
  degrades to the thing the design already supports for free.

### Granularity and what it reads

**Directories, not files.** A tree with tens of millions of files has a few
hundred thousand directories, three orders of magnitude fewer, and that is the
granularity a migration plan operates at anyway. Nobody places 40 million files
individually; they place `Twitch/2019`.

The loop reads structure by default: names, child distribution, extensions, size
and date ranges, all already in the store. Reading actual bytes is a tool call
it can choose to make for a README or a manifest, and that choice is bounded by
budget rather than by policy.

Annotations land in `record_overlay` against the directory record, so they
survive re-indexing and travel with the source. Each row carries which model
wrote it, because a swappable producer means rows from several will coexist.

**The thumbnail tier is a latent second input.** `thumbs.pvcache` already holds
BGRA8 tiles keyed by record uuid, one cache per source, mmap-readable from
another process (`crates/pvcache`). A native multimodal model plus a contact
sheet assembled from tiles the loop is already holding uuids for is the
difference between "4,812 files named DSC_*.jpg" and knowing what the shoot
was. It costs no new extraction, since baking thumbnails is already on the path.

**Settled: annotations do not expire.** An annotation is an assertion, and
assertions in `record_overlay` outlive the generation by design; a model's claim
is treated exactly like a person's. This costs no schema, and it forecloses
nothing: `record_overlay.updated_at` and `record.modified_at` already sit beside
each other, so anything that later wants to surface an aged claim can derive it
without a new column.

Deliberately not P5 day one. Images cost far more input tokens than a directory
listing, and the value has to be proven on structure-only annotation before
spending a budget that is already tight. Worth designing the annotation row so
that a later visual pass refines it rather than replacing it.

## What is deliberately not Spacedrive's job

Building a harness, or a chat surface. Everything a person interacts with
belongs to whatever agent they already use.

The tools shipped are the map's query surface plus the guidance for using it:
volume inventory, cross-source lookup, duplicate and at-risk queries, plan
read/write, and workflow control. A person plans their migration by talking to
their own agent and watches it happen in the Spacedrive UI. The native loop
calls that same surface with nobody watching.

**MCP is the primary interface, not an export of one.** No capability may exist
only inside the Spacedrive app. The test is that deleting every first-party
conversational surface tomorrow costs nothing but convenience.

## Phases

**P1. Deliberate sources.** A flow for "track this drive" and "track this
folder" that creates a source with a stable identity, independent of a browse.
Volume indexing already produces the walk; what is missing is the intent.
Prerequisite for everything, because the map is only as good as the set of
things registered in it.

**P2. Catalog.** Pull `catalog.db` forward from teardown phase 5. Global
enumeration plus placement rows swept from source stores. This is the keystone:
duplicates, at-risk, and global search over detached drives all appear the
moment it exists. It reads from source stores, which now exist, so it does not
depend on the entries teardown finishing.

**P3. MCP.** Expose inventory, map queries, and catalog answers as tools. Small
surface, and the point at which the use case becomes real for an outside agent
even with no planning primitive at all. Complete by rule: anything the app can
do, an outside agent can do. Also the tool surface P5 runs on, so it lands
first.

**P4. Plan and workflow.** Desired placement rows, the diff against catalog
placement, capacity and time projection from volume speed, per-step reasoning,
and an executor with resume. Redundancy expressed as a placement constraint.

The workflow row is the same shape as a job preset
(`2026-08-25-extensibility.md`): durable, parameterized, executable, logged,
carrying the reasoning that produced it. Worth building it as one table with two
consumers rather than discovering the duplication later.

**P5. Filesystem intelligence.** The native agent loop: a local model driving
the P3 tools, headless, budgeted in output tokens and resumable, writing
annotations to `record_overlay` at branch granularity. Qwen3.8-27B as the
reference model, LM Studio and Ollama as the first two providers over their
OpenAI-compatible endpoints. Needs an author field on overlays first, so a
model's claim never silently reads as a person's and rows from a replaced model
stay distinguishable from its successor's. Structure only; the visual pass over
`thumbs.pvcache` is a later refinement on the same rows.

**P6. Planning UI.** Configuration comparison, the "what if I delete this and
split across those" view, workflow progress with per-step reasoning.

P2 and P3 together answer the use case with no planning primitive at all, using
someone's agent as the planner. That is the sequencing insight: the value
arrives before the product does.

## The shortest path to a real migration

Worth stating separately from the product, because the two are not the same
scope and confusing them would cost the deadline.

An actual 160TB migration needs P1, P2 and P3, and nothing else. Register every
drive once, let each one index while attached, sweep placement into the
catalog, expose it over MCP, and plan by conversation. Moves get executed by
whatever tool is already trusted for moving bytes, and verification is a
re-index and a placement diff.

P4 through P6 are what turn that into something other people can use without a
terminal. They are the product. They are not the prerequisite.

One ordering note that comes from the physics rather than from the code: the
indexing passes are the long pole and they are gated on physically attaching
drives. Registering and walking drives can start as soon as P1 lands, and
should, because that clock runs whether or not the rest is written.

## Open

- **Does `location` survive?** With deliberate sources, a location and a
  filesystem source look like the same concept wearing two names. Leaning
  toward the source absorbing it, which would fold the question into the
  teardown rather than answering it separately.
- **Where does the catalog live?** Library-scoped is the obvious answer, but
  the migration case involves a person carrying drives between machines, so
  whether a catalog can be handed to another device is worth settling before
  the schema sets.
- **Sampled hash cost on a NAS.** The sampled tier is IOPS-bound, and a spinning
  array over a network is exactly where IOPS is scarce. Worth measuring on real
  hardware before promising that the candidate map is cheap.
- **Annotation trust.** A model's claim about a directory is evidence rather
  than fact, and the same rule that stops a candidate hash from authorising a
  delete should stop an annotation from authorising a move. `TrustTier` already
  exists for source content; annotations may want the same treatment.
- **How big can one branch prompt usefully get?** 262k tokens of context is not
  262k tokens of attention that stays sharp. The question is not whether a whole
  subtree fits but at what size the model stops noticing the thing three
  thousand lines up. That sets the loop's real step size and it has to be
  measured rather than assumed from the context number.
- **What is the budget policy?** Per source, per session, per drive-attachment
  window? The drive is the scarce resource, since it may be unplugged at any
  moment, which argues for spending eagerly while attached rather than pacing
  evenly. Denominated in output tokens, since that is what binds.
- **Does the loop earn its place against a frontier agent?** Because it runs on
  the same MCP tools, an outside model can do the same work whenever someone
  wants to pay for it. The local loop justifies itself on scale and privacy: a
  few hundred thousand directories is a lot of round trips to a paid endpoint,
  and nothing leaves the machine. If that stops being true it is a provider
  swap, not a rewrite, which is the point of building it on the public surface.
