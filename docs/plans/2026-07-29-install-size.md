# Install size: what we measured and what was actually wrong

Captured 2026-07-29. Direction is James's; the measurements are mine and every
number below is reproducible from the commands recorded here.

The goal was to find out how small Spacedrive can get today, without removing
features, so the README can make a claim backed by a number. The answer is 156 MB
down to 44.4 MB, and almost none of it came from where the previous plan expected.

---

## Headline

| | |
|---|---|
| macOS payload before | ~156 MB (73.2 MB binary + 83 MB framework) |
| Default build now | **44.4 MB, a single standalone file** |
| Reduction | **72%**, with no feature removed from the product |

Getting there, each step measured:

| Step | Binary |
|---|---|
| Baseline, `--features ffmpeg,heif` | 73.2 MB |
| `default = []` | 70.1 MB |
| Source maps excluded from the embed | 53.3 MB |
| Icon set converted to WebP | 45.3 MB |
| Image set converted to WebP | **44.4 MB** |

The embedded frontend payload went from 17.58 MB to 8.71 MB across those last two
steps, and no PNG remains in the bundle.

The default build links only macOS system libraries and has zero
`@executable_path` references. It needs no downloaded bundle at all.

---

## Method, and why the obvious tool lied

`cargo bloat` reports the `.text` section only. On this binary `.text` is 32.5% of
the file, so it attributes a third of the problem and stays silent about the rest.
Reading it alone would have sent us after dependencies for a week.

`size -m` on the Mach-O gives the real breakdown and should be the first command
next time.

Baseline, `sd-server` built `--release --features ffmpeg,heif`:

| Section | Size |
|---|---|
| `__const` | 36.7 MB |
| `__text` | 28.5 MB |
| `__eh_frame` + `__gcc_except_tab` | 4.5 MB |
| `__cstring`, `__unwind_info`, other | ~1.4 MB |

Half the binary was constant data. None of it was code.

Note that `strip = true` in `[profile.release]` (`Cargo.toml:154`) means
`cargo bloat` cannot read symbols at all unless you override it:

```sh
CARGO_PROFILE_RELEASE_STRIP=false cargo bloat --release -p sd-server --crates -n 40
```

---

## Findings, in order of how wrong we were

### 1. Feature flags are nearly worthless here

Turning off `wasm`, `ffmpeg` and `heif` together moved the binary from 73.2 MB to
70.1 MB. A 3.1 MB saving, 4%.

The reason is that FFmpeg and libheif are dylibs loaded at runtime from the
framework, so gating the feature removes the thin Rust wrapper and leaves the
codecs entirely untouched. Only wasmer was a genuine static removal.

Any plan that justifies itself on "gate the heavy features to shrink the build"
needs to say which of them are actually statically linked. Most are not.

### 2. 16.68 MB of the binary was JavaScript source maps

`apps/server/src/main.rs:35` embeds `apps/web/dist/` wholesale via `rust-embed`.
That directory is 33 MB:

| Type | Count | Size |
|---|---|---|
| `.map` | 4 | **16.68 MB** |
| `.png` | 173 | 12.19 MB |
| `.js` | 4 | 4.82 MB |
| everything else | 23 | 0.57 MB |

`apps/web/vite.config.ts:113` sets `sourcemap: true`, and nothing filtered the
output before embedding it. Every production `sd-server` ever built has carried
source maps for its own frontend, `index-*.js.map` alone being 10.63 MB.

Fixed by excluding them from the embed rather than from the build, so they stay on
disk for debugging:

```rust
#[derive(Embed)]
#[folder = "../web/dist/"]
#[exclude = "*.map"]
struct WebAssets;
```

`rust-embed` gates that attribute behind its `include-exclude` feature, which has
to be enabled in `apps/server/Cargo.toml` or the derive fails with a message that
does not mention the derive.

Result: 70.1 MB to 53.3 MB. `__const` fell from 36.7 MB to 19.9 MB, matching the
map total exactly.

### 3. 45.3 MB of the framework is a dead object detection stack

`Spacedrive.framework` ships `yolov8s.onnx` (22.4 MB, dated February 2025) and
`libonnxruntime.dylib` (22.9 MB) to run it.

Nothing links onnxruntime. No `Cargo.toml` in the workspace declares `ort` or
`onnx`. `crates/archive/src/safety.rs:3` is a stub whose comment says the ONNX
runtime was never integrated, and `crates/archive/src/embed.rs` is a stub
returning zero vectors. The only reference anywhere in the repo is
`scripts/utils/patchTauri.mjs:83`, which copies the model into the bundle.

Every macOS install has carried a 45 MB model and runtime that never executed.
This resolves itself rather than needing a fix, because the default installation
will not ship the native-deps bundle at all.

### 4. The icon set is 10.8 MB of PNG that should be 2.5 MB of WebP

`packages/assets/icons` holds 197 PNGs. 161 are 384x384, 26 are 40px variants, and
a handful are oversized outliers: `Terminal.png` is 1152x1152 and 421 KB,
`Sync.png` is 768x768 and 387 KB.

