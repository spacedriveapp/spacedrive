## Benchmarking Suite

This document explains how to use and extend the benchmarking suite that lives in `benchmarks/`. It covers concepts, CLI commands, recipe schema, data generation, scenarios, metrics, reporting, CI guidance, and troubleshooting.

### Goals

- Reliable, reproducible performance evaluation of core workflows (e.g., indexing discovery, content identification).
- Modular architecture: add scenarios, reporters, and data generators without touching the core wiring.
- CI-friendly: deterministic runs, structured outputs, small quick recipes for PR checks.

## Overview

- `benchmarks/` is a standalone Rust crate that provides:

  - CLI binary: `sd-bench`
  - Dataset generator(s): `benchmarks/src/generator/`
  - Scenarios: `benchmarks/src/scenarios/`
  - Runner & metrics: `benchmarks/src/runner/`, `benchmarks/src/metrics/`
  - Reporting: `benchmarks/src/reporting/`
  - Recipes (YAML): `benchmarks/recipes/`
  - Results (JSON): `benchmarks/results/`

- The CLI boots the core in an isolated data directory, enables job logging, creates/opens a dedicated benchmark library if needed, and orchestrates scenario execution.

## Installation

- Requirements: Rust toolchain, workspace builds.
- Build the bench crate:
  - `cargo build -p sd-bench --bin sd-bench`

## Quickstart

- Generate one recipe:
  - `cargo run -p sd-bench -- mkdata --recipe benchmarks/recipes/nvme_small.yaml`
- Generate all recipes in a directory:
  - `cargo run -p sd-bench -- mkdata-all --recipes-dir benchmarks/recipes`
- Run one scenario with one recipe and write a JSON summary:
  - Discovery: `cargo run -p sd-bench -- run --scenario indexing-discovery --recipe benchmarks/recipes/nvme_small.yaml --out-json benchmarks/results/nvme_small-indexing-discovery.json`
  - Content identification: `cargo run -p sd-bench -- run --scenario content-identification --recipe benchmarks/recipes/nvme_small.yaml --out-json benchmarks/results/nvme_small-content-identification.json`
- Run the scenario for all recipes (writes per-recipe JSON summaries):
  - `cargo run -p sd-bench -- run-all`
  - Equivalent explicit form:
    - `cargo run -p sd-bench -- run-all --scenario indexing-discovery --recipes-dir benchmarks/recipes --out-dir benchmarks/results`
  - Reuse existing datasets without regenerating: `cargo run -p sd-bench -- run-all --skip-generate`

The CLI always prints a brief stdout summary and (if applicable) the path to the generated JSON. It also prints job log paths for later inspection.

## Commands

- `mkdata --recipe <path>`
  - Generates a dataset based on a YAML recipe (see Recipe Schema below).
- `mkdata-all [--recipes-dir <dir>]`
  - Scans a directory for `.yaml` / `.yml` and runs `mkdata` for each file.
- `run --scenario <name> --recipe <path> [--out-json <path>]`
  - Boots an isolated core, ensures a benchmark library, adds recipe locations, waits for jobs to finish.
  - Summarizes metrics to stdout; optionally writes JSON summary at `--out-json`.
- `run-all [--scenario <name>] [--recipes-dir <dir>] [--out-dir <dir>] [--skip-generate]`
  - For each recipe file, runs `mkdata` then `run` and writes JSON per recipe into `--out-dir`.
  - With `--skip-generate`, it will not call `mkdata` and will instead validate that all `locations[].path` exist, skipping recipes whose dataset paths are missing.

## Architecture

- Thin bin: `benchmarks/src/bin/sd-bench-new.rs` delegates to `benchmarks/src/cli/commands.rs`.
- Core modules exported via `benchmarks/src/mod_new.rs`:
  - `generator/` (dataset generation)
  - `scenarios/` (Scenario trait implementations)
  - `runner/` (orchestration & report emission)
  - `metrics/` (result model and phase timings)
  - `reporting/` (reporters like JSON)
  - `core_boot/` (isolated core boot + job logging)
  - `recipe/` (schema + validation)
  - `util/` (helpers)

## Recipe Schema

YAML schema (see `benchmarks/recipes/*.yaml`). Example:

