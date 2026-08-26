# Extensibility: presets, native dependencies, plugins

> Captured 2026-08-25 from a voice dump, then checked against the tree. Most of
> what it asks for is closer than it sounds, and one piece of it is both small
> and on the critical path for
> `2026-08-25-filesystem-intelligence.md`.
>
> **Related.** `2026-07-29-install-size.md` already unbundled the codecs.
> `2026-08-18-native-app-gpui.md` is the GPUI plan this revisits.
> `docs/extensions/*` is the existing extension SDK documentation.

## The four threads

The dump contains four separable things, and separating them is most of the
work:

1. **Native dependencies** as managed software rather than bundled bytes.
2. **Job presets**: a saved, parameterized, re-runnable job configuration.
3. **Plugins** as the packaging unit that carries jobs, binaries, skills,
   workflows and app definitions.
4. **Sidecars**, and where a thumbnail durably lives now that the hot tier
   exists.

Only (2) is both new and small. (1) is half done. (3) is a wrapper around the
others and should be last. (4) is a question about location rather than a
missing system.

## What is already true

**The codecs are already unbundled.** `2026-07-29-install-size.md` moved the
91 MB codec bundle behind `cargo xtask setup --native-deps`, made `ffmpeg` and
`heif` optional features, and taught the Tauri template to render without them.
A fresh clone no longer pays for codecs it will not use. So "we should not
bundle ffmpeg" is done; what does not exist is a *registry*, which is a
different thing.

**The SDK is already a plugin SDK.** `crates/sdk/src` carries `actions`,
`job_context`, `tasks`, `agent`, `ai`, `vdfs`, `models`, `query` and `ffi`,
with seven documents under `docs/extensions/`. The surface exists; what is
missing is the manifest, the lifecycle and the registry that would let a
third party install one.

**Thumbnails already have a durable form in the design.**
`SidecarKind::Thumb` exists (`core/src/ops/sidecar/types.rs`), and
`crates/sidecar-path` resolves it to
`content/{h0}/{h1}/{content_uuid}/thumbs/{variant}.{ext}`. The sidecar tree is
content-addressed, so a sidecar survives a rename and can be recognised on
another device. Generation is parked, not deleted: the teardown lists detail
sidecars under "temporarily dark" until enrichment drains exist.

## The preset is the primitive

**A preset is a row: a registered job kind, a set of parameters, a name, and
who made it.** A job is what runs one. That is the whole model, and it needs
neither plugins nor a dependency registry to exist.

It is worth building first because it is the same shape as the workflow in the
filesystem intelligence plan. Both are durable, parameterized, executable,
logged, and carry the reasoning that produced them. "Convert these to MP4 the
way I like it" and "move these files to that drive because it has the capacity"
differ in what they do and not in what they are. One primitive, two uses, and
the second one is already on the critical path.

It also pays off immediately over MCP. List presets, invoke one, read the log:
that is precisely "let the AI reuse my preferences", and it needs no model
running locally and no plugin system.

### The boundary that makes presets safe

The dump proposes letting models produce jobs rather than scaling Rust
horizontally. That is right, with one line drawn hard:

**A model composes registered kinds. A plugin adds kinds.**

A preset is parameters over a job kind that already exists and has already been
reviewed. If a model can author the *command*, a preset becomes a shell script
with a nicer name, and everything that makes a managed job valuable turns into
a liability: durable execution, retries and monitoring are good properties for
work someone vetted and bad ones for arbitrary strings. Keeping models on the
parameter side of that line is what lets the rest be permissive.

Built-in kinds worth having on day one are the ones already implemented or
nearly so: transcribe, extract audio, proxy generation, thumbstrip, gaussian
splat. Those become preset-able rather than being rewritten.

## Native dependencies want a registry

Unbundling solved distribution size. It did not answer:

- Which version of a tool is installed, and does its hash match what was
  expected.
- Which platform build to fetch, and from where.
- **Which job kinds are runnable right now**, given what is present.

The third is the interesting one, because it makes job availability dynamic.
The UI and the MCP tool list both have to be able to say "this preset needs
ffmpeg, which is not installed", and offer to fetch it. That is a small amount
of state and a significant amount of surface, and it is the reason the registry
is not just a downloader.

Trust boundary: a registry that fetches and executes binaries is the most
security-sensitive thing in this document. Pinned versions, hash verification,
and no automatic execution of anything the person did not install by name.

## Plugins are the wrapper, and they go last

A plugin registers job kinds, declares the binaries they need, and ships
presets, skills, workflows and eventually app definitions. Every one of those
is a payload type that has to exist first and is useful on its own. Building
the wrapper before the payloads means designing a manifest for things whose
shape is not settled.

So the order is: preset rows, then job kind registration, then the dependency
registry, then the manifest that bundles them.

## Sidecars: two tiers, and the open question is where

Not a rewrite. Both tiers are already designed and they do not compete:

| | `thumbs.pvcache` | `thumb` sidecar |
|---|---|---|
| Form | BGRA8 tiles, fixed size, mmap | encoded file, original form |
| Keyed by | record uuid, per source | content uuid, content-addressed |
| For | drawing a grid with no decode | keeping the render |
| If lost | rebuild from the sidecar | rebuild from the original bytes |

The hot tier is a display cache and says so. The sidecar is the durable
artifact. The instinct in the dump is right and the design already agrees with
it; what is parked is generation, not the concept.

**The open question is where the sidecar tree lives.** Today it is
library-scoped. For an archived external drive that is wrong: the previews
should travel with the drive, so that plugging it into another machine does not
re-bake 27 TB of video. That argues for the sidecar root following the *source*,
with the option to place it on the medium itself, which is the same "the store
might live on the drive" idea from the filesystem intelligence dump applied to
derived data.

That should be a per-source setting, since a fast internal volume and an
archival drive in a drawer want opposite answers.

**And the line is the source.** The dump asks where location versus source is
drawn now. Locations are being deleted (`2026-08-22-source-convergence.md`, P4);
sources absorbed them. Anything that reads "per location or per source" is per
source.

## GPUI

The plan (`2026-08-18-native-app-gpui.md`) already commits to GPUI and already
scopes `apps/native` to Photos: an app launched from Spacedrive that follows a
file explorer window through `navigation.focus`, not a second Spacedrive. That
revision was deliberate and it still looks right.

The honest read on going further. Parity between two full interfaces is a
standing tax on every feature, paid forever, by a team that is currently
mid-teardown on the layer underneath both. The photos app is the correct wedge
precisely because it is narrow: it proves the hot tier, the daemon protocol and
the component set against a real surface, and none of that work is wasted if
the full explorer follows.

What would change the calculus is the backend settling. Once the entries world
is gone and the source substrate is the only substrate, a second client is
reading one stable set of ops rather than tracking a moving one. Before then,
every op that gets rewritten gets rewritten twice.

So: build Photos, keep the web UI as the full interface, and let the native
explorer follow the teardown rather than race it.

## Against the thirty days

Stating this plainly because the dump is a scope expansion arriving during a
deadline: **none of this is on the path to the 160 TB migration.** That path is
deliberate sources, then the catalog, then MCP, and it stays that way.

The one piece worth pulling forward is the preset row, and only because the
workflow primitive in the filesystem intelligence plan needs the same table. If
it gets built there, presets are a second consumer rather than new work.
Everything else here should wait for the teardown to finish, which is also when
plugins stop being a manifest over a moving target.