All of them ship. The generated `packages/assets/icons/index.ts` imports every
icon and the file-kind lookup is dynamic, so nothing tree-shakes. That is inherent
to a file manager, which needs every file-type icon available at runtime, so the
fix is format rather than culling.

Measured over the full set:

| | Size |
|---|---|
| Original PNG | 10.80 MB |
| Resizing the oversized outliers only | 9.98 MB |
| WebP q85, alpha_q 100 | **2.46 MB** |

Dimensions are close to irrelevant and the encoding is the whole saving: 8.35 MB,
77%. Lossless WebP was not measured because `-z 9` over 197 files exceeded a
two minute budget; it will land somewhere between the two.

**Done.** 197 files converted at q85 with `alpha_q 100`, 6 oversized ones resized
to 384 first, and the PNGs deleted. `packages/assets/scripts/generate.mjs` derives
variable names with `fileName.split('.')[0]` so it regenerated `index.ts` without
modification, and `packages/assets/util/{index,mobile}.ts` go through that index so
they needed no change. 85 direct `@sd/assets/icons/*.png` imports across 18 files
in `packages/interface`, `packages/ts-client` and `apps/mobile` were rewritten, and
`types.d.ts` now declares `@sd/assets/icons/*.webp`.

Verified by smoke test: `sd-server` serves icons as `image/webp`, and requesting a
source map falls through the SPA handler to `index.html` rather than returning the
10.6 MB file.

Metro's default `assetExts` includes `webp` and `apps/mobile/metro.config.js:33`
only removes `svg`, so React Native picks the new files up. iOS decodes WebP
through ImageIO from iOS 14 onward, which is worth a device check before release.

### 5. One encoder setting does not fit one asset folder

`packages/assets/images` (14 files, 1.92 MB) was converted next and behaved
nothing like the icons. At q90 the whole folder saved only 10%, because the three
`Bloom` gradients came out **larger than the PNGs they replaced**, by 60%, 59% and
90%. Smooth alpha gradients are the worst case for lossy WebP, and PNG's row
filters handle them better.

Lossless WebP fixed exactly the files lossy failed on:

| File | PNG | lossy q90 | lossless |
|---|---|---|---|
| BloomTwo | 279 KB | 530 KB | **250 KB** |
| Dropbox | 78 KB | 77 KB | **25 KB** |
| Mega | 8 KB | 9 KB | **3 KB** |
| AppLogoV2 | 233 KB | **14 KB** | larger |

So the rule applied was per file rather than per folder: encode both ways, keep
whichever beats the original, and prefer lossless unless lossy is more than 20%
smaller. That biases artwork away from artifacts where the saving is marginal.
Result 1.92 MB to 1.00 MB, 48%, with all three gradients and the flat-colour
brand marks left mathematically identical to the originals.

Worth remembering before anyone runs a bulk `cwebp -q` over an asset directory and
assumes the total went down.

### 6. iroh is 2.4 MiB, so gating it is not a size decision

Summed from `cargo bloat`, the entire P2P stack in `.text`:

| Crate | Size |
|---|---|
| iroh | 934.6 KiB |
| iroh_relay | 415.0 KiB |
| igd_next | 292.7 KiB |
| iroh_quinn_proto | 270.8 KiB |
| hickory_proto | 189.8 KiB |
| swarm_discovery | 163.8 KiB |
| portmapper | 107.7 KiB |
| hickory_resolver | 84.5 KiB |
| **total** | **~2.4 MiB** |

Phase 2 of `2026-07-28-lighten-and-consolidate.md` is worth doing for
simplification and to make room for a Tailscale transport, and it is not worth
doing for binary size. That doc should be amended to say so.

Two other corrections to that plan while we are here. It counts 25 files
referencing iroh, which is right for the crate but wrong for the work: the
`NetworkingService` **type** appears in 306 references across 50 files, so gating
the module means touching all of them. And `sd-server --help` already exposes a
`--p2p` flag, so the capability is opt-in at runtime today, which lowers the
urgency further.

### 7. Our own crate is the largest single consumer of code

`sd_core` is 9.9 MiB of the 25 MiB `.text`, or 39.7%. Bigger than std, bigger than
every dependency combined in its tier. That is monomorphization across sea-orm,
specta and the action registry, and it means dependency removal has a floor well
above zero. Worth its own investigation before anyone promises a very small
binary.

---

## What changed

No code was removed to get the size down:

| File | Change |
|---|---|
| `core/Cargo.toml` | `default = ["wasm"]` becomes `default = []`, plus the `sd-imageio` macOS dependency |
| `apps/server/Cargo.toml` | `rust-embed` gains the `include-exclude` feature |
| `apps/server/src/main.rs` | `#[exclude = "*.map"]` on `WebAssets` |
| `packages/assets/{icons,images}` | 211 PNGs converted to WebP, imports rewritten |
| `crates/imageio` | new, the ImageIO and QuickLook backend |
| `core/src/ops/media/thumbnail/generator.rs` | `System` generator variant, macOS only |
| `xtask`, `.cargo/config.toml.mustache` | the codec bundle download moves behind `--native-deps` |