```yaml
name: nvme_small
seed: 12345
locations:
  - path: benchdata/nvme_small
    structure:
      depth: 2
      fanout_per_dir: 8
    files:
      total: 5000
      size_buckets:
        small: { range: [4096, 131072], share: 0.6 }
        medium: { range: [1048576, 5242880], share: 0.3 }
        large: { range: [5242880, 10485760], share: 0.1 }
      extensions: [pdf, zip, jpg, txt]
      duplicate_ratio: 0.1
      content_gen:
        mode: partial # zeros | partial | full
        sample_block_size: 10240 # 10 KiB; aligns with content hashing sample size
        magic_headers: true # write registry-derived magic bytes
media:
  generate_thumbnails: false
```

### Fields

- `name`: logical recipe name.
- `seed`: RNG seed (deterministic runs). If omitted, one is derived from entropy.
- `locations[]`:
  - `path`: base directory for generated files.
  - `structure.depth`: max nested subdirectory depth (randomized per file up to this depth).
  - `structure.fanout_per_dir`: number of subdirectory options at each level.
  - `files.total`: total files per location (before duplicates).
  - `files.size_buckets`: map of bucket name => `{ range: [min, max], share }`; shares are normalized.
  - `files.extensions`: file extension sampling pool (e.g., `[pdf, zip, jpg, txt]`).
  - `files.duplicate_ratio`: fraction of duplicates (hardlink, fallback to copy).
  - `files.content_gen`:
    - `mode`:
      - `zeros`: sparse file; fast; not realistic for content identification.
      - `partial`: writes header + evenly spaced samples + footer; gaps remain sparse zeros; matches content hashing sampling points.
      - `full`: fills the entire file with deterministic bytes; slowest, most realistic.
    - `sample_block_size`: size of each inner sample block (default 10 KiB). Leave at 10 KiB to match the content hashing algorithm.
    - `magic_headers`: if true, writes file signature patterns based on the `file_type` registry for the chosen extension.
- `media` (reserved for future synthetic media generation; currently optional/no-op by default).

## Content Generation Details

- The generator can write content that aligns with the content hash sampling algorithm in `src/domain/content_identity.rs`:
  - For large files (> 100 KiB):
    - Includes file size (handled by the hash function).
    - Hashes a header (8 KiB), 4 evenly spaced inner samples (default 10 KiB each), and a footer (8 KiB).
  - For small files: full-content hashing.
- `partial` mode writes the header/samples/footer only (deterministic pseudo-random bytes), leaving gaps as sparse zeros. This yields realistic, stable hashes without full writes.
- `full` mode writes deterministic content for the entire file for maximum realism.
- `magic_headers: true` uses `sd_core_new::file_type::FileTypeRegistry` to write magic byte signatures for the chosen extension when available.

## Scenarios

- Implement `Scenario` in `benchmarks/src/scenarios/` and register in `scenarios/registry.rs`.
- Built-in:
  - `indexing-discovery`: Adds locations (shallow indexing) and waits for indexing jobs to complete; collects metrics.
  - `content-identification`: Runs content mode and reports content-only throughput using phase timings (excludes discovery).

### Adding a scenario

- Create `benchmarks/src/scenarios/<your_scenario>.rs` implementing:
  - `name(&self) -> &'static str`
  - `describe(&self) -> &'static str`
  - `prepare(&mut self, boot: &CoreBoot, recipe: &Recipe)`
  - `run(&mut self, boot: &CoreBoot, recipe: &Recipe)`
- Register it in `benchmarks/src/scenarios/registry.rs`.

## Metrics and Phase Timing

- The indexer logs a formatted summary including phase timings (discovery, processing, content). The bench runner parses these logs (temporary approach) and produces `ScenarioResult` with:
  - `duration_s`: total duration
  - `discovery_duration_s`, `processing_duration_s`, `content_duration_s`: optional phase timings
  - throughput and counts (files, dirs, total size, errors)
  - `raw_artifacts`: paths to job logs
- For content-only benchmarking, use `content_duration_s` to compute throughput and exclude discovery time.
- Future: event-driven or structured metrics ingestion to avoid log parsing.

## Reporting

- JSON reporter writes summaries into a single JSON:
  - `benchmarks/src/reporting/json_summary.rs` writes `{ "runs": [ ...ScenarioResult... ] }`.
- Register additional reporters in `benchmarks/src/reporting/registry.rs`.
- Planned: Markdown, CSV, HTML.

## Core Boot (Isolated)

- The bench boot uses its own data dir, e.g. `~/Library/Application Support/spacedrive-bench/<scenario>` or the system temp dir fallback.
- Job logging is enabled and sized for benchmarks. Job logs are printed after each run and are included as artifacts in results.
- A dedicated library is created/used for benchmark runs.

## Best Practices