The 19 `#[cfg(feature = "wasm")]` sites, `extensions/`, and the wasmer dependency
are all untouched. `mobile` already built with `default-features = false`, and no
crate in the workspace opted into `wasm` explicitly, so nothing needed to opt back
in. The `ffmpeg` and `heif` features still work and still take priority on the
platforms that have no system codecs to fall back on.

---

## What the default build gave up, and how it was closed

Turning the features off cost exactly three things, all of them thumbnail paths:

- video thumbnails (`ffmpeg` feature off)
- HEIC and RAW decoding (`heif` feature off)
- PDF thumbnails (`crates/images/src/pdf.rs:80` binds pdfium at runtime and the
  dylib is absent)

Indexing, FTS5 search, P2P sync, cloud volumes, the archive, JPEG/PNG/WebP
thumbnails and the embedded web UI were intact throughout.

**Phase 1 closes all three, and is done.** `crates/imageio` (`sd-imageio`) wraps the
system codecs: ImageIO for anything it can decode, QuickLook for everything else
Finder can preview. `core/src/ops/media/thumbnail/generator.rs` gained a `System`
variant that takes priority on macOS for `image/*`, `video/*` and
`application/pdf`, and re-encodes to WebP so the output contract matches the other
generators. One dependency, `core-foundation`, and the binary stayed at 44.4 MB.

Measured against real files at `max_px` 256:

| Fixture | Result | Time |
|---|---|---|
| JPEG | 256x144 | 19 ms |
| PNG | 256x144 | 132 ms |
| HEIC | 256x144 | 143 ms |
| PDF | 197x256 | 4 ms |
| MOV | 256x165 | 31 ms |
| MP4 | 256x165 | 25 ms |
| TXT | 256x256 | 12 ms |

One thing worth knowing about ImageIO. Asking for a thumbnail with
`kCGImageSourceCreateThumbnailFromImageIfAbsent` reuses whatever thumbnail the file
already carries, which is fast, but EXIF thumbnails are commonly 160px and it will
hand one back rather than the size you asked for. The first JPEG measurement came
out at 160x90 in 10 ms for that reason. `Source::thumbnail_jpeg` now checks the
returned dimensions and re-renders from the full image when the result is
undersized, which is the 19 ms above.

So the sequence was: the default install is 53.3 MB, and Phase 1 makes it 53.3 MB
with nothing missing. Verify with `cargo run -p sd-imageio --example probe -- <file>...`.

---

## The setup download

`cargo xtask setup` used to download the 91 MB codec bundle unconditionally,
symlink `Spacedrive.framework`, and build the release daemon with
`sd-core/ffmpeg,sd-core/heif`. A fresh clone paid for it whether or not the
contributor ever built those features, and on macOS the system codecs now cover
the same formats.

It is now opt-in through `cargo xtask setup --native-deps`. Without the flag setup
downloads nothing and builds the daemon with default features. An existing
`apps/.deps` is detected and reused, so rerunning plain `setup` does not undo a
previous `--native-deps` run.

The template had to learn the same distinction. `.cargo/config.toml.mustache`
emitted `-L {{nativeDeps}}/lib` and the `heif` link sections outside the
`{{#nativeDeps}}` guard, so with no bundle it would have rendered `-L /lib` and
pointed the linker at the system library directory. Those sections are now guarded,
and the `cargo daemon` and `cargo cli` aliases only carry
`--features sd-core/ffmpeg,sd-core/heif` when the bundle is there. The iOS and
Android sections were guarded the same way, since they keyed off installed rustup
targets alone and would have emitted paths into a directory that was never
downloaded.

Three tests in `xtask/src/config.rs` render the template across every OS and bundle
combination and assert the result parses as TOML, which is the only check that
matters for a file cargo reads before anything else.

---

## What is left, ranked

| Work | Saving | Effort |
|---|---|---|
| `panic = "abort"` | ~4 MB | deliberate tradeoff, currently unwind |
| Investigate `sd_core`'s 9.9 MiB | unknown | unscoped |
| Phase 2: gate iroh | ~2.4 MB | 50 files |

Ending state is 44.4 MB installed, from 156 MB, which is 72%, and a fresh clone
that downloads no dependency bundle at all.

---

## Open questions

**Linux and Windows.** ImageIO and QuickLook are macOS only, so those platforms
keep `ffmpeg` and `heif` behind features and continue to need a bundle. Whether
that bundle stays as-is, drops the dead ONNX payload, or gets replaced per
platform is undecided. The 156 MB to 53.3 MB claim is a macOS claim and the README
should say so.

**Whether `default = []` is right for the desktop app.** The default crate build no
longer includes the WASM extension system. Given `extensions/` is slated for
deletion that looks correct, but it is a behaviour change and the desktop
bundle should be checked before release.

---

## Reproducing

```sh
cargo build --release -p sd-server --features ffmpeg,heif   # baseline
cargo build --release -p sd-server                          # default
size -m target/release/sd-server                            # section breakdown
otool -L target/release/sd-server                           # linkage
CARGO_PROFILE_RELEASE_STRIP=false \
  cargo bloat --release -p sd-server --crates -n 40         # code attribution
```