- For fast iteration, use smaller recipes (`nvme_tiny.yaml`) and `content_gen.mode: partial`.
- For realistic content identification, set `magic_headers: true` and `content_gen.mode: partial` or `full` for a subset of files.
- Keep seeds fixed in CI to avoid result variance.

## CI Integration

- Add a job that runs a tiny recipe end-to-end and uploads the JSON summary artifacts (and optionally logs) for inspection.
- Suggested command:
  - `cargo run -p sd-bench -- run --scenario indexing-discovery --recipe benchmarks/recipes/nvme_tiny.yaml --out-json benchmarks/results/ci-indexing-discovery.json`

## Troubleshooting

- “Files look empty / zeros”: ensure your recipe has `files.content_gen` defined with `mode: partial` or `full`, and consider `magic_headers: true`.
- “Unknown scenario”: run with `--scenario indexing-discovery` or add your scenario to `scenarios/registry.rs`.
- “No recipes found”: check `--recipes-dir` path and that files end with `.yaml` or `.yml`.

## Extending the Suite

- Add a generator: implement `DatasetGenerator` in `benchmarks/src/generator/`, register in `generator/registry.rs`.
- Add a reporter: implement `Reporter` in `benchmarks/src/reporting/`, register in `reporting/registry.rs`.
- Add a scenario: see the Scenarios section above.

## References

- CLI entrypoint and commands: `benchmarks/src/bin/sd-bench-new.rs`, `benchmarks/src/cli/commands.rs`
- Dataset generation: `benchmarks/src/generator/filesystem.rs`
- Recipe schema: `benchmarks/src/recipe/schema.rs`
- Scenarios: `benchmarks/src/scenarios/`
- Runner: `benchmarks/src/runner/mod.rs`
- Metrics: `benchmarks/src/metrics/mod.rs`
- Reporting: `benchmarks/src/reporting/`
- Isolated core boot: `benchmarks/src/core_boot/mod.rs`

---

## Future Benchmarks & Roadmap

The suite is designed to grow into a comprehensive performance harness that reflects the whitepaper and system goals.

- **Indexing pipeline**

  - Content identification (done): measure content-only throughput using phase timings.
  - Deep indexing: include thumbnail generation and metadata extraction; track throughput and error rates.
  - Rescan/change detection: cold vs warm cache; latency from change to consistency.

- **File operations**

  - Copy throughput: large vs small files, overlap detection, progressive copy correctness; bytes/s and resource usage.
  - Delete/cleanup: large tree deletion, DB cleanup cost, vacuum.
  - Validation/integrity: CAS verification throughput; corruption handling.

- **Duplicates & de-duplication**

  - Duplicate detection: time to detect N duplicates; content-identity correctness; DB write pressure.

- **Search & querying**

  - (If applicable) index build time and query latency (P50/P95); warm vs cold cache comparisons.

- **Media pipeline**

  - Thumbnail generation: per-kind throughput; GPU/CPU offload if available.
  - Metadata extraction: EXIF/FFprobe across formats.

- **Networking & transfer**

  - Pairing: time-to-pair and success rate under various conditions.
  - Cross-device transfer: LAN/WAN throughput and latency; concurrency sweeps.

- **Volume & system**
  - Volume detection and tracking: discovery latency; multi-volume scaling.
  - Disk type profiling: HDD vs NVMe vs network FS; impact on indexing and copy.

### Data generation enhancements

- Media synthesis: small valid PNG/JPG/WebP; short MP4/AAC clips.
- Rich content sets: archives (ZIP/TAR), PDFs, docs, code, text; symlinks/permissions; nested trees.
- Change-set support: scripted add/modify/delete to exercise rescan.
- Ground-truth manifests: emitted metadata (size, hash) to validate correctness.

### Metrics & telemetry

- Structured metrics export from jobs (avoid log parsing).
- System snapshot per run: CPU/RAM, disk model/FS, OS; thermal state if available.
- Resource usage: CPU%, RSS/peak, IO bytes/IOPS.

### Reporting & analysis

- Markdown/CSV reporters; baseline-diff mode for regression detection.
- HTML dashboard for trend charts over time/history.

### CLI ergonomics

- `--list-scenarios`, `--list-reporters`; recipe filters; scenario parameters (mode, scope, concurrency).
- `--timeout`, `--retries`, `--clean`/`--reuse`; max parallelism; sharding.

### CI integration

- PR smoke tests: tiny recipes for key scenarios; upload JSON/logs.
- Nightly heavy runs on tagged hardware; publish time-series metrics.
- Regression gates: fail PRs on significant metric regressions.
